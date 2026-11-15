//! The match on a Horizon server: teams, the board, what each event completes, and the
//! payloads every client is sent. Nothing here touches the host: hooks turn what they hear
//! into an [`Event`], [`State`] answers with [`Action`]s, and `lib.rs` carries them out once
//! the state's lock is released, so a host call that fires another hook can't deadlock.
use crate::goals::{self, DATA_NONE, HOSTILE, LEATHER_RGB, OpponentEvent, Trigger};
use crate::wire::{self, COLOR_ORDER, Payload, TEAM_COLORS, WireGoal, WireTeam};
use std::collections::{BTreeSet, HashMap};

pub const MIN_BOARD_SIZE: usize = 3;
pub const MAX_BOARD_SIZE: usize = 7;

const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;
const GRAY: u32 = 0xAAAAAA;

/// Something for the host to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tell {
        player: u128,
        text: String,
        color: u32,
    },
    Broadcast {
        text: String,
        color: u32,
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
    Sound {
        player: u128,
        sound: &'static str,
        pitch: f32,
    },
}

#[derive(Debug, Clone)]
pub struct Team {
    pub name: String,
    pub color: usize,
    pub players: Vec<(u128, String)>,
    pub points: u32,
}

impl Team {
    fn wire(&self) -> WireTeam {
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

/// A goal's progress for one team: what it has seen, and how much.
#[derive(Debug, Clone, Default)]
struct Progress {
    seen: BTreeSet<String>,
    count: f64,
}

#[derive(Debug)]
pub struct Match {
    pub teams: Vec<Team>,
    pub board: Vec<Square>,
    /// Server ticks since the start; negative while players look at the board.
    pub ticks: i64,
    pub started: bool,
    pub running: bool,
    progress: HashMap<(usize, usize), Progress>,
}

/// An inventory slot's stack, as far as goals look at it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Held {
    pub item: String,
    pub count: u32,
    pub potion: Option<String>,
    pub dyed: Option<i32>,
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

/// What a hook heard, in the terms goals care about. Player ids are UUIDs.
#[derive(Debug, Clone)]
pub enum Event {
    /// The 41 inventory slots (`Inventory` numbering: 36..40 the armour, feet first).
    Inventory {
        player: u128,
        stacks: Vec<Option<Held>>,
    },
    Advancement {
        player: u128,
        id: String,
    },
    Kill {
        player: u128,
        kind: String,
        color: Option<String>,
        dimension: Option<String>,
    },
    Death {
        player: u128,
        damage_type: String,
        killer_kind: Option<String>,
        killer_player: Option<u128>,
        message_key: Option<String>,
    },
    Breed {
        player: u128,
        kind: String,
    },
    Tame {
        player: u128,
        kind: String,
    },
    EffectAdded {
        player: u128,
        effect: String,
    },
    Effects {
        player: u128,
        effects: Vec<String>,
    },
    Consumed {
        player: u128,
        item: String,
        potion: Option<String>,
    },
    Stat {
        player: u128,
        stat: String,
    },
    Mined {
        player: u128,
        block: String,
    },
    Dimension {
        player: u128,
        dimension: String,
    },
    Position {
        player: u128,
        dimension: String,
        y: f64,
        biome: Option<String>,
        below: Option<String>,
        in_water: bool,
        on_fire: bool,
    },
    XpLevel {
        player: u128,
        level: i32,
    },
    Crafted {
        player: u128,
        item: String,
    },
    Ride {
        player: u128,
        vehicle: String,
    },
    Named {
        player: u128,
        kind: String,
        name: String,
    },
    ItemBroken {
        player: u128,
        item: String,
    },
    PlacedBlock {
        player: u128,
        item: String,
    },
    PlacedHanging {
        player: u128,
        kind: String,
    },
    UsedItem {
        player: u128,
        item: String,
    },
    MilkAway {
        player: u128,
    },
    Food {
        player: u128,
        from: i32,
        to: i32,
    },
    Enraged {
        player: u128,
        kind: String,
    },
    ShieldDisabled {
        player: u128,
    },
    ItemFrameInFrame {
        player: u128,
    },
    WolfArmor {
        player: u128,
    },
    Stonecutter {
        player: u128,
    },
    Loom {
        player: u128,
    },
    Anvil {
        player: u128,
    },
    Damage {
        victim: Option<u128>,
        attacker: Option<u128>,
        damage_type: String,
        amount: f32,
    },
    HitBy {
        victim: u128,
        projectile: String,
    },
    PickedUp {
        player: u128,
        thrower: Option<u128>,
    },
    SprintCm {
        player: u128,
        cm: i32,
    },
}

impl Event {
    fn player(&self) -> Option<u128> {
        use Event::*;
        match self {
            Inventory { player, .. }
            | Advancement { player, .. }
            | Kill { player, .. }
            | Death { player, .. }
            | Breed { player, .. }
            | Tame { player, .. }
            | EffectAdded { player, .. }
            | Effects { player, .. }
            | Consumed { player, .. }
            | Stat { player, .. }
            | Mined { player, .. }
            | Dimension { player, .. }
            | Position { player, .. }
            | XpLevel { player, .. }
            | Crafted { player, .. }
            | Ride { player, .. }
            | Named { player, .. }
            | ItemBroken { player, .. }
            | PlacedBlock { player, .. }
            | PlacedHanging { player, .. }
            | UsedItem { player, .. }
            | MilkAway { player }
            | Food { player, .. }
            | Enraged { player, .. }
            | ShieldDisabled { player }
            | ItemFrameInFrame { player }
            | WolfArmor { player }
            | Stonecutter { player }
            | Loom { player }
            | Anvil { player }
            | PickedUp { player, .. }
            | SprintCm { player, .. } => Some(*player),
            HitBy { victim, .. } => Some(*victim),
            Damage { victim, attacker, .. } => victim.or(*attacker),
        }
    }
}

/// What a team does to a goal.
enum Outcome {
    None,
    /// The player's team completes it.
    Complete,
    /// The other team does (1v1 goals about what happens to an opponent).
    CompleteForOpponent,
    /// The progress changed: the tooltip is sent again.
    Progressed,
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
    /// Also put goals that need a biome or structure nearby on random boards.
    pub located_goals: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            board_size: 5,
            start_seconds: 60,
            located_goals: true,
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
    /// The sprint statistic each player had at the start, so only this match's sprint counts.
    pub sprint_base: HashMap<u128, i32>,
    pub server_ticks: i64,
}

fn tell(player: u128, text: impl Into<String>, color: u32) -> Action {
    Action::Tell {
        player,
        text: text.into(),
        color,
    }
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
            progress: HashMap::new(),
        }
    }

    pub fn mode_name(&self) -> &'static str {
        if self.teams.len() > 1 { "Lockout" } else { "Blackout" }
    }

    pub fn team_of(&self, player: u128) -> Option<usize> {
        self.teams
            .iter()
            .position(|t| t.players.iter().any(|(u, _)| *u == player))
    }

    fn player_name(&self, player: u128) -> String {
        self.teams
            .iter()
            .flat_map(|t| &t.players)
            .find(|(u, _)| *u == player)
            .map_or_else(String::new, |(_, n)| n.clone())
    }

    /// The other team, in a match of two.
    fn opponent(&self, team: usize) -> Option<usize> {
        (self.teams.len() == 2).then_some(1 - team)
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
        for i in 0..self.board.len() {
            if let Some(tooltip) = self.tooltip(i, self.team_of(player)) {
                out.push(Payload::UpdateTooltip {
                    goal: self.board[i].id.clone(),
                    tooltip,
                });
            }
        }
        out
    }

    fn amount_of(trigger: &Trigger) -> Option<(f64, &'static str)> {
        Some(match trigger {
            Trigger::UniqueAdvancements { amount } => (*amount as f64, "Advancements"),
            Trigger::KillUniqueHostile { amount } => (*amount as f64, "Unique Hostile Mobs Killed"),
            Trigger::BreedUnique { amount } => (*amount as f64, "Unique Animals Bred"),
            Trigger::EatUnique { amount } => (*amount as f64, "Unique Food types"),
            Trigger::UniqueBiomes { amount } => (*amount as f64, "Biomes visited"),
            Trigger::CraftUnique { amount } => (*amount as f64, "Unique items crafted"),
            Trigger::DieUniqueWays { amount } => (*amount as f64, "Unique deaths"),
            Trigger::MineUnique { amount, .. } => (*amount as f64, "Types mined"),
            Trigger::KillAll { entities } => (entities.len() as f64, "Mobs killed"),
            Trigger::EatAll { items } => (items.len() as f64, "Eaten"),
            Trigger::BiomesAll { biomes } => (biomes.len() as f64, "Biomes visited"),
            Trigger::KillCount { amount, .. } => (*amount as f64, "Killed"),
            Trigger::TakeDamage { amount } => (*amount as f64, "Damage taken"),
            Trigger::DealDamage { amount } => (*amount as f64, "Damage dealt"),
            _ => return None,
        })
    }

    /// The board tooltip's progress lines (`UpdateTooltipPayload`): a team's own for its
    /// players, every team's count for spectators (`team` None).
    pub fn tooltip(&self, square: usize, team: Option<usize>) -> Option<String> {
        let (amount, label) = Self::amount_of(&self.board[square].trigger)?;
        let mut lines = vec![" ".to_owned()];
        let value = |t: usize| {
            self.progress.get(&(square, t)).map_or(0.0, |p| {
                if p.seen.is_empty() {
                    p.count
                } else {
                    p.seen.len() as f64
                }
            })
        };
        match team {
            Some(t) => {
                lines.push(format!("{label}: {}/{}", value(t).floor(), amount));
                if let Some(p) = self.progress.get(&(square, t)) {
                    let names: Vec<String> = p.seen.iter().map(|s| pretty(s)).collect();
                    for chunk in names.chunks(4) {
                        lines.push(chunk.join(", "));
                    }
                }
            }
            None => {
                for (i, t) in self.teams.iter().enumerate() {
                    lines.push(format!("{}: {}/{}", t.name, value(i).floor(), amount));
                }
            }
        }
        lines.push(" ".to_owned());
        Some(lines.join("\n"))
    }
}

/// `minecraft:zombie_villager` → `Zombie Villager`.
fn pretty(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    let path = path.rsplit('/').next().unwrap_or(path);
    path.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn holds(stacks: &[Option<Held>], item: &str) -> u32 {
    stacks
        .iter()
        .flatten()
        .filter(|h| h.item == item)
        .map(|h| h.count)
        .sum()
}

fn worn(stacks: &[Option<Held>]) -> impl Iterator<Item = &Held> {
    stacks.iter().skip(36).take(4).flatten()
}

fn armor(stacks: &[Option<Held>]) -> Vec<&str> {
    worn(stacks).map(|h| h.item.as_str()).collect()
}

/// `minecraft:iron_helmet` → `iron`: the armour's material, for "different materials".
fn armor_material(item: &str) -> Option<&str> {
    let path = item.rsplit(':').next()?;
    let (material, piece) = path.rsplit_once('_')?;
    ["helmet", "chestplate", "leggings", "boots"]
        .contains(&piece)
        .then_some(material)
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

    /// `team` takes square `square`. `online` are the players on the server.
    pub fn complete(&mut self, square: usize, team: usize, message: String, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = &mut self.game else { return };
        if !m.started || !m.running || m.board[square].completed_by.is_some() {
            return;
        }
        m.board[square].completed_by = Some(team);
        m.teams[team].points += 1;
        let id = m.board[square].id.clone();
        let members: Vec<u128> = m.teams[team].players.iter().map(|(u, _)| *u).collect();
        let others: Vec<u128> = m
            .teams
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != team)
            .flat_map(|(_, t)| t.players.iter().map(|(u, _)| *u))
            .collect();
        self.announce(team, &message, online, out);
        out.push(Action::SendAll {
            payload: Payload::CompleteTask {
                goal: id,
                team: team as i32,
            },
        });
        for p in members {
            out.push(Action::Sound {
                player: p,
                sound: "minecraft:block.note_block.chime",
                pitch: 1.0,
            });
        }
        for p in others {
            out.push(Action::Sound {
                player: p,
                sound: "minecraft:entity.guardian.death",
                pitch: 1.0,
            });
        }
        self.evaluate_winner(team, out);
    }

    /// Takes a "have the most" square from whoever held it.
    fn clear(&mut self, square: usize, out: &mut Vec<Action>) {
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
            out.push(Action::Broadcast {
                text: format!("{} wins.", m.teams[w].name),
                color: 0xFFFFFF,
            });
            winners.push(w as i32);
            m.running = false;
        } else if m.remaining() == 0 && m.teams.len() > 1 {
            let best = m.teams.iter().map(|t| t.points).max().unwrap_or(0);
            let tied: Vec<usize> = (0..m.teams.len()).filter(|&t| m.teams[t].points == best).collect();
            let names: Vec<&str> = tied.iter().map(|&t| m.teams[t].name.as_str()).collect();
            out.push(Action::Broadcast {
                text: format!("It's a tie! {} win.", names.join(", ")),
                color: 0xFFFFFF,
            });
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

    /// One server tick: the countdown, the timer, and the "have the most" squares.
    /// `xp` holds each online player's level, `crafted`'s count comes from the progress.
    pub fn tick(&mut self, online: &[u128], xp: &HashMap<u128, i32>) -> Vec<Action> {
        let mut out = Vec::new();
        self.server_ticks += 1;
        let Some(m) = &mut self.game else { return out };
        if !m.running {
            return out;
        }
        m.ticks += 1;
        if !m.started {
            if [-60, -40, -20].contains(&m.ticks) {
                out.push(Action::Broadcast {
                    text: format!("Starting in {}...", -m.ticks / 20),
                    color: 0xFFFFFF,
                });
            }
            if m.ticks >= 0 {
                m.started = true;
                out.push(Action::SendAll {
                    payload: Payload::Start,
                });
                for t in &m.teams {
                    for (p, _) in &t.players {
                        out.push(Action::GameMode { player: *p, mode: 0 });
                    }
                }
                out.push(Action::Broadcast {
                    text: format!("{} has begun.", m.mode_name()),
                    color: 0xFFFFFF,
                });
            }
        }
        if m.ticks % 20 == 0 {
            out.push(Action::SendAll {
                payload: Payload::UpdateTimer { ticks: m.ticks },
            });
            if m.started {
                self.update_most(online, xp, &mut out);
            }
        }
        out
    }

    fn update_most(&mut self, online: &[u128], xp: &HashMap<u128, i32>, out: &mut Vec<Action>) {
        let Some(m) = &self.game else { return };
        let mut changes = Vec::new();
        for (i, sq) in m.board.iter().enumerate() {
            let score: Vec<f64> = match sq.trigger {
                Trigger::MostXpLevels => m
                    .teams
                    .iter()
                    .map(|t| {
                        t.players
                            .iter()
                            .filter_map(|(u, _)| xp.get(u))
                            .copied()
                            .max()
                            .unwrap_or(0) as f64
                    })
                    .collect(),
                Trigger::MostUniqueCrafts => (0..m.teams.len())
                    .map(|t| m.progress.get(&(i, t)).map_or(0, |p| p.seen.len()) as f64)
                    .collect(),
                _ => continue,
            };
            let best = score.iter().cloned().fold(0.0, f64::max);
            let leaders: Vec<usize> = (0..score.len()).filter(|&t| score[t] == best).collect();
            let leader = (best > 0.0 && leaders.len() == 1).then(|| leaders[0]);
            if leader != sq.completed_by {
                changes.push((i, leader));
            }
        }
        for (i, leader) in changes {
            self.clear(i, out);
            if let Some(t) = leader {
                let name = self
                    .game
                    .as_ref()
                    .map(|m| format!("{} now has {}.", m.teams[t].name, m.board[i].name))
                    .unwrap_or_default();
                self.complete(i, t, name, online, out);
            }
        }
    }

    /// The goals `event` completes or moves along.
    pub fn handle(&mut self, event: Event, online: &[u128]) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(m) = &mut self.game else { return out };
        if !m.started || !m.running {
            return out;
        }
        let Some(actor) = event.player() else { return out };
        let actor_team = m.team_of(actor);
        let mut done: Vec<(usize, usize, String)> = Vec::new();
        let mut progressed: Vec<(usize, usize)> = Vec::new();
        let n = m.board.len();
        for i in 0..n {
            if m.board[i].completed_by.is_some() {
                continue;
            }
            let trigger = m.board[i].trigger.clone();
            // The team this event is about: the actor's, or the victim's for damage.
            let team = match &event {
                Event::Damage { victim, attacker, .. } => match trigger {
                    Trigger::DealDamage { .. } | Trigger::ShootOpponentWithArrow => attacker.and_then(|a| m.team_of(a)),
                    _ => victim.and_then(|v| m.team_of(v)),
                },
                _ => actor_team,
            };
            let Some(team) = team else { continue };
            let progress = m.progress.entry((i, team)).or_default();
            let outcome = evaluate(&trigger, &event, progress, m.teams.len());
            let outcome = match (&trigger, &event) {
                // An arrow from one team into the other.
                (
                    Trigger::ShootOpponentWithArrow,
                    Event::Damage {
                        victim: Some(v),
                        damage_type,
                        ..
                    },
                ) => {
                    if damage_type == "minecraft:arrow" && m.team_of(*v).is_some_and(|vt| vt != team) {
                        Outcome::Complete
                    } else {
                        Outcome::None
                    }
                }
                (
                    Trigger::KillOtherPlayer,
                    Event::Death {
                        killer_player: Some(k),
                        player,
                        ..
                    },
                ) => match (m.team_of(*k), m.team_of(*player)) {
                    (Some(kt), Some(pt)) if kt != pt => {
                        done.push((i, kt, format!("{} completed {}.", m.player_name(*k), m.board[i].name)));
                        Outcome::None
                    }
                    _ => Outcome::None,
                },
                (Trigger::PickUpOpponentItem, Event::PickedUp { thrower: Some(t), .. }) => {
                    if m.team_of(*t).is_some_and(|tt| tt != team) {
                        Outcome::Complete
                    } else {
                        Outcome::None
                    }
                }
                _ => outcome,
            };
            match outcome {
                Outcome::Complete => {
                    let who = match &event {
                        Event::Damage { attacker: Some(a), .. }
                            if matches!(trigger, Trigger::DealDamage { .. } | Trigger::ShootOpponentWithArrow) =>
                        {
                            *a
                        }
                        _ => actor,
                    };
                    let name = if matches!(event, Event::Damage { .. })
                        && !matches!(trigger, Trigger::DealDamage { .. } | Trigger::ShootOpponentWithArrow)
                    {
                        m.teams[team].name.clone()
                    } else {
                        m.player_name(who)
                    };
                    done.push((i, team, format!("{name} completed {}.", m.board[i].name)));
                }
                Outcome::CompleteForOpponent => {
                    if let Some(o) = m.opponent(team) {
                        done.push((
                            i,
                            o,
                            format!("{} {}.", m.player_name(actor), opponent_message(&trigger)),
                        ));
                    }
                }
                Outcome::Progressed => progressed.push((i, team)),
                Outcome::None => {}
            }
        }
        for (i, team) in progressed {
            self.send_tooltips(i, team, online, &mut out);
        }
        for (i, team, message) in done {
            self.complete(i, team, message, online, &mut out);
        }
        out
    }

    fn send_tooltips(&self, square: usize, team: usize, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = &self.game else { return };
        let id = m.board[square].id.clone();
        if let Some(tooltip) = m.tooltip(square, Some(team)) {
            for (p, _) in &m.teams[team].players {
                out.push(Action::Send {
                    player: *p,
                    payload: Payload::UpdateTooltip {
                        goal: id.clone(),
                        tooltip: tooltip.clone(),
                    },
                });
            }
        }
        if let Some(tooltip) = m.tooltip(square, None) {
            for s in self.spectators(online) {
                out.push(Action::Send {
                    player: s,
                    payload: Payload::UpdateTooltip {
                        goal: id.clone(),
                        tooltip: tooltip.clone(),
                    },
                });
            }
        }
    }

    /// Starts a match: resets the players (by console commands, run on the next tick),
    /// sends the board and starts the countdown.
    pub fn start(
        &mut self,
        teams: Vec<Team>,
        goals: Vec<(String, String)>,
        online: &[(u128, String)],
        sprint: &HashMap<u128, i32>,
    ) -> Vec<Action> {
        let mut out = Vec::new();
        let start = self.settings.start_seconds.max(5) * 20;
        let game = Match::new(teams, goals, start);
        self.last_hurt_by.clear();
        self.sprint_base = sprint.clone();
        for (uuid, name) in online {
            for c in [
                format!("clear {name}"),
                format!("effect clear {name}"),
                format!("xp set {name} 0 levels"),
                format!("xp set {name} 0 points"),
                format!("advancement revoke {name} everything"),
                format!("effect give {name} minecraft:instant_health 1 20 true"),
                format!("effect give {name} minecraft:saturation 1 20 true"),
                format!("extinguish {name}"),
            ] {
                out.push(Action::Command(c));
            }
            if game.team_of(*uuid).is_some() {
                out.push(Action::GameMode { player: *uuid, mode: 2 });
            } else {
                out.push(Action::GameMode { player: *uuid, mode: 3 });
                out.push(tell(*uuid, "You are spectating this match.", GRAY));
            }
        }
        out.push(Action::Command("time set 0".into()));
        let ids: Vec<u128> = online.iter().map(|(u, _)| *u).collect();
        for p in &ids {
            for payload in game.join_payloads(*p) {
                out.push(Action::Send { player: *p, payload });
            }
        }
        self.game = Some(game);
        out
    }
}

/// The line an opponent's slip is announced with.
fn opponent_message(trigger: &Trigger) -> String {
    match trigger {
        Trigger::Opponent(OpponentEvent::Obtains(item)) => format!("obtained {}", pretty(item)),
        Trigger::Opponent(OpponentEvent::Dies { .. }) => "died".into(),
        Trigger::Opponent(OpponentEvent::CatchesFire) => "caught on fire".into(),
        Trigger::Opponent(OpponentEvent::HitBy(p)) => format!("was hit by {}", pretty(p)),
        Trigger::Opponent(OpponentEvent::TakesDamage(_)) => "took too much damage".into(),
        Trigger::Opponent(OpponentEvent::FallDamage) => "took fall damage".into(),
        Trigger::Opponent(OpponentEvent::TouchesWater) => "touched water".into(),
        Trigger::Opponent(OpponentEvent::EatsFood) => "ate food".into(),
        _ => "slipped".into(),
    }
}

/// Counts `key` once, completing when `amount` are seen.
fn see(progress: &mut Progress, key: &str, amount: usize) -> Outcome {
    if progress.seen.insert(key.to_owned()) {
        if progress.seen.len() >= amount {
            Outcome::Complete
        } else {
            Outcome::Progressed
        }
    } else {
        Outcome::None
    }
}

fn add(progress: &mut Progress, by: f64, amount: f64) -> Outcome {
    progress.count += by;
    if progress.count >= amount {
        Outcome::Complete
    } else {
        Outcome::Progressed
    }
}

fn yes(b: bool) -> Outcome {
    if b { Outcome::Complete } else { Outcome::None }
}

/// What `event` does to a goal with `trigger`, for the team whose progress this is.
fn evaluate(trigger: &Trigger, event: &Event, progress: &mut Progress, teams: usize) -> Outcome {
    use Event as E;
    use Trigger as T;
    match (trigger, event) {
        (T::ObtainAll { items, amount }, E::Inventory { stacks, .. }) => {
            yes(!items.is_empty() && items.iter().all(|i| holds(stacks, i) >= *amount))
        }
        (T::ObtainSome { items, amount }, E::Inventory { stacks, .. }) => {
            yes(items.iter().filter(|i| holds(stacks, i) > 0).count() >= *amount as usize)
        }
        (T::ObtainAny { items, amount }, E::Inventory { stacks, .. }) => {
            yes(items.iter().any(|i| holds(stacks, i) >= *amount))
        }
        (T::StackOf64, E::Inventory { stacks, .. }) => yes(stacks.iter().take(36).flatten().any(|h| h.count >= 64)),
        (T::FillInventoryUnique, E::Inventory { stacks, .. }) => {
            let main: Vec<&str> = stacks.iter().take(36).flatten().map(|h| h.item.as_str()).collect();
            let unique: BTreeSet<&str> = main.iter().copied().collect();
            yes(main.len() == 36 && unique.len() == 36)
        }
        (T::ObtainPotion { potions }, E::Inventory { stacks, .. }) => yes(stacks
            .iter()
            .flatten()
            .any(|h| h.potion.as_ref().is_some_and(|p| potions.contains(p)))),
        (T::WearAll { items }, E::Inventory { stacks, .. }) => {
            let worn = armor(stacks);
            yes(items.iter().all(|i| worn.contains(&i.as_str())))
        }
        (T::WearAny { items }, E::Inventory { stacks, .. }) => {
            yes(armor(stacks).iter().any(|w| items.iter().any(|i| i == w)))
        }
        (T::WearUniqueMaterials, E::Inventory { stacks, .. }) => {
            let worn = armor(stacks);
            let materials: BTreeSet<&str> = worn.iter().filter_map(|w| armor_material(w)).collect();
            yes(worn.len() == 4 && materials.len() == 4)
        }
        (T::WearDyed { item, rgb }, E::Inventory { stacks, .. }) => {
            yes(worn(stacks).any(|h| &h.item == item && h.dyed == Some(*rgb)))
        }
        (T::WearUniqueDyedLeather, E::Inventory { stacks, .. }) => {
            let leather = ["helmet", "chestplate", "leggings", "boots"].map(|p| format!("minecraft:leather_{p}"));
            let pieces: Vec<&Held> = worn(stacks).filter(|h| leather.contains(&h.item)).collect();
            let colors: BTreeSet<i32> = pieces
                .iter()
                .filter_map(|h| h.dyed)
                .filter(|c| *c != LEATHER_RGB)
                .collect();
            yes(pieces.len() == 4 && colors.len() == 4)
        }
        (T::Opponent(OpponentEvent::Obtains(item)), E::Inventory { stacks, .. }) if teams == 2 => {
            if holds(stacks, item) > 0 {
                Outcome::CompleteForOpponent
            } else {
                Outcome::None
            }
        }
        (T::Advancement { ids }, E::Advancement { id, .. }) => yes(ids.contains(id)),
        (T::UniqueAdvancements { amount }, E::Advancement { id, .. }) => {
            if id.contains("recipes/") {
                Outcome::None
            } else {
                see(progress, id, *amount)
            }
        }
        (
            T::Kill {
                entities,
                color,
                dimension,
            },
            E::Kill {
                kind,
                color: c,
                dimension: d,
                ..
            },
        ) => yes(entities.contains(kind) && (color.is_none() || color == c) && (dimension.is_none() || dimension == d)),
        (T::KillCount { entities, amount }, E::Kill { kind, .. }) => {
            if entities.as_ref().is_none_or(|e| e.contains(kind)) {
                add(progress, 1.0, *amount as f64)
            } else {
                Outcome::None
            }
        }
        (T::KillAll { entities }, E::Kill { kind, .. }) if entities.contains(kind) => {
            see(progress, kind, entities.len())
        }
        (T::KillUniqueHostile { amount }, E::Kill { kind, .. }) if HOSTILE.contains(&kind.as_str()) => {
            see(progress, kind, *amount)
        }
        (T::DieTo { damage_types }, E::Death { damage_type, .. }) => yes(damage_types.contains(damage_type)),
        (
            T::DieToEntity { entities },
            E::Death {
                killer_kind: Some(k), ..
            },
        ) => yes(entities.contains(k)),
        (
            T::DieByMessage { key },
            E::Death {
                message_key: Some(k), ..
            },
        ) => yes(k == key),
        (T::DieUniqueWays { amount }, E::Death { damage_type, .. }) => see(progress, damage_type, *amount),
        (T::Opponent(OpponentEvent::Dies { times }), E::Death { .. }) if teams == 2 => {
            progress.count += 1.0;
            if progress.count >= *times as f64 {
                Outcome::CompleteForOpponent
            } else {
                Outcome::None
            }
        }
        (T::Breed { entities }, E::Breed { kind, .. }) => yes(entities.contains(kind)),
        (T::BreedUnique { amount }, E::Breed { kind, .. }) => see(progress, kind, *amount),
        (T::Tame { entities }, E::Tame { kind, .. }) => yes(entities.contains(kind)),
        (T::Effect { effects }, E::EffectAdded { effect, .. }) => yes(effects.contains(effect)),
        (T::Effect { effects }, E::Effects { effects: now, .. }) => yes(now.iter().any(|e| effects.contains(e))),
        (T::EffectsAtOnce { amount }, E::Effects { effects, .. }) => yes(effects.len() >= *amount),
        (T::Consume { items }, E::Consumed { item, .. }) => yes(items.contains(item)),
        (T::DrinkPotion { potions }, E::Consumed { item, potion, .. }) => {
            yes(item == "minecraft:potion" && potion.as_ref().is_some_and(|p| potions.contains(p)))
        }
        (T::EatUnique { amount }, E::Consumed { item, .. })
            if !item.ends_with("potion") && item != "minecraft:milk_bucket" && item != "minecraft:honey_bottle" =>
        {
            see(progress, item, *amount)
        }
        (T::EatAll { items }, E::Consumed { item, .. }) if items.contains(item) => see(progress, item, items.len()),
        (T::Opponent(OpponentEvent::EatsFood), E::Food { from, to, .. }) if teams == 2 && to > from => {
            Outcome::CompleteForOpponent
        }
        (T::Stat { stats }, E::Stat { stat, .. }) => yes(stats.contains(stat)),
        (T::SprintCm { cm }, E::SprintCm { cm: now, .. }) => yes(now >= cm),
        (T::Mine { blocks }, E::Mined { block, .. }) => yes(blocks.contains(block)),
        (T::MineUnique { blocks, amount }, E::Mined { block, .. }) if blocks.contains(block) => {
            see(progress, block, *amount)
        }
        (T::Dimension { dimension }, E::Dimension { dimension: d, .. }) => yes(d == dimension),
        (T::Dimension { dimension }, E::Position { dimension: d, .. }) => yes(d == dimension),
        (T::Biome { biomes }, E::Position { biome: Some(b), .. }) => yes(biomes.contains(b)),
        (T::BiomesAll { biomes }, E::Position { biome: Some(b), .. }) if biomes.contains(b) => {
            see(progress, b, biomes.len())
        }
        (T::UniqueBiomes { amount }, E::Position { biome: Some(b), .. }) => see(progress, b, *amount),
        (T::Height { dimension, min_y }, E::Position { dimension: d, y, .. }) => yes(d == dimension && y >= min_y),
        (
            T::StandOn { dimension, block },
            E::Position {
                dimension: d,
                below: Some(b),
                ..
            },
        ) => yes(d == dimension && b == block),
        (T::Opponent(OpponentEvent::TouchesWater), E::Position { in_water: true, .. }) if teams == 2 => {
            Outcome::CompleteForOpponent
        }
        (T::Opponent(OpponentEvent::CatchesFire), E::Position { on_fire: true, .. }) if teams == 2 => {
            Outcome::CompleteForOpponent
        }
        (T::XpLevel { level }, E::XpLevel { level: l, .. }) => yes(l >= level),
        (T::CraftUnique { amount }, E::Crafted { item, .. }) => see(progress, item, *amount),
        (T::MostUniqueCrafts, E::Crafted { item, .. }) => {
            progress.seen.insert(item.clone());
            Outcome::None
        }
        (T::CraftItemSuffix { suffix }, E::Crafted { item, .. }) => yes(item.ends_with(suffix.as_str())),
        (T::Ride { entities }, E::Ride { vehicle, .. }) => yes(entities.contains(vehicle)),
        (T::Name { entity, name }, E::Named { kind, name: n, .. }) => yes(kind == entity && n == name),
        (T::BreakItem { matcher }, E::ItemBroken { item, .. }) => yes(matcher.matches(item)),
        (T::PlaceBlockSuffix { suffix }, E::PlacedBlock { item, .. }) => yes(item.ends_with(suffix.as_str())),
        (T::PlaceHanging { kind }, E::PlacedHanging { kind: k, .. }) => yes(k == kind),
        (T::UseItem { item }, E::UsedItem { item: i, .. }) => yes(i == item),
        (T::MilkAwayEffect, E::MilkAway { .. }) => Outcome::Complete,
        (T::EmptyHunger, E::Food { to, .. }) => yes(*to == 0),
        (T::Enrage { entity }, E::Enraged { kind, .. }) => yes(kind == entity),
        (T::ShieldDisabled, E::ShieldDisabled { .. })
        | (T::ItemFrameInItemFrame, E::ItemFrameInFrame { .. })
        | (T::WolfArmor, E::WolfArmor { .. })
        | (T::Stonecutter, E::Stonecutter { .. })
        | (T::Loom, E::Loom { .. })
        | (T::Anvil, E::Anvil { .. }) => Outcome::Complete,
        (
            T::TakeDamage { amount },
            E::Damage {
                victim: Some(_),
                amount: a,
                ..
            },
        ) => add(progress, *a as f64, *amount as f64),
        (
            T::DealDamage { amount },
            E::Damage {
                attacker: Some(_),
                amount: a,
                ..
            },
        ) => add(progress, *a as f64, *amount as f64),
        (
            T::Opponent(OpponentEvent::TakesDamage(limit)),
            E::Damage {
                victim: Some(_),
                amount,
                ..
            },
        ) if teams == 2 => {
            progress.count += *amount as f64;
            if progress.count >= *limit as f64 {
                Outcome::CompleteForOpponent
            } else {
                Outcome::None
            }
        }
        (
            T::Opponent(OpponentEvent::FallDamage),
            E::Damage {
                victim: Some(_),
                damage_type,
                ..
            },
        ) if teams == 2 && damage_type == "minecraft:fall" => Outcome::CompleteForOpponent,
        (
            T::Opponent(OpponentEvent::CatchesFire),
            E::Damage {
                victim: Some(_),
                damage_type,
                ..
            },
        ) if teams == 2 && (damage_type == "minecraft:on_fire" || damage_type == "minecraft:in_fire") => {
            Outcome::CompleteForOpponent
        }
        (T::Opponent(OpponentEvent::HitBy(kind)), E::HitBy { projectile, .. }) if teams == 2 && projectile == kind => {
            Outcome::CompleteForOpponent
        }
        _ => Outcome::None,
    }
}

/// Teams from vanilla team names and the members `/team list` gave for each. `color` is
/// the team's colour name when the server's teams have one.
pub fn teams_from(
    listed: Vec<(String, Option<String>, Vec<String>)>,
    online: &[(u128, String)],
) -> Result<Vec<Team>, String> {
    let mut teams = Vec::new();
    for (i, (name, color, members)) in listed.into_iter().enumerate() {
        let players: Vec<(u128, String)> = members
            .iter()
            .filter_map(|m| online.iter().find(|(_, n)| n.eq_ignore_ascii_case(m)).cloned())
            .collect();
        if players.is_empty() {
            return Err(format!("Team {name} has no players online."));
        }
        let color = color
            .and_then(|c| TEAM_COLORS.iter().position(|(n, _)| *n == c))
            .unwrap_or(COLOR_ORDER[i % COLOR_ORDER.len()]);
        teams.push(Team {
            name,
            color,
            players,
            points: 0,
        });
    }
    Ok(teams)
}

/// One team per player (`/lockout players`), or one team of them all (`/blackout players`).
pub fn teams_of_players(names: &[String], online: &[(u128, String)], blackout: bool) -> Result<Vec<Team>, String> {
    let mut players = Vec::new();
    for name in names {
        match online.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)) {
            Some(p) => players.push(p.clone()),
            None => return Err(format!("Player {name} is invalid.")),
        }
    }
    if blackout {
        if players.is_empty() {
            return Err("Not enough players listed.".into());
        }
        let color = COLOR_ORDER[0];
        let name = wire::color_display(TEAM_COLORS[color].0);
        return Ok(vec![Team {
            name,
            color,
            players,
            points: 0,
        }]);
    }
    if players.len() < 2 {
        return Err("Not enough players listed. Make sure you separate player names with spaces.".into());
    }
    if players.len() > 16 {
        return Err("Too many players listed.".into());
    }
    Ok(players
        .into_iter()
        .enumerate()
        .map(|(i, p)| Team {
            name: p.1.clone(),
            color: COLOR_ORDER[i],
            players: vec![p],
            points: 0,
        })
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

    const A: u128 = 1;
    const B: u128 = 2;

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
    fn unique_goals_count_each_kind_once_and_send_progress() {
        let mut s = running(&["KILL_7_UNIQUE_HOSTILE_MOBS", "ENTER_END", "ENTER_NETHER"]);
        for kind in [
            "zombie", "zombie", "skeleton", "creeper", "spider", "witch", "pig", "enderman",
        ] {
            let out = s.handle(
                Event::Kill {
                    player: A,
                    kind: format!("minecraft:{kind}"),
                    color: None,
                    dimension: None,
                },
                &[A, B],
            );
            assert!(completed(&out).is_empty());
        }
        let out = s.handle(
            Event::Kill {
                player: A,
                kind: "minecraft:blaze".into(),
                color: None,
                dimension: None,
            },
            &[A, B],
        );
        assert_eq!(completed(&out), vec![("KILL_7_UNIQUE_HOSTILE_MOBS".into(), 0)]);
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
        let out = s.handle(
            Event::Inventory {
                player: A,
                stacks: stacks.clone(),
            },
            &[A, B],
        );
        assert_eq!(
            completed(&out),
            vec![("WEAR_LEATHER_ARMOR".into(), 0), ("OBTAIN_64_ARROWS".into(), 0)]
        );
        stacks[4] = Some(Held::new("minecraft:wooden_shovel", 1));
        let out = s.handle(Event::Inventory { player: B, stacks }, &[A, B]);
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
        assert!(
            completed(&s.handle(
                Event::Inventory {
                    player: A,
                    stacks: stacks.clone()
                },
                &[A, B]
            ))
            .is_empty()
        );
        stacks[36] = Some(Held {
            dyed: Some(0xB02E26),
            ..Held::new("minecraft:leather_boots", 1)
        });
        let out = s.handle(Event::Inventory { player: A, stacks }, &[A, B]);
        assert_eq!(completed(&out), vec![("WEAR_COLORED_LEATHER_ARMOR_PIECE".into(), 0)]);
        let golem = |d: &str| Event::Kill {
            player: B,
            kind: "minecraft:snow_golem".into(),
            color: None,
            dimension: Some(d.into()),
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
    fn opponent_goals_go_to_the_other_team() {
        let mut s = running(&["OPPONENT_OBTAINS_OBSIDIAN", "ENTER_END", "KILL_WITCH"]);
        let mut stacks = vec![None; 41];
        stacks[0] = Some(Held::new("minecraft:obsidian", 1));
        let out = s.handle(Event::Inventory { player: A, stacks }, &[A, B]);
        assert_eq!(completed(&out), vec![("OPPONENT_OBTAINS_OBSIDIAN".into(), 1)]);
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
        for _ in 0..100 {
            let out = s.tick(&[A, B], &HashMap::new());
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
    fn most_xp_levels_moves_with_the_leader() {
        let mut s = running(&[
            "HAVE_MORE_XP_LEVELS",
            "ENTER_END",
            "KILL_WITCH",
            "ENTER_NETHER",
            "KILL_GHAST",
        ]);
        s.game.as_mut().unwrap().ticks = 19;
        s.tick(&[A, B], &HashMap::from([(A, 5), (B, 3)]));
        assert_eq!(s.game.as_ref().unwrap().board[0].completed_by, Some(0));
        s.game.as_mut().unwrap().ticks = 39;
        s.tick(&[A, B], &HashMap::from([(A, 5), (B, 9)]));
        assert_eq!(s.game.as_ref().unwrap().board[0].completed_by, Some(1));
        let points: Vec<u32> = s.game.as_ref().unwrap().teams.iter().map(|t| t.points).collect();
        assert_eq!(points, vec![0, 1]);
    }
}
