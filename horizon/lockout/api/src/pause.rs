//! `MatchPause`: a match restored after a server restart stays paused until every player is
//! back online. Players who are already back are frozen in adventure mode (and can't be
//! hurt); once everyone is online, a short countdown resumes the match. The timer doesn't run
//! while paused.
use crate::server::{Action, Match, Span};
use crate::wire::Payload;
use std::collections::HashMap;

const RESUME_COUNTDOWN_SECONDS: i32 = 5;
const WAITING_MESSAGE_INTERVAL_TICKS: i32 = 20 * 15;
const MAX_DRIFT: f64 = 0.5;
const YELLOW: u32 = 0xFFFF55;
const GREEN: u32 = 0x55FF55;
const WHITE: u32 = 0xFFFFFF;
/// The attribute modifier that freezes players while paused.
pub const PAUSE_FREEZE: &str = "lockout:paused";

/// An online player, as the pause needs them.
#[derive(Debug, Clone)]
pub struct Present {
    pub uuid: u128,
    pub dimension: String,
    pub position: [f64; 3],
}

#[derive(Debug, Default)]
pub struct Pause {
    resume_countdown: i32,
    paused_ticks: i32,
    frozen: HashMap<u128, (String, [f64; 3])>,
}

pub fn waiting_message(missing: &[String]) -> Action {
    Action::Broadcast(vec![Span::new(
        format!(
            "The match is paused until everyone is back. Waiting for: {}",
            missing.join(", ")
        ),
        YELLOW,
    )])
}

pub fn missing(game: &Match, online: &[Present]) -> Vec<String> {
    game.teams
        .iter()
        .flat_map(|t| &t.players)
        .filter(|(u, _)| !online.iter().any(|p| p.uuid == *u))
        .map(|(_, n)| n.clone())
        .collect()
}

impl Pause {
    pub fn new() -> Self {
        Self {
            resume_countdown: -1,
            paused_ticks: 0,
            frozen: HashMap::new(),
        }
    }

    pub fn is_frozen(&self, player: u128) -> bool {
        self.frozen.contains_key(&player)
    }

    /// Ends the pause without resuming (a new match or draft replaces it): unfreezes everyone.
    pub fn cancel(&mut self, out: &mut Vec<Action>) {
        for p in self.frozen.keys() {
            out.push(Action::Freeze {
                player: *p,
                modifier: PAUSE_FREEZE,
                on: false,
            });
        }
        self.frozen.clear();
    }

    /// Freezes the player in place (again, if they respawned or rejoined) and pulls them back
    /// if they drifted.
    fn keep_frozen(&mut self, p: &Present, out: &mut Vec<Action>) {
        match self.frozen.get(&p.uuid) {
            None => {
                out.push(Action::GameMode {
                    player: p.uuid,
                    mode: 2,
                });
                out.push(Action::Freeze {
                    player: p.uuid,
                    modifier: PAUSE_FREEZE,
                    on: true,
                });
                self.frozen.insert(p.uuid, (p.dimension.clone(), p.position));
            }
            Some((dim, at)) => {
                let drift = ((at[0] - p.position[0]).powi(2)
                    + (at[1] - p.position[1]).powi(2)
                    + (at[2] - p.position[2]).powi(2))
                .sqrt();
                if *dim != p.dimension || drift > MAX_DRIFT {
                    out.push(Action::Teleport {
                        player: p.uuid,
                        dimension: dim.clone(),
                        pos: *at,
                    });
                }
            }
        }
    }

    /// A player left: they are frozen again when they're back.
    pub fn left(&mut self, player: u128) {
        self.frozen.remove(&player);
    }

    /// One tick; true once the match has resumed (`game` is then started again).
    pub fn tick(&mut self, game: &mut Match, online: &[Present], out: &mut Vec<Action>) -> bool {
        self.paused_ticks += 1;
        let lockout: Vec<&Present> = online.iter().filter(|p| game.team_of(p.uuid).is_some()).collect();
        for p in &lockout {
            self.keep_frozen(p, out);
        }
        let missing = missing(game, online);
        if !missing.is_empty() {
            if self.resume_countdown >= 0 {
                out.push(Action::Broadcast(vec![Span::new(
                    format!("{} left. Waiting for everyone to be back.", missing.join(", ")),
                    YELLOW,
                )]));
                self.resume_countdown = -1;
            } else if self.paused_ticks % WAITING_MESSAGE_INTERVAL_TICKS == 1 {
                out.push(waiting_message(&missing));
            }
            return false;
        }
        if self.resume_countdown < 0 {
            self.resume_countdown = RESUME_COUNTDOWN_SECONDS * 20;
            out.push(Action::Broadcast(vec![Span::new(
                format!("Everyone is back! Resuming in {RESUME_COUNTDOWN_SECONDS}..."),
                GREEN,
            )]));
            return false;
        }
        self.resume_countdown -= 1;
        if self.resume_countdown == 0 {
            for p in &lockout {
                out.push(Action::Freeze {
                    player: p.uuid,
                    modifier: PAUSE_FREEZE,
                    on: false,
                });
                out.push(Action::GameMode {
                    player: p.uuid,
                    mode: 0,
                });
            }
            self.frozen.clear();
            // Restarted during the pre-match countdown: start right away.
            if game.ticks < 0 {
                game.ticks = 0;
            }
            game.started = true;
            out.push(Action::SendAll {
                payload: Payload::Start,
            });
            out.push(Action::SendAll {
                payload: Payload::UpdateTimer { ticks: game.ticks },
            });
            out.push(Action::Broadcast(vec![Span::new("The match has resumed!", GREEN)]));
            return true;
        } else if self.resume_countdown % 20 == 0 && self.resume_countdown <= 3 * 20 {
            out.push(Action::Broadcast(vec![Span::new(
                format!("Resuming in {}...", self.resume_countdown / 20),
                WHITE,
            )]));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goals::DATA_NONE;
    use crate::server::teams_of_players;

    const A: u128 = 1;
    const B: u128 = 2;

    fn present(uuid: u128) -> Present {
        Present {
            uuid,
            dimension: "minecraft:overworld".into(),
            position: [0.0, 64.0, 0.0],
        }
    }

    #[test]
    fn waits_for_everyone_then_counts_down() {
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(A, "Alex".into()), (B, "Steve".into())],
            false,
        )
        .unwrap();
        let mut game = Match::new(teams, vec![("ENTER_END".into(), DATA_NONE.into())], 0);
        game.ticks = 1200;
        let mut pause = Pause::new();
        let mut out = Vec::new();
        assert!(!pause.tick(&mut game, &[present(A)], &mut out));
        assert!(out.contains(&Action::Freeze {
            player: A,
            modifier: PAUSE_FREEZE,
            on: true
        }));
        assert!(out.contains(&waiting_message(&["Steve".into()])));
        let mut resumed = false;
        for _ in 0..(5 * 20 + 2) {
            let mut out = Vec::new();
            resumed |= pause.tick(&mut game, &[present(A), present(B)], &mut out);
        }
        assert!(resumed && game.started && game.ticks == 1200);
    }
}
