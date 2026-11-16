//! `CompassItemHandler`: every lockout player gets a compass that tracks another player.
//! Using it cycles to the next player; it can't be thrown away, and it comes back into the
//! same slot after a death.
use crate::server::{Action, Team};
use std::collections::HashMap;

/// The custom data key that marks a tracking compass (`PlayerTracker`).
pub const TRACKER_KEY: &str = "PlayerTracker";

#[derive(Debug, Clone, Default)]
pub struct Compass {
    pub players: Vec<u128>,
    pub names: HashMap<u128, String>,
    selection: HashMap<u128, usize>,
    /// Where each player's compass was when they died (41-slot numbering).
    pub slots: HashMap<u128, usize>,
    /// The last target sent for each player, so only changes are sent.
    sent: HashMap<u128, Option<(String, [i32; 3])>>,
}

impl Compass {
    pub fn new(teams: &[Team]) -> Self {
        let mut c = Self::default();
        for (i, (uuid, name)) in teams.iter().flat_map(|t| &t.players).enumerate() {
            c.players.push(*uuid);
            c.names.insert(*uuid, name.clone());
            c.selection.insert(*uuid, if i == 0 { 1 } else { 0 });
        }
        c
    }

    /// The player `player`'s compass tracks.
    pub fn target(&self, player: u128) -> Option<u128> {
        self.selection.get(&player).and_then(|i| self.players.get(*i)).copied()
    }

    /// `Tracking: <name>`, the compass's name.
    pub fn label(&self, player: u128) -> String {
        let name = self
            .target(player)
            .and_then(|t| self.names.get(&t))
            .cloned()
            .unwrap_or_default();
        format!("Tracking: {name}")
    }

    /// Right click: the next player, skipping yourself.
    pub fn cycle(&mut self, player: u128) -> Vec<Action> {
        let n = self.players.len();
        let Some(cur) = self.selection.get(&player).copied() else {
            return Vec::new();
        };
        if n < 2 {
            return Vec::new();
        }
        let mut next = (cur + 1) % n;
        if self.players[next] == player {
            next = (next + 1) % n;
        }
        self.selection.insert(player, next);
        vec![Action::RenameCompass {
            player,
            name: self.label(player),
        }]
    }

    /// Points each compass at its target when that player is in the same dimension; at the
    /// world spawn when they are offline; unchanged while they are elsewhere.
    pub fn update(&mut self, online: &[(u128, String, [f64; 3])]) -> Vec<Action> {
        let mut out = Vec::new();
        for &player in &self.players {
            let Some(me) = online.iter().find(|p| p.0 == player) else {
                continue;
            };
            let Some(target) = self.target(player) else { continue };
            let next = match online.iter().find(|p| p.0 == target) {
                None => Some(None),
                Some(t) if t.1 == me.1 => Some(Some((t.1.clone(), t.2.map(|v| v.floor() as i32)))),
                Some(_) => None,
            };
            if let Some(next) = next
                && self.sent.get(&player) != Some(&next)
            {
                self.sent.insert(player, next.clone());
                out.push(Action::CompassTarget { player, target: next });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::teams_of_players;

    #[test]
    fn cycles_past_yourself() {
        let roster = [(1, "A".to_string()), (2, "B".to_string()), (3, "C".to_string())];
        let teams = teams_of_players(&["A".into(), "B".into(), "C".into()], &roster, false).unwrap();
        let mut c = Compass::new(&teams);
        assert_eq!(c.target(1), Some(2));
        assert_eq!(c.target(2), Some(1));
        c.cycle(2);
        assert_eq!(c.target(2), Some(3));
        c.cycle(2);
        assert_eq!(c.target(2), Some(1), "skips player 2 itself");
        assert_eq!(c.label(2), "Tracking: A");
    }

    #[test]
    fn points_only_within_a_dimension() {
        let roster = [(1, "A".to_string()), (2, "B".to_string())];
        let teams = teams_of_players(&["A".into(), "B".into()], &roster, false).unwrap();
        let mut c = Compass::new(&teams);
        let ow = "minecraft:overworld".to_string();
        let out = c.update(&[(1, ow.clone(), [0.0, 64.0, 0.0]), (2, ow.clone(), [10.5, 70.0, -3.2])]);
        assert!(out.contains(&Action::CompassTarget {
            player: 1,
            target: Some((ow.clone(), [10, 70, -4]))
        }));
        let out = c.update(&[
            (1, ow.clone(), [0.0, 64.0, 0.0]),
            (2, "minecraft:the_nether".into(), [1.0, 1.0, 1.0]),
        ]);
        assert!(!out.iter().any(|a| matches!(a, Action::CompassTarget { player: 1, .. })));
        let out = c.update(&[(1, ow, [0.0, 64.0, 0.0])]);
        assert!(out.contains(&Action::CompassTarget {
            player: 1,
            target: None
        }));
    }
}
