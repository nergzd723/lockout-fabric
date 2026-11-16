//! Drafts (`DraftSession`, `DraftScouting`): before a match, teams build the board by picking
//! goals in turns, in snake order (1, 2, ..., n, n, ..., 2, 1, 1, 2, ...). Each turn offers a
//! few goals; when the timer runs out one of them is picked at random. Before the first pick,
//! drafting players may scout: frozen in place, jumping toggles a bird's-eye view 30 blocks
//! up, and a spyglass zooms.
//!
//! As in `server.rs`, nothing here calls the host: it answers with [`Action`]s.
use crate::board::{self, Rng, World};
use crate::goals::{self, DATA_NONE};
use crate::server::{Action, Span, Team};
use crate::wire::{DraftStatus, DraftTeam, Payload, REROLL, TEAM_COLORS};
use std::collections::{HashMap, HashSet, VecDeque};

const GRAY: u32 = 0xAAAAAA;
const WHITE: u32 = 0xFFFFFF;
const AQUA: u32 = 0x55FFFF;
const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;

const BIRDS_EYE_HEIGHT: f64 = 30.0;
const MAX_DRIFT: f64 = 0.5;
/// Vanilla's flying speed (`Abilities.getFlyingSpeed`), given back after scouting.
pub const DEFAULT_FLYING_SPEED: f32 = 0.05;
/// The attribute modifier that freezes scouting players.
pub const SCOUTING_FREEZE: &str = "lockout:scouting";

/// `/SetDraftSettings` and `/SetScoutingTime`, defaults from `config/lockout.json`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DraftSettings {
    pub offer_size: usize,
    pub pick_seconds: i32,
    pub rerolls: i32,
    pub scouting_seconds: i32,
}

impl Default for DraftSettings {
    fn default() -> Self {
        Self {
            offer_size: 2,
            pick_seconds: 10,
            rerolls: 1,
            scouting_seconds: 15,
        }
    }
}

/// A drafting player as the scouting needs them this tick.
#[derive(Debug, Clone)]
pub struct Scout {
    pub uuid: u128,
    pub dimension: String,
    pub position: [f64; 3],
    pub jumping: bool,
    pub has_spyglass: bool,
}

#[derive(Debug)]
pub struct Scouting {
    players: Vec<u128>,
    ground: HashMap<u128, (String, [f64; 3])>,
    birds_eye: HashSet<u128>,
    was_jumping: HashSet<u128>,
    ticks_left: i32,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl Scouting {
    fn new(teams: &[Team], seconds: i32) -> Self {
        Self {
            players: teams.iter().flat_map(|t| t.players.iter().map(|(u, _)| *u)).collect(),
            ground: HashMap::new(),
            birds_eye: HashSet::new(),
            was_jumping: HashSet::new(),
            ticks_left: seconds * 20,
        }
    }

    pub fn is_scouting(&self, player: u128) -> bool {
        self.ground.contains_key(&player)
    }

    fn anchor(&self, player: u128) -> Option<(String, [f64; 3])> {
        let (dim, ground) = self.ground.get(&player)?;
        let up = if self.birds_eye.contains(&player) {
            BIRDS_EYE_HEIGHT
        } else {
            0.0
        };
        Some((dim.clone(), [ground[0], ground[1] + up, ground[2]]))
    }

    fn freeze(&mut self, scout: &Scout, out: &mut Vec<Action>) {
        self.ground
            .insert(scout.uuid, (scout.dimension.clone(), scout.position));
        out.push(Action::Freeze {
            player: scout.uuid,
            modifier: SCOUTING_FREEZE,
            on: true,
        });
        out.push(Action::FlySpeed {
            player: scout.uuid,
            speed: Some(0.0),
        });
        if !scout.has_spyglass {
            out.push(Action::Give {
                player: scout.uuid,
                item: "minecraft:spyglass".into(),
            });
        }
    }

    fn release(&mut self, player: u128, out: &mut Vec<Action>) {
        if let Some((dim, pos)) = self.ground.get(&player) {
            out.push(Action::Teleport {
                player,
                dimension: dim.clone(),
                pos: *pos,
            });
        }
        out.push(Action::Freeze {
            player,
            modifier: SCOUTING_FREEZE,
            on: false,
        });
        out.push(Action::Flight {
            player,
            allow: false,
            flying: false,
        });
        out.push(Action::FlySpeed {
            player,
            speed: Some(DEFAULT_FLYING_SPEED),
        });
        out.push(Action::ClearItem {
            player,
            item: "minecraft:spyglass".into(),
        });
    }

    fn start(&mut self, online: &[Scout], out: &mut Vec<Action>) {
        out.push(Action::Broadcast(vec![Span::new(
            format!(
                "Scouting for {}s before the draft: jump for a bird's-eye view, right click the Spyglass to zoom.",
                self.ticks_left / 20
            ),
            AQUA,
        )]));
        let players = self.players.clone();
        for s in online.iter().filter(|s| players.contains(&s.uuid)) {
            self.freeze(s, out);
        }
    }

    /// One tick; true once scouting is over.
    fn tick(&mut self, online: &[Scout], out: &mut Vec<Action>) -> bool {
        self.ticks_left -= 1;
        let players = self.players.clone();
        for s in online.iter().filter(|s| players.contains(&s.uuid)) {
            if !self.ground.contains_key(&s.uuid) {
                self.freeze(s, out); // joined during scouting
            }
            if s.jumping && !self.was_jumping.contains(&s.uuid) {
                if !self.birds_eye.remove(&s.uuid) {
                    self.birds_eye.insert(s.uuid);
                }
                // Hover while up high: flying with no flying speed means no falling and no moving.
                let up = self.birds_eye.contains(&s.uuid);
                out.push(Action::Flight {
                    player: s.uuid,
                    allow: up,
                    flying: up,
                });
                if let Some((dim, pos)) = self.anchor(s.uuid) {
                    out.push(Action::Teleport {
                        player: s.uuid,
                        dimension: dim,
                        pos,
                    });
                }
            }
            if s.jumping {
                self.was_jumping.insert(s.uuid);
            } else {
                self.was_jumping.remove(&s.uuid);
            }
            if let Some((dim, pos)) = self.anchor(s.uuid)
                && (dim != s.dimension || distance(pos, s.position) > MAX_DRIFT)
            {
                out.push(Action::Teleport {
                    player: s.uuid,
                    dimension: dim,
                    pos,
                });
            }
            if self.ticks_left % 20 == 0 {
                out.push(Action::ActionBar {
                    player: s.uuid,
                    spans: vec![
                        Span::new(format!("Scouting: {}s", self.ticks_left / 20), AQUA),
                        Span::new("  ·  Jump: bird's-eye view  ·  Right click: zoom", GRAY),
                    ],
                });
            }
        }
        if self.ticks_left <= 0 {
            self.end(out);
            return true;
        }
        false
    }

    /// Puts everyone back on the ground and gives back control. Safe to call more than once.
    fn end(&mut self, out: &mut Vec<Action>) {
        let players: Vec<u128> = self.ground.keys().copied().collect();
        for p in players {
            self.release(p, out);
        }
        self.ground.clear();
        self.birds_eye.clear();
    }
}

/// What a tick of the draft led to.
pub enum DraftOutcome {
    Running,
    /// The board is full: start the match with these teams and this board.
    Finished {
        teams: Vec<Team>,
        board: Vec<(String, String)>,
    },
    /// Out of goals to offer.
    Cancelled,
}

pub struct Draft {
    pub teams: Vec<Team>,
    board_size: usize,
    offer_size: usize,
    pick_ticks: i32,
    rerolls_left: Vec<i32>,
    /// Goals that can still be offered, in random order.
    pool: VecDeque<String>,
    picks: Vec<(String, String)>,
    pick_teams: Vec<usize>,
    offer: Vec<(String, String)>,
    ticks_left: i32,
    done: bool,
    scouting_seconds: i32,
    /// Set while players are scouting, before the first pick.
    pub scouting: Option<Scouting>,
    world: World,
    rng: Rng,
}

fn team_span(team: &Team) -> Span {
    Span::new(team.display_name(), TEAM_COLORS[team.color].1)
}

fn goal_name(id: &str, data: &str) -> String {
    goals::table().get(id).map_or_else(|| id.to_owned(), |g| g.name(data))
}

impl Draft {
    pub fn new(teams: Vec<Team>, board_size: usize, settings: DraftSettings, world: World, mut rng: Rng) -> Self {
        // Same filtering as random boards: located biomes and structures, team count, the random pool.
        let mut candidates: Vec<String> = goals::table()
            .goals
            .iter()
            .filter(|g| board::eligible(g, teams.len(), &world))
            .map(|g| g.id.clone())
            .collect();
        rng.shuffle(&mut candidates);
        Self {
            rerolls_left: vec![settings.rerolls; teams.len()],
            teams,
            board_size,
            offer_size: settings.offer_size.max(1),
            pick_ticks: settings.pick_seconds * 20,
            pool: candidates.into(),
            picks: Vec::new(),
            pick_teams: Vec::new(),
            offer: Vec::new(),
            ticks_left: 0,
            done: false,
            scouting_seconds: settings.scouting_seconds,
            scouting: None,
            world,
            rng,
        }
    }

    pub fn start(&mut self, online: &[Scout]) -> Vec<Action> {
        let mut out = Vec::new();
        if self.scouting_seconds > 0 {
            let mut s = Scouting::new(&self.teams, self.scouting_seconds);
            s.start(online, &mut out);
            self.scouting = Some(s);
        } else {
            self.begin_picking(&mut out);
        }
        out
    }

    fn begin_picking(&mut self, out: &mut Vec<Action>) {
        let n = self.board_size;
        if self.teams.len() == 1 {
            out.push(Action::Broadcast(vec![Span::new(
                format!("Drafting a {n}x{n} Blackout board."),
                WHITE,
            )]));
        } else {
            let mut spans = vec![Span::new(format!("Drafting a {n}x{n} board. Pick order: "), WHITE)];
            for (i, t) in self.teams.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::new(" → ", GRAY));
                }
                spans.push(team_span(t));
            }
            spans.push(Span::new(" (snake order).", GRAY));
            out.push(Action::Broadcast(spans));
        }
        self.new_offer();
        out.push(self.state(DraftStatus::Running));
    }

    /// The team picking now: forward on even rounds, backward on odd ones.
    pub fn current_team(&self) -> usize {
        let n = self.teams.len().max(1);
        let round = self.picks.len() / n;
        let position = self.picks.len() % n;
        if round.is_multiple_of(2) {
            position
        } else {
            n - 1 - position
        }
    }

    pub fn tick(&mut self, online: &[Scout]) -> (Vec<Action>, DraftOutcome) {
        let mut out = Vec::new();
        if self.done {
            return (out, DraftOutcome::Running);
        }
        if let Some(s) = &mut self.scouting {
            if s.tick(online, &mut out) {
                self.scouting = None;
                self.begin_picking(&mut out);
            }
            return (out, DraftOutcome::Running);
        }
        self.ticks_left -= 1;
        if self.offer.is_empty() {
            // Ran out of goals to offer (only possible with a tiny goal pool).
            out.extend(self.cancel());
            return (out, DraftOutcome::Cancelled);
        } else if self.ticks_left <= 0 {
            let i = self.rng.below(self.offer.len());
            if let Some(done) = self.pick(i, true, online, &mut out) {
                return (out, done);
            }
        } else if self.ticks_left % 20 == 0 {
            out.push(self.state(DraftStatus::Running));
        }
        (out, DraftOutcome::Running)
    }

    /// A drafting player's `DraftActionPayload`: an offer index, or [`REROLL`].
    pub fn action(&mut self, player: u128, action: i32, online: &[Scout]) -> (Vec<Action>, DraftOutcome) {
        let mut out = Vec::new();
        if self.done || self.scouting.is_some() {
            return (out, DraftOutcome::Running);
        }
        let team = self.current_team();
        if !self.teams[team].players.iter().any(|(u, _)| *u == player) {
            out.push(Action::Tell {
                player,
                spans: vec![Span::new("It's not your team's turn to pick.", RED)],
            });
            return (out, DraftOutcome::Running);
        }
        if action == REROLL {
            if self.rerolls_left[team] <= 0 {
                return (out, DraftOutcome::Running);
            }
            self.rerolls_left[team] -= 1;
            out.push(Action::Broadcast(vec![
                team_span(&self.teams[team]),
                Span::new(" rerolled.", GRAY),
            ]));
            self.pool.extend(self.offer.iter().map(|(id, _)| id.clone()));
            self.new_offer();
            out.push(self.state(DraftStatus::Running));
        } else if action >= 0
            && (action as usize) < self.offer.len()
            && let Some(done) = self.pick(action as usize, false, online, &mut out)
        {
            return (out, done);
        }
        (out, DraftOutcome::Running)
    }

    fn pick(&mut self, index: usize, timed_out: bool, online: &[Scout], out: &mut Vec<Action>) -> Option<DraftOutcome> {
        let team = self.current_team();
        let goal = self.offer.remove(index);
        let name = goal_name(&goal.0, &goal.1);
        self.picks.push(goal);
        self.pick_teams.push(team);
        // Goals that weren't picked can be offered again later.
        self.pool.extend(self.offer.drain(..).map(|(id, _)| id));
        out.push(Action::Broadcast(vec![
            team_span(&self.teams[team]),
            Span::new(
                if timed_out {
                    " ran out of time, got "
                } else {
                    " picked "
                },
                WHITE,
            ),
            Span::new(name, WHITE),
        ]));
        for s in online {
            out.push(Action::Sound {
                player: s.uuid,
                sound: "minecraft:block.note_block.pling",
                category: "master",
                volume: 0.6,
                pitch: if timed_out { 0.8 } else { 1.6 },
            });
        }
        if self.picks.len() == self.board_size * self.board_size {
            self.done = true;
            out.push(self.state(DraftStatus::Finished));
            out.push(Action::Broadcast(vec![Span::new("The board is drafted!", GREEN)]));
            return Some(DraftOutcome::Finished {
                teams: self.teams.clone(),
                board: self.picks.clone(),
            });
        }
        self.new_offer();
        out.push(self.state(DraftStatus::Running));
        None
    }

    fn new_offer(&mut self) {
        self.offer.clear();
        let picked: Vec<String> = self.picks.iter().map(|(id, _)| id.clone()).collect();
        let table = goals::table();
        // Respect goal groups (no conflicting goals on the board), checked against what's picked.
        let mut skipped = Vec::new();
        while self.offer.len() < self.offer_size {
            let Some(goal) = self.pool.pop_front() else { break };
            if board::can_add(table, &goal, &picked) {
                let data = self.data_for(&goal);
                self.offer.push((goal, data));
            } else {
                skipped.push(goal);
            }
        }
        self.pool.extend(skipped);
        // Out of compatible goals: offer anything that isn't on the board yet.
        while self.offer.len() < self.offer_size {
            let Some(goal) = self.pool.pop_front() else { break };
            let data = self.data_for(&goal);
            self.offer.push((goal, data));
        }
        self.ticks_left = self.pick_ticks;
    }

    fn data_for(&mut self, goal: &str) -> String {
        match goals::table().get(goal) {
            Some(g) => board::data_for(g, &self.world, &mut self.rng),
            None => DATA_NONE.to_owned(),
        }
    }

    pub fn cancel(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        self.done = true;
        if let Some(mut s) = self.scouting.take() {
            s.end(&mut out);
        }
        out.push(self.state(DraftStatus::Cancelled));
        out.push(Action::Broadcast(vec![Span::new("The draft was cancelled.", RED)]));
        out
    }

    pub fn is_scouting(&self, player: u128) -> bool {
        self.scouting.as_ref().is_some_and(|s| s.is_scouting(player))
    }

    /// The full state for every client (`DraftStatePayload`).
    pub fn state(&self, status: DraftStatus) -> Action {
        Action::SendAll {
            payload: Payload::DraftState {
                status,
                teams: self
                    .teams
                    .iter()
                    .zip(&self.rerolls_left)
                    .map(|(t, r)| DraftTeam {
                        team: t.wire(),
                        rerolls: *r,
                    })
                    .collect(),
                board_size: self.board_size as i32,
                picks: self
                    .picks
                    .iter()
                    .zip(&self.pick_teams)
                    .map(|((id, data), t)| (id.clone(), data.clone(), *t as i32))
                    .collect(),
                offer: self.offer.clone(),
                current_team: self.current_team() as i32,
                ticks_left: self.ticks_left.max(0),
                pick_ticks: self.pick_ticks,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::teams_of_players;

    const A: u128 = 1;
    const B: u128 = 2;

    fn draft(scouting: i32) -> Draft {
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(A, "Alex".into()), (B, "Steve".into())],
            false,
        )
        .unwrap();
        let settings = DraftSettings {
            offer_size: 2,
            pick_seconds: 3,
            rerolls: 1,
            scouting_seconds: scouting,
        };
        Draft::new(teams, 3, settings, World::everything(goals::table()), Rng::new(9))
    }

    fn offer(d: &Draft) -> Vec<(String, String)> {
        d.offer.clone()
    }

    #[test]
    fn picks_go_in_snake_order_and_fill_the_board() {
        let mut d = draft(0);
        d.start(&[]);
        let mut order = Vec::new();
        let mut finished = None;
        for _ in 0..9 {
            let team = d.current_team();
            order.push(team);
            let player = if team == 0 { A } else { B };
            let (_, outcome) = d.action(player, 0, &[]);
            if let DraftOutcome::Finished { board, .. } = outcome {
                finished = Some(board);
            }
        }
        assert_eq!(order, vec![0, 1, 1, 0, 0, 1, 1, 0, 0]);
        let board = finished.expect("the board is drafted");
        assert_eq!(board.len(), 9);
        let ids: Vec<String> = board.iter().map(|(id, _)| id.clone()).collect();
        for group in &goals::table().groups {
            assert!(ids.iter().filter(|i| group.goals.contains(i)).count() <= group.limit);
        }
    }

    #[test]
    fn only_the_picking_team_picks_and_rerolls_run_out() {
        let mut d = draft(0);
        d.start(&[]);
        let (out, _) = d.action(B, 0, &[]);
        assert!(matches!(&out[0], Action::Tell { player: B, .. }));
        let before = offer(&d);
        d.action(A, REROLL, &[]);
        assert_ne!(offer(&d), before);
        let again = offer(&d);
        d.action(A, REROLL, &[]);
        assert_eq!(offer(&d), again, "one reroll per team");
    }

    #[test]
    fn the_timer_picks_at_random() {
        let mut d = draft(0);
        d.start(&[]);
        for _ in 0..60 {
            d.tick(&[]);
        }
        assert_eq!(d.picks.len(), 1);
        assert_eq!(d.current_team(), 1);
    }

    #[test]
    fn scouting_freezes_toggles_the_view_and_releases() {
        let mut d = draft(2);
        let scout = |jumping: bool, y: f64| Scout {
            uuid: A,
            dimension: "minecraft:overworld".into(),
            position: [0.5, y, 0.5],
            jumping,
            has_spyglass: false,
        };
        let out = d.start(&[scout(false, 64.0)]);
        assert!(out.contains(&Action::Freeze {
            player: A,
            modifier: SCOUTING_FREEZE,
            on: true
        }));
        assert!(out.contains(&Action::Give {
            player: A,
            item: "minecraft:spyglass".into()
        }));
        let (out, _) = d.tick(&[scout(true, 64.0)]);
        assert!(out.contains(&Action::Flight {
            player: A,
            allow: true,
            flying: true
        }));
        assert!(out.contains(&Action::Teleport {
            player: A,
            dimension: "minecraft:overworld".into(),
            pos: [0.5, 94.0, 0.5]
        }));
        let mut released = false;
        for _ in 0..40 {
            let (out, _) = d.tick(&[scout(false, 94.0)]);
            released |= out.contains(&Action::Freeze {
                player: A,
                modifier: SCOUTING_FREEZE,
                on: false,
            });
        }
        assert!(released && d.scouting.is_none() && !d.offer.is_empty());
    }
}
