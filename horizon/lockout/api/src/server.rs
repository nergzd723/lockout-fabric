//! The match on a Horizon server: teams, the board, what each event completes, and the
//! payloads every client is sent. Nothing here touches the host: hooks turn what they hear
//! into an [`Event`], [`State`] answers with [`Action`]s, and `lib.rs` carries them out once
//! the state's lock is released, so a host call that fires another hook can't deadlock.
use crate::board::World;
use crate::compass::Compass;
use crate::draft::{Draft, DraftSettings};
use crate::goals::{self, DATA_NONE, Trigger};
use crate::pause::{self, Pause, Present};
pub use crate::rules::Event;
use crate::rules::{Trackers, Viewer};
use crate::wire::{self, COLOR_ORDER, Payload, TEAM_COLORS, WireGoal, WireTeam};
use std::collections::{HashMap, HashSet};

pub const MIN_BOARD_SIZE: usize = 3;
pub const MAX_BOARD_SIZE: usize = 7;

const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;
const GRAY: u32 = 0xAAAAAA;
const WHITE: u32 = 0xFFFFFF;

/// `§c`: a team colour as a legacy formatting code, as the Java mod's strings carry them.
pub fn legacy_color(color: usize) -> String {
    format!("§{}", "0123456789abcdef".as_bytes()[color.min(15)] as char)
}

/// `A`, `A and B`, `A, B and C`: `Lockout.getWinnerTeamsString`.
pub fn join_and(names: &[String]) -> String {
    let mut out = String::new();
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            out.push_str(if i + 1 == names.len() { " and " } else { ", " });
        }
        out.push_str(n);
    }
    out
}

/// A run of chat text in one colour (0xRRGGBB): Java's lines mix `ChatFormatting` colours.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: u32,
    pub italic: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, color: u32) -> Self {
        Self {
            text: text.into(),
            color,
            italic: false,
        }
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

/// The Java mod saves the running match every 30 seconds.
const SAVE_INTERVAL_TICKS: i64 = 20 * 30;

/// Something for the host to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tell {
        player: u128,
        spans: Vec<Span>,
    },
    Broadcast(Vec<Span>),
    ActionBar {
        player: u128,
        spans: Vec<Span>,
    },
    Send {
        player: u128,
        payload: Payload,
    },
    SendAll {
        payload: Payload,
    },
    /// A console command; only runs from the server's tick.
    Command(String),
    GameMode {
        player: u128,
        mode: i32,
    },
    /// A sound at the player, in `category` (`master`, `blocks`).
    Sound {
        player: u128,
        sound: &'static str,
        category: &'static str,
        volume: f32,
        pitch: f32,
    },
    Teleport {
        player: u128,
        dimension: String,
        pos: [f64; 3],
    },
    Flight {
        player: u128,
        allow: bool,
        flying: bool,
    },
    FlySpeed {
        player: u128,
        speed: Option<f32>,
    },
    /// Freezes the player in place (movement and jump strength to 0, damage refused), as the
    /// Java mod's transient attribute modifier `modifier` and `invulnerable` do; `on: false`
    /// lets them go.
    Freeze {
        player: u128,
        modifier: &'static str,
        on: bool,
    },
    Give {
        player: u128,
        item: String,
    },
    /// Takes every `item` from the player's inventory.
    ClearItem {
        player: u128,
        item: String,
    },
    /// A player-tracking compass, into inventory slot `slot` (41-slot numbering) when given.
    GiveCompass {
        player: u128,
        slot: Option<usize>,
        name: String,
    },
    /// Where the player's tracking compass points; None points it at the world spawn.
    CompassTarget {
        player: u128,
        target: Option<(String, [i32; 3])>,
    },
    RenameCompass {
        player: u128,
        name: String,
    },
    /// Writes the running match to the world's `lockout-match.json`.
    Save,
    /// Clears the player for a new match: inventory, effects, health, food, experience,
    /// fire, statistics and advancements.
    Reset {
        player: u128,
    },
    DeleteSave,
}

#[derive(Debug, Clone)]
pub struct Team {
    pub color: usize,
    pub players: Vec<(u128, String)>,
    pub points: u32,
}

impl Team {
    pub fn new(color: usize, players: Vec<(u128, String)>) -> Self {
        Self {
            color,
            players,
            points: 0,
        }
    }

    /// `LockoutTeam.getDisplayName`: the player for a team of one, else `Team <Colour>`.
    pub fn display_name(&self) -> String {
        match self.players.as_slice() {
            [(_, name)] => name.clone(),
            _ => format!("Team {}", wire::color_display(TEAM_COLORS[self.color].0)),
        }
    }

    pub fn rgb(&self) -> u32 {
        TEAM_COLORS[self.color].1
    }

    pub fn wire(&self) -> WireTeam {
        WireTeam {
            color: TEAM_COLORS[self.color].0.to_owned(),
            players: self.players.iter().map(|(_, n)| n.clone()).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Square {
    pub id: String,
    pub data: String,
    pub name: String,
    pub trigger: Trigger,
    pub completed_by: Option<usize>,
}

#[derive(Debug)]
pub struct Match {
    pub teams: Vec<Team>,
    pub board: Vec<Square>,
    /// Server ticks since the start; negative while players look at the board.
    pub ticks: i64,
    pub started: bool,
    pub running: bool,
    /// The Java mod's progress fields (`Lockout.foodTypesEaten`, ...).
    pub trackers: Trackers,
    /// Advancement titles heard this session, for the tooltips.
    pub titles: HashMap<String, String>,
}

/// An inventory slot's stack, as far as goals look at it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Held {
    pub item: String,
    pub count: u32,
    pub potion: Option<String>,
    pub dyed: Option<i32>,
    /// `ItemStack.isEnchanted`.
    pub enchanted: bool,
    /// A `minecraft:base_color` (a shield with a banner).
    pub base_color: bool,
    /// A bundle's contents, by item id.
    pub bundle: Option<Vec<String>>,
}

impl Held {
    pub fn new(item: &str, count: u32) -> Self {
        Self {
            item: item.to_owned(),
            count,
            ..Self::default()
        }
    }
}

/// A start someone asked for. Vanilla teams are read with `/team list`, which only runs
/// from the server tick, so the command leaves this for the next tick.
#[derive(Debug, Clone)]
pub enum StartRequest {
    Teams {
        requester: Option<u128>,
        names: Vec<String>,
        blackout: bool,
    },
    Players {
        requester: Option<u128>,
        names: Vec<String>,
        blackout: bool,
    },
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub board_size: usize,
    pub start_seconds: i64,
    pub draft: DraftSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            board_size: 5,
            start_seconds: 60,
            draft: DraftSettings::default(),
        }
    }
}

#[derive(Default)]
pub struct State {
    pub settings: Settings,
    pub game: Option<Match>,
    pub custom_board: Option<Vec<(String, String)>>,
    pub pending_start: Option<StartRequest>,
    /// Actions heard inside a player's hook that need every player in reach (chat lines,
    /// sounds, game modes, commands): carried out on the next server tick.
    pub deferred: Vec<Action>,
    /// Who has hurt each entity last: entity id -> (player, tick), so a kill by a bow counts.
    pub last_hurt_by: HashMap<u32, (u128, i64)>,
    pub server_ticks: i64,
    pub draft: Option<Draft>,
    /// What was located near spawn; None until the server has looked.
    pub world: Option<World>,
    /// Set while a match restored after a restart waits for its players.
    pub pause: Option<Pause>,
    pub compass: Option<Compass>,
    /// Players chatting with their team (`/chat team`).
    pub team_chat: HashSet<u128>,
    /// A save exists on disk for the running match.
    pub saved: bool,
}

/// An online player, as the server's tick reads them.
#[derive(Debug, Clone)]
pub struct Online {
    pub uuid: u128,
    pub name: String,
    pub dimension: String,
    pub position: [f64; 3],
    pub xp_level: i32,
    pub dead: bool,
}

impl Online {
    pub fn present(&self) -> Present {
        Present {
            uuid: self.uuid,
            dimension: self.dimension.clone(),
            position: self.position,
        }
    }
}

pub fn tell(player: u128, text: impl Into<String>, color: u32) -> Action {
    Action::Tell {
        player,
        spans: vec![Span::new(text, color)],
    }
}

fn broadcast(text: impl Into<String>, color: u32) -> Action {
    Action::Broadcast(vec![Span::new(text, color)])
}

impl Match {
    pub fn new(teams: Vec<Team>, goals: Vec<(String, String)>, start_ticks: i64) -> Self {
        let table = goals::table();
        let board = goals
            .into_iter()
            .map(|(id, data)| {
                let def = table.get(&id);
                Square {
                    name: def.map_or_else(|| id.clone(), |d| d.name(&data)),
                    trigger: def.map_or(Trigger::Unsupported, |d| d.trigger(&data)),
                    id,
                    data,
                    completed_by: None,
                }
            })
            .collect();
        Self {
            teams,
            board,
            ticks: -start_ticks,
            started: false,
            running: true,
            trackers: Trackers::default(),
            titles: HashMap::new(),
        }
    }

    pub fn mode_name(&self) -> &'static str {
        if self.teams.len() > 1 { "Lockout" } else { "Blackout" }
    }

    /// One team of one player: no tracking compass to hand out.
    pub fn solo_blackout(&self) -> bool {
        self.teams.len() == 1 && self.teams[0].players.len() == 1
    }

    pub fn team_of(&self, player: u128) -> Option<usize> {
        self.teams
            .iter()
            .position(|t| t.players.iter().any(|(u, _)| *u == player))
    }

    pub fn remaining(&self) -> usize {
        self.board.iter().filter(|s| s.completed_by.is_none()).count()
    }

    fn is_winner(&self, team: usize) -> bool {
        if self.teams.len() == 1 {
            return self.remaining() == 0;
        }
        let points = self.teams[team].points as usize;
        self.teams
            .iter()
            .enumerate()
            .all(|(i, t)| i == team || t.points as usize + self.remaining() < points)
    }

    pub fn goals_teams_payload(&self) -> Payload {
        Payload::GoalsTeams {
            teams: self.teams.iter().map(Team::wire).collect(),
            goals: self
                .board
                .iter()
                .map(|s| WireGoal {
                    id: s.id.clone(),
                    data: s.data.clone(),
                    completed_by: s.completed_by.map_or(-1, |t| t as i32),
                })
                .collect(),
            running: self.running,
        }
    }

    /// Everything a client joining now needs.
    pub fn join_payloads(&self, player: u128) -> Vec<Payload> {
        let mut out = vec![self.goals_teams_payload(), Payload::UpdateTimer { ticks: self.ticks }];
        if self.started {
            out.push(Payload::Start);
        }
        let viewer = self
            .team_of(player)
            .map_or(Viewer::Spectator, |t| Viewer::Player(t, player));
        for i in 0..self.board.len() {
            if let Some(tooltip) = self.tooltip(i, viewer) {
                out.push(Payload::UpdateTooltip {
                    goal: self.board[i].id.clone(),
                    tooltip,
                });
            }
        }
        out
    }
}

impl State {
    /// Every online player who isn't on a team watches.
    fn spectators(&self, online: &[u128]) -> Vec<u128> {
        match &self.game {
            Some(m) => online.iter().copied().filter(|p| m.team_of(*p).is_none()).collect(),
            None => Vec::new(),
        }
    }

    /// A team's line is green for it, red for the others, plain for spectators.
    fn announce(&self, winner: usize, message: &str, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = &self.game else { return };
        for (i, team) in m.teams.iter().enumerate() {
            let color = if i == winner { GREEN } else { RED };
            for (p, _) in &team.players {
                out.push(tell(*p, message, color));
            }
        }
        for s in self.spectators(online) {
            out.push(tell(s, message, 0xFFFFFF));
        }
    }

    /// `Lockout.completeGoal(goal, team, message)`: `team` takes square `square`. `online`
    /// are the players on the server.
    pub fn complete(&mut self, square: usize, team: usize, message: String, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = &mut self.game else { return };
        if !m.started || !m.running || m.board[square].completed_by.is_some() {
            return;
        }
        self.complete_quietly(square, team, out);
        self.announce(team, &message, online, out);
        self.finish_completion(square, team, out);
    }

    pub(crate) fn complete_quietly(&mut self, square: usize, team: usize, _out: &mut Vec<Action>) {
        let Some(m) = &mut self.game else { return };
        m.teams[team].points += 1;
        m.board[square].completed_by = Some(team);
    }

    /// The completion's packet (the clients play the sounds, `LockoutClient`'s
    /// CompleteTaskPayload receiver), then the winner check.
    pub(crate) fn finish_completion(&mut self, square: usize, team: usize, out: &mut Vec<Action>) {
        let Some(m) = &self.game else { return };
        out.push(Action::SendAll {
            payload: Payload::CompleteTask {
                goal: m.board[square].id.clone(),
                team: team as i32,
            },
        });
        self.evaluate_winner(team, out);
    }

    /// Takes a "have the most" square from whoever held it.
    pub(crate) fn clear(&mut self, square: usize, out: &mut Vec<Action>) {
        let Some(m) = &mut self.game else { return };
        if let Some(t) = m.board[square].completed_by.take() {
            m.teams[t].points -= 1;
            out.push(Action::SendAll {
                payload: Payload::CompleteTask {
                    goal: m.board[square].id.clone(),
                    team: -1,
                },
            });
        }
    }

    fn evaluate_winner(&mut self, scored: usize, out: &mut Vec<Action>) {
        let Some(m) = &mut self.game else { return };
        let winner = if m.is_winner(scored) {
            Some(scored)
        } else {
            (0..m.teams.len()).find(|&t| m.is_winner(t))
        };
        let mut winners = Vec::new();
        if let Some(w) = winner {
            out.push(broadcast(format!("{} wins.", m.teams[w].display_name()), WHITE));
            winners.push(w as i32);
            m.running = false;
        } else if m.remaining() == 0 && m.teams.len() > 1 {
            let best = m.teams.iter().map(|t| t.points).max().unwrap_or(0);
            let tied: Vec<usize> = (0..m.teams.len()).filter(|&t| m.teams[t].points == best).collect();
            let names: Vec<String> = tied.iter().map(|&t| m.teams[t].display_name()).collect();
            out.push(broadcast(format!("It's a tie! {} win.", join_and(&names)), WHITE));
            winners.extend(tied.iter().map(|&t| t as i32));
            m.running = false;
        }
        if !m.running {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64);
            out.push(Action::SendAll {
                payload: Payload::End { winners, time },
            });
        }
    }

    /// One server tick (`EndServerTickEventHandler`): the draft, the pause, the countdown,
    /// the timer, saving, the compasses and the "have the most" squares.
    pub fn tick(&mut self, online: &[Online], scouts: &[crate::draft::Scout]) -> Vec<Action> {
        let mut out = Vec::new();
        self.server_ticks += 1;
        let uuids: Vec<u128> = online.iter().map(|p| p.uuid).collect();
        if let Some(d) = &mut self.draft {
            let (actions, outcome) = d.tick(scouts);
            out.extend(actions);
            match outcome {
                crate::draft::DraftOutcome::Running => {}
                crate::draft::DraftOutcome::Cancelled => self.draft = None,
                crate::draft::DraftOutcome::Finished { teams, board } => {
                    self.draft = None;
                    out.extend(self.start(teams, board, online));
                }
            }
        }
        // A finished (or replaced) match isn't restored after a restart.
        if self.saved && self.game.as_ref().is_some_and(|m| !m.running) {
            self.saved = false;
            out.push(Action::DeleteSave);
        }
        let Some(m) = &mut self.game else { return out };
        if !m.running {
            return out;
        }
        if let Some(p) = &mut self.pause {
            let present: Vec<Present> = online.iter().map(Online::present).collect();
            if p.tick(m, &present, &mut out) {
                self.pause = None;
            }
            return out;
        }
        if !m.started {
            // `gameStartRunnables`: the countdown lines, then the start.
            let to_start = -m.ticks;
            if [60, 40, 20].contains(&to_start) {
                out.push(broadcast(format!("Starting in {}...", to_start / 20), WHITE));
            }
            if to_start <= 0 {
                m.started = true;
                out.push(Action::SendAll {
                    payload: Payload::Start,
                });
                for t in &m.teams {
                    for (p, _) in &t.players {
                        if uuids.contains(p) {
                            out.push(Action::GameMode { player: *p, mode: 0 });
                        }
                    }
                }
                out.push(broadcast(format!("{} has begun.", m.mode_name()), WHITE));
            }
        }
        m.ticks += 1;
        if m.ticks % SAVE_INTERVAL_TICKS == 0 {
            out.push(Action::Save);
        }
        if let Some(c) = &mut self.compass
            && m.ticks % 10 == 0
        {
            let positions: Vec<(u128, String, [f64; 3])> = online
                .iter()
                .map(|p| (p.uuid, p.dimension.clone(), p.position))
                .collect();
            out.extend(c.update(&positions));
        }
        // `HaveMostXPLevelsGoal`: every online player's level, 0 while dead.
        let levels: Vec<(u128, i32)> = online
            .iter()
            .map(|p| (p.uuid, if p.dead { 0 } else { p.xp_level }))
            .collect();
        self.recalculate_xp(&levels, &uuids, &mut out);
        if let Some(m) = &self.game
            && m.ticks % 20 == 0
        {
            out.push(Action::SendAll {
                payload: Payload::UpdateTimer { ticks: m.ticks },
            });
        }
        out
    }

    /// Starts a match: resets the players (by console commands, run on the next tick),
    /// sends the board and starts the countdown.
    /// Starts a match (`LockoutServer.startLockout`): resets every player (by console
    /// commands, run on the next tick), hands out the tracking compasses, sends the board
    /// and starts the countdown.
    pub fn start(&mut self, teams: Vec<Team>, goals: Vec<(String, String)>, online: &[Online]) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(mut p) = self.pause.take() {
            p.cancel(&mut out);
        }
        let start = self.settings.start_seconds * 20;
        let game = Match::new(teams, goals, start);
        self.last_hurt_by.clear();
        for p in online {
            out.push(Action::Reset { player: p.uuid });
            if game.team_of(p.uuid).is_some() {
                out.push(Action::GameMode {
                    player: p.uuid,
                    mode: 2,
                });
            } else {
                out.push(Action::GameMode {
                    player: p.uuid,
                    mode: 3,
                });
                out.push(Action::Tell {
                    player: p.uuid,
                    spans: vec![Span::new("You are spectating this match.", GRAY).italic()],
                });
            }
        }
        let compass = Compass::new(&game.teams);
        out.push(Action::Command("time set 0".into()));
        for p in online {
            for payload in game.join_payloads(p.uuid) {
                out.push(Action::Send {
                    player: p.uuid,
                    payload,
                });
            }
            if !game.solo_blackout() && game.team_of(p.uuid).is_some() {
                out.push(Action::GiveCompass {
                    player: p.uuid,
                    slot: None,
                    name: compass.label(p.uuid),
                });
            }
        }
        self.compass = Some(compass);
        self.game = Some(game);
        out.push(Action::Save);
        out
    }

    /// `/lockout cancel` and `/blackout cancel` (`LockoutServer.cancelCommand`): ends a draft,
    /// or the running match with no winner (`EndLockoutPayload` with none, so Fabric clients
    /// end it too), and puts every player back in survival. None when nothing runs.
    pub fn cancel_match(&mut self, online: &[Online]) -> Option<Vec<Action>> {
        if self.draft.is_some() {
            return Some(self.cancel_draft());
        }
        let mut out = Vec::new();
        let running = self.game.as_ref().is_some_and(|m| m.running);
        if !running {
            return None;
        }
        if let Some(mut p) = self.pause.take() {
            p.cancel(&mut out);
        }
        if let Some(m) = &mut self.game {
            m.running = false;
        }
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        out.push(Action::SendAll {
            payload: Payload::End {
                winners: Vec::new(),
                time,
            },
        });
        out.push(broadcast("The match was cancelled.", RED));
        for p in online {
            out.push(Action::GameMode {
                player: p.uuid,
                mode: 0,
            });
        }
        Some(out)
    }

    /// Ends a draft in progress, if there is one (`LockoutServer.cancelDraft`).
    pub fn cancel_draft(&mut self) -> Vec<Action> {
        self.draft.take().map(|mut d| d.cancel()).unwrap_or_default()
    }

    /// `/lockout draft` and `/blackout draft` (`LockoutServer.startDraft`): the running match
    /// stops, drafting players wait in adventure mode and everyone else spectates.
    pub fn start_draft(
        &mut self,
        teams: Vec<Team>,
        online: &[Online],
        scouts: &[crate::draft::Scout],
        seed: u64,
    ) -> Vec<Action> {
        let mut out = self.cancel_draft();
        if let Some(mut p) = self.pause.take() {
            p.cancel(&mut out);
        }
        if let Some(m) = &mut self.game {
            m.running = false;
        }
        for p in online {
            let drafting = teams.iter().any(|t| t.players.iter().any(|(u, _)| *u == p.uuid));
            out.push(Action::GameMode {
                player: p.uuid,
                mode: if drafting { 2 } else { 3 },
            });
        }
        let world = self.world.clone().unwrap_or_default();
        let mut draft = Draft::new(
            teams,
            self.settings.board_size,
            self.settings.draft,
            world,
            crate::board::Rng::new(seed),
        );
        out.extend(draft.start(scouts));
        self.draft = Some(draft);
        out
    }

    /// A drafting player's pick or reroll.
    pub fn draft_action(
        &mut self,
        player: u128,
        action: i32,
        online: &[Online],
        scouts: &[crate::draft::Scout],
    ) -> Vec<Action> {
        let Some(d) = &mut self.draft else { return Vec::new() };
        let (mut out, outcome) = d.action(player, action, scouts);
        if let crate::draft::DraftOutcome::Finished { teams, board } = outcome {
            self.draft = None;
            out.extend(self.start(teams, board, online));
        }
        out
    }

    /// Whether the player is frozen (scouting or paused), and so can't be hurt.
    pub fn frozen(&self, player: u128) -> bool {
        self.draft.as_ref().is_some_and(|d| d.is_scouting(player))
            || self.pause.as_ref().is_some_and(|p| p.is_frozen(player))
    }

    /// Restores a saved match (`LockoutServer.restoreSavedMatch`): paused until every player
    /// is back.
    pub fn restore(&mut self, game: Match) {
        self.compass = Some(Compass::new(&game.teams));
        self.game = Some(game);
        self.pause = Some(Pause::new());
        self.saved = true;
    }

    /// A player joined (`LockoutVersionPayload`'s receiver on the Fabric server): their game
    /// mode, tooltips, the board and the timer.
    pub fn join(&self, player: u128, online: &[Online]) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(m) = &self.game else { return out };
        if !m.running {
            return out;
        }
        if m.team_of(player).is_some() {
            if self.pause.is_some() {
                // Frozen by the pause until everyone is back.
                out.push(Action::GameMode { player, mode: 2 });
                let present: Vec<Present> = online.iter().map(Online::present).collect();
                let missing = pause::missing(m, &present);
                if !missing.is_empty()
                    && let Action::Broadcast(spans) = pause::waiting_message(&missing)
                {
                    out.push(Action::Tell { player, spans });
                }
            } else {
                out.push(Action::GameMode { player, mode: 0 });
            }
        } else {
            out.push(Action::GameMode { player, mode: 3 });
            out.push(Action::Tell {
                player,
                spans: vec![Span::new("You are spectating this match.", GRAY).italic()],
            });
        }
        for payload in m.join_payloads(player) {
            out.push(Action::Send { player, payload });
        }
        out
    }

    /// `/GiveGoal <player> <goal number>`: the goal goes to that player's team, whoever held it.
    pub fn give_goal(
        &mut self,
        player: Option<(u128, String)>,
        number: usize,
        online: &[u128],
    ) -> Result<(String, Vec<Action>), String> {
        let Some(m) = &self.game else {
            return Err("There's no active lockout match.".into());
        };
        if !m.running {
            return Err("There's no active lockout match.".into());
        }
        let Some((uuid, name)) = player else {
            return Err("Invalid target.".into());
        };
        if m.team_of(uuid).is_none() {
            return Err(format!("Player {name} is not playing Lockout."));
        }
        if number == 0 || number > m.board.len() {
            return Err("Goal number does not exist on the board.".into());
        }
        let square = number - 1;
        let reply = format!("Gave {name} goal \"{}\".", m.board[square].name);
        let mut out = Vec::new();
        self.update_goal_completion(square, uuid, online, &mut out);
        Ok((reply, out))
    }
}

/// The teams, and the vanilla teams (name, colour) whose colour was set.
pub type TeamsAndRecolors = (Vec<Team>, Vec<(String, String)>);

/// A vanilla team as `/team list` describes it: None when there is no such team.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedTeam {
    pub name: String,
    pub color: Option<String>,
    pub members: Vec<String>,
}

/// `/lockout teams` and `/blackout team` (`LockoutServer.parseArgumentsIntoTeams`): the
/// teams, and the vanilla teams whose colour had to be set because they had none or shared
/// one (`team modify <name> color <colour>`, as the Java mod sets it).
pub fn teams_from(
    names: &[String],
    listed: &[Option<ListedTeam>],
    online: &[(u128, String)],
    blackout: bool,
) -> Result<TeamsAndRecolors, String> {
    if blackout {
        if names.is_empty() {
            return Err("Not enough teams listed.".into());
        }
        if names.len() > 1 {
            return Err("Only one team can play Blackout.".into());
        }
    } else {
        if names.len() < 2 {
            return Err("Not enough teams listed. Make sure you separate team names with spaces.".into());
        }
        if names.len() > 16 {
            return Err("Too many teams listed.".into());
        }
    }
    let find = |m: &str| online.iter().find(|(_, n)| n.eq_ignore_ascii_case(m)).cloned();
    let mut found = Vec::new();
    for (name, team) in names.iter().zip(listed) {
        let Some(team) = team else {
            return Err(format!("Team {name} is invalid."));
        };
        for m in &team.members {
            if find(m).is_none() {
                return Err(format!(
                    "Player {m} on team {name} is invalid. Remove them from the team and try again."
                ));
            }
        }
        found.push(team);
    }
    let mut teams: Vec<Team> = Vec::new();
    let mut recolored = Vec::new();
    for team in found {
        if team.members.is_empty() {
            return Err(format!("Team {} doesn't have any players.", team.name));
        }
        let mut color = team
            .color
            .as_deref()
            .and_then(|c| TEAM_COLORS.iter().position(|(n, _)| *n == c));
        if color.is_none_or(|c| teams.iter().any(|t| t.color == c)) {
            // Select an available colour.
            match COLOR_ORDER
                .iter()
                .copied()
                .find(|c| !teams.iter().any(|t| t.color == *c))
            {
                Some(c) => {
                    color = Some(c);
                    recolored.push((team.name.clone(), TEAM_COLORS[c].0.to_owned()));
                }
                None => {
                    return Err(format!(
                        "Could not find assignable color for team {}. Try recreating teams.",
                        team.name
                    ));
                }
            }
        }
        let players = team.members.iter().filter_map(|m| find(m)).collect();
        teams.push(Team::new(color.unwrap_or(COLOR_ORDER[0]), players));
    }
    Ok((teams, recolored))
}

/// One team per player (`/lockout players`), or one team of them all (`/blackout players`).
pub fn teams_of_players(names: &[String], online: &[(u128, String)], blackout: bool) -> Result<Vec<Team>, String> {
    if blackout && names.is_empty() {
        return Err("Not enough players listed.".into());
    }
    if !blackout {
        if names.len() < 2 {
            return Err("Not enough players listed. Make sure you separate player names with spaces.".into());
        }
        if names.len() > 16 {
            return Err("Too many players listed.".into());
        }
    }
    let mut players = Vec::new();
    for name in names {
        match online.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)) {
            Some(p) => players.push(p.clone()),
            None => return Err(format!("Player {name} is invalid.")),
        }
    }
    if blackout {
        return Ok(vec![Team::new(COLOR_ORDER[0], players)]);
    }
    Ok(players
        .into_iter()
        .enumerate()
        .map(|(i, p)| Team::new(COLOR_ORDER[i], vec![p]))
        .collect())
}

/// A custom board's problems, as the Java mod lists them.
pub fn invalid_goals(goals: &[(String, String)]) -> Vec<String> {
    let table = goals::table();
    goals
        .iter()
        .filter(|(id, _)| table.get(id).is_none())
        .map(|(id, data)| {
            format!(
                " - '{id}'{}",
                if data == DATA_NONE {
                    String::new()
                } else {
                    format!(" with data: '{data}'")
                }
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Polled;

    const A: u128 = 1;
    const B: u128 = 2;

    fn online(xp: &[(u128, i32)]) -> Vec<Online> {
        xp.iter()
            .map(|(u, l)| Online {
                uuid: *u,
                name: if *u == A { "Alex".into() } else { "Steve".into() },
                dimension: "minecraft:overworld".into(),
                position: [0.0, 64.0, 0.0],
                xp_level: *l,
                dead: false,
            })
            .collect()
    }

    fn running(goals: &[&str]) -> State {
        let mut s = State::default();
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(A, "Alex".into()), (B, "Steve".into())],
            false,
        )
        .unwrap();
        let mut m = Match::new(
            teams,
            goals.iter().map(|g| (g.to_string(), DATA_NONE.to_string())).collect(),
            0,
        );
        m.started = true;
        s.game = Some(m);
        s
    }

    fn completed(actions: &[Action]) -> Vec<(String, i32)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::SendAll {
                    payload: Payload::CompleteTask { goal, team },
                } => Some((goal.clone(), *team)),
                _ => None,
            })
            .collect()
    }

    fn tooltips(actions: &[Action], to: u128) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Send {
                    player,
                    payload: Payload::UpdateTooltip { tooltip, .. },
                } if *player == to => Some(tooltip.clone()),
                _ => None,
            })
            .collect()
    }

    fn polled(player: u128, stacks: Vec<Option<Held>>) -> Event {
        Event::Tick(Box::new(Polled {
            player,
            name: if player == A { "Alex".into() } else { "Steve".into() },
            stacks,
            dimension: "minecraft:overworld".into(),
            y: 64.0,
            inside: "minecraft:air".into(),
            below: "minecraft:stone".into(),
            food: 20,
            ..Polled::default()
        }))
    }

    fn kill(player: u128, kind: &str) -> Event {
        Event::Kill {
            player,
            kind: format!("minecraft:{kind}"),
            color: None,
            dimension: "minecraft:overworld".into(),
            damage_type: "minecraft:player_attack".into(),
        }
    }

    #[test]
    fn entering_the_nether_completes_for_the_player_team() {
        let mut s = running(&["ENTER_NETHER", "ENTER_END", "KILL_WITCH"]);
        let out = s.handle(
            Event::Dimension {
                player: B,
                dimension: "minecraft:the_nether".into(),
            },
            &[A, B],
        );
        assert_eq!(completed(&out), vec![("ENTER_NETHER".into(), 1)]);
        assert!(out.contains(&tell(B, "Steve completed Enter Nether.", GREEN)));
        assert!(out.contains(&tell(A, "Steve completed Enter Nether.", RED)));
        // Taken goals stay taken.
        assert!(
            completed(&s.handle(
                Event::Dimension {
                    player: A,
                    dimension: "minecraft:the_nether".into()
                },
                &[A, B]
            ))
            .is_empty()
        );
    }

    #[test]
    fn unique_kills_count_each_kind_once_with_the_java_tooltip() {
        let mut s = running(&["KILL_7_UNIQUE_HOSTILE_MOBS", "ENTER_END", "ENTER_NETHER"]);
        let mut last = Vec::new();
        for kind in [
            "zombie", "zombie", "skeleton", "creeper", "spider", "witch", "pig", "enderman",
        ] {
            last = s.handle(kill(A, kind), &[A, B]);
            assert!(completed(&last).is_empty());
        }
        assert_eq!(
            tooltips(&last, A),
            vec![
                " \nUnique Hostile Mobs Killed: 6/7\n§7 §oZombie, Skeleton, Creeper, Spider, Witch,\n§7 §oEnderman\n "
            ]
        );
        let out = s.handle(kill(A, "blaze"), &[A, B]);
        assert_eq!(completed(&out), vec![("KILL_7_UNIQUE_HOSTILE_MOBS".into(), 0)]);
        assert!(out.contains(&tell(A, "Alex completed Kill 7 Unique Hostile Mobs.", GREEN)));
        // Spectators see every team's count.
        let m = s.game.as_ref().unwrap();
        assert_eq!(
            m.tooltip(0, crate::rules::Viewer::Spectator).unwrap(),
            " \n§cAlex§r: 7/7\n§9Steve§r: 0/7\n "
        );
    }

    #[test]
    fn inventory_goals_read_counts_and_armour() {
        let mut s = running(&[
            "OBTAIN_WOODEN_TOOLS",
            "WEAR_LEATHER_ARMOR",
            "OBTAIN_64_ARROWS",
            "ENTER_END",
            "KILL_WITCH",
            "KILL_GHAST",
        ]);
        let mut stacks: Vec<Option<Held>> = vec![None; 41];
        for (i, t) in ["axe", "hoe", "pickaxe", "sword"].iter().enumerate() {
            stacks[i] = Some(Held::new(&format!("minecraft:wooden_{t}"), 1));
        }
        stacks[10] = Some(Held::new("minecraft:arrow", 40));
        stacks[11] = Some(Held::new("minecraft:arrow", 24));
        for (i, p) in ["boots", "leggings", "chestplate", "helmet"].iter().enumerate() {
            stacks[36 + i] = Some(Held::new(&format!("minecraft:leather_{p}"), 1));
        }
        let out = s.handle(polled(A, stacks.clone()), &[A, B]);
        assert_eq!(
            completed(&out),
            vec![("WEAR_LEATHER_ARMOR".into(), 0), ("OBTAIN_64_ARROWS".into(), 0)]
        );
        stacks[4] = Some(Held::new("minecraft:wooden_shovel", 1));
        let out = s.handle(polled(B, stacks), &[A, B]);
        assert_eq!(completed(&out), vec![("OBTAIN_WOODEN_TOOLS".into(), 1)]);
    }

    #[test]
    fn dyed_leather_must_be_the_board_colour() {
        let mut s = State::default();
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(A, "Alex".into()), (B, "Steve".into())],
            false,
        )
        .unwrap();
        let goals = vec![
            (
                "WEAR_COLORED_LEATHER_ARMOR_PIECE".to_string(),
                "leather_boots&red".to_string(),
            ),
            ("KILL_SNOW_GOLEM_IN_NETHER".into(), DATA_NONE.into()),
            ("ENTER_END".into(), DATA_NONE.into()),
            ("KILL_WITCH".into(), DATA_NONE.into()),
            ("KILL_GHAST".into(), DATA_NONE.into()),
        ];
        let mut m = Match::new(teams, goals, 0);
        m.started = true;
        s.game = Some(m);
        let mut stacks: Vec<Option<Held>> = vec![None; 41];
        stacks[36] = Some(Held {
            dyed: Some(0x3C44AA),
            ..Held::new("minecraft:leather_boots", 1)
        });
        assert!(completed(&s.handle(polled(A, stacks.clone()), &[A, B])).is_empty());
        stacks[36] = Some(Held {
            dyed: Some(0xB02E26),
            ..Held::new("minecraft:leather_boots", 1)
        });
        let out = s.handle(polled(A, stacks), &[A, B]);
        assert_eq!(completed(&out), vec![("WEAR_COLORED_LEATHER_ARMOR_PIECE".into(), 0)]);
        let golem = |d: &str| Event::Kill {
            player: B,
            kind: "minecraft:snow_golem".into(),
            color: None,
            dimension: d.into(),
            damage_type: "minecraft:player_attack".into(),
        };
        assert!(completed(&s.handle(golem("minecraft:overworld"), &[A, B])).is_empty());
        assert_eq!(
            completed(&s.handle(golem("minecraft:the_nether"), &[A, B])),
            vec![("KILL_SNOW_GOLEM_IN_NETHER".into(), 1)]
        );
    }

    #[test]
    fn a_team_that_cannot_be_caught_wins() {
        let mut s = running(&["ENTER_NETHER", "ENTER_END", "KILL_WITCH"]);
        s.handle(
            Event::Dimension {
                player: A,
                dimension: "minecraft:the_nether".into(),
            },
            &[A, B],
        );
        let out = s.handle(
            Event::Dimension {
                player: A,
                dimension: "minecraft:the_end".into(),
            },
            &[A, B],
        );
        assert!(
            out.iter()
                .any(|a| matches!(a, Action::SendAll { payload: Payload::End { winners, .. } } if winners == &vec![0]))
        );
        assert!(!s.game.as_ref().unwrap().running);
    }

    #[test]
    fn opponent_goals_go_to_the_other_team_with_the_java_line() {
        let mut s = running(&[
            "OPPONENT_OBTAINS_OBSIDIAN",
            "OPPONENT_TOUCHES_WATER",
            "ENTER_END",
            "KILL_WITCH",
        ]);
        let mut stacks = vec![None; 41];
        stacks[0] = Some(Held::new("minecraft:obsidian", 1));
        let out = s.handle(polled(A, stacks), &[A, B]);
        assert_eq!(completed(&out), vec![("OPPONENT_OBTAINS_OBSIDIAN".into(), 1)]);
        assert!(out.contains(&tell(B, "Alex obtained Obsidian.", GREEN)));
        assert!(out.contains(&tell(A, "Alex obtained Obsidian.", RED)));
        let Event::Tick(mut p) = polled(B, vec![None; 41]) else {
            unreachable!()
        };
        p.inside = "minecraft:water".into();
        let out = s.handle(Event::Tick(p), &[A, B]);
        assert_eq!(completed(&out), vec![("OPPONENT_TOUCHES_WATER".into(), 0)]);
        assert!(out.contains(&tell(A, "Steve touched water.", GREEN)));
    }

    #[test]
    fn a_pig_needs_a_carrot_on_a_stick() {
        let mut s = running(&["RIDE_PIG", "ENTER_END", "KILL_WITCH"]);
        let Event::Tick(mut p) = polled(A, vec![None; 41]) else {
            unreachable!()
        };
        p.vehicle = Some("minecraft:pig".into());
        assert!(completed(&s.handle(Event::Tick(p.clone()), &[A, B])).is_empty());
        p.stacks[3] = Some(Held::new("minecraft:carrot_on_a_stick", 1));
        p.selected = 3;
        assert_eq!(
            completed(&s.handle(Event::Tick(p), &[A, B])),
            vec![("RIDE_PIG".into(), 0)]
        );
    }

    #[test]
    fn the_carved_pumpkin_counts_five_minutes_worn() {
        let mut s = running(&["WEAR_CARVED_PUMPKIN_FOR_5_MINUTES", "ENTER_END", "KILL_WITCH"]);
        let mut stacks = vec![None; 41];
        stacks[39] = Some(Held::new("minecraft:carved_pumpkin", 1));
        for _ in 0..(5 * 60 * 20 - 1) {
            assert!(completed(&s.handle(polled(A, stacks.clone()), &[A, B])).is_empty());
        }
        let out = s.handle(polled(A, stacks), &[A, B]);
        assert_eq!(completed(&out), vec![("WEAR_CARVED_PUMPKIN_FOR_5_MINUTES".into(), 0)]);
        let m = s.game.as_ref().unwrap();
        assert_eq!(
            m.tooltip(0, crate::rules::Viewer::Player(0, A)).unwrap(),
            " \nTime worn: 05:00\n "
        );
    }

    #[test]
    fn biomes_are_adventuring_time_criteria() {
        let mut s = running(&[
            "VISIT_MUSHROOM_BIOME",
            "VISIT_10_UNIQUE_BIOMES",
            "ENTER_END",
            "KILL_WITCH",
        ]);
        let visit = |b: &str| Event::Criterion {
            player: B,
            advancement: "minecraft:adventure/adventuring_time".into(),
            criterion: format!("minecraft:{b}"),
        };
        let out = s.handle(visit("mushroom_fields"), &[A, B]);
        assert_eq!(completed(&out), vec![("VISIT_MUSHROOM_BIOME".into(), 1)]);
        assert_eq!(
            tooltips(&out, B),
            vec![" \nBiomes visited: 1/10\n§7 §oMushroom Fields\n "]
        );
        // Another advancement's criteria are no biomes.
        let other = Event::Criterion {
            player: B,
            advancement: "minecraft:husbandry/balanced_diet".into(),
            criterion: "minecraft:apple".into(),
        };
        assert!(s.handle(other, &[A, B]).is_empty());
    }

    #[test]
    fn only_foods_count_as_eaten_and_honey_is_one() {
        let mut s = running(&["EAT_5_UNIQUE_FOOD", "OPPONENT_EATS_FOOD", "ENTER_END", "KILL_WITCH"]);
        let eat = |item: &str, food: bool| Event::Consumed {
            player: A,
            item: format!("minecraft:{item}"),
            potion: None,
            food,
        };
        assert!(completed(&s.handle(eat("milk_bucket", false), &[A, B])).is_empty());
        let out = s.handle(eat("honey_bottle", true), &[A, B]);
        assert_eq!(completed(&out), vec![("OPPONENT_EATS_FOOD".into(), 1)]);
        assert!(out.contains(&tell(A, "Alex ate food.", RED)));
        for item in ["beef", "apple", "bread"] {
            s.handle(eat(item, true), &[A, B]);
        }
        let out = s.handle(eat("cookie", true), &[A, B]);
        assert_eq!(completed(&out), vec![("EAT_5_UNIQUE_FOOD".into(), 0)]);
        assert!(out.contains(&tell(A, "Alex completed Eat 5 Unique Food.", GREEN)));
    }

    #[test]
    fn damage_goals_count_landed_hits_by_team() {
        let mut s = running(&[
            "TAKE_200_DAMAGE",
            "OPPONENT_TAKES_FALL_DAMAGE",
            "ENTER_END",
            "KILL_WITCH",
        ]);
        let hit = |amount: f64, damage_type: &str| Event::Damage {
            victim: Some(B),
            attacker: None,
            arrow: false,
            damage_type: damage_type.into(),
            amount,
        };
        let out = s.handle(hit(150.5, "minecraft:lava"), &[A, B]);
        assert_eq!(tooltips(&out, B), vec![" \nDamage: 150/200\n "]);
        let out = s.handle(hit(3.0, "minecraft:fall"), &[A, B]);
        assert_eq!(completed(&out), vec![("OPPONENT_TAKES_FALL_DAMAGE".into(), 0)]);
        assert!(out.contains(&tell(B, "Steve took fall damage.", RED)));
        let out = s.handle(hit(50.0, "minecraft:lava"), &[A, B]);
        assert_eq!(completed(&out), vec![("TAKE_200_DAMAGE".into(), 1)]);
        assert!(out.contains(&tell(B, "Steve completed Take 200 damage.", GREEN)));
    }

    #[test]
    fn a_match_can_be_cancelled_with_no_winner() {
        let mut s = running(&["ENTER_END", "ENTER_NETHER", "KILL_WITCH"]);
        let out = s.cancel_match(&online(&[(A, 0), (B, 0)])).unwrap();
        assert!(out.iter().any(|a| matches!(
            a,
            Action::SendAll { payload: Payload::End { winners, .. } } if winners.is_empty()
        )));
        assert!(out.contains(&Action::GameMode { player: A, mode: 0 }));
        assert!(!s.game.as_ref().unwrap().running);
        // Nothing left to cancel.
        assert!(s.cancel_match(&online(&[(A, 0)])).is_none());
    }

    #[test]
    fn the_countdown_starts_the_match() {
        let mut s = State::default();
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(A, "Alex".into()), (B, "Steve".into())],
            false,
        )
        .unwrap();
        s.game = Some(Match::new(teams, vec![("ENTER_END".into(), DATA_NONE.into())], 100));
        let mut started = false;
        // `gameStartRunnables`: the start runs on the tick after the countdown's last.
        for _ in 0..101 {
            let out = s.tick(&online(&[(A, 0), (B, 0)]), &[]);
            started |= out.iter().any(|a| {
                matches!(
                    a,
                    Action::SendAll {
                        payload: Payload::Start
                    }
                )
            });
        }
        assert!(started && s.game.as_ref().unwrap().started);
    }

    #[test]
    fn most_xp_levels_stays_with_its_holder_on_a_tie() {
        let mut s = running(&[
            "HAVE_MORE_XP_LEVELS",
            "ENTER_END",
            "KILL_WITCH",
            "ENTER_NETHER",
            "KILL_GHAST",
        ]);
        s.tick(&online(&[(A, 5), (B, 3)]), &[]);
        assert_eq!(s.game.as_ref().unwrap().board[0].completed_by, Some(0));
        // `recalculateXPGoal`: a tie keeps the goal where it is.
        s.tick(&online(&[(A, 5), (B, 5)]), &[]);
        assert_eq!(s.game.as_ref().unwrap().board[0].completed_by, Some(0));
        s.tick(&online(&[(A, 5), (B, 9)]), &[]);
        assert_eq!(s.game.as_ref().unwrap().board[0].completed_by, Some(1));
        let points: Vec<u32> = s.game.as_ref().unwrap().teams.iter().map(|t| t.points).collect();
        assert_eq!(points, vec![0, 1]);
        // Nobody with a level: nobody has it.
        let out = s.tick(&online(&[(A, 0), (B, 0)]), &[]);
        assert_eq!(completed(&out), vec![("HAVE_MORE_XP_LEVELS".into(), -1)]);
    }
}
