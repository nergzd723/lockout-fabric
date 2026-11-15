//! The board in the window: what the server's payloads say, drawn as the Fabric mod draws
//! it (top right, a square per goal filled with the colour of the team that holds it, the
//! score and the timer under it, the players' names below). Hold the board key (B by
//! default) to list the goals' names and the progress the server sent.
use crate::goals;
use crate::wire::{self, Payload, WireTeam};
use horizon_client::mod_api::*;
use std::collections::HashMap;
use std::sync::Mutex;

const PADDING: i32 = 2;
const PADDING_BOTTOM: i32 = 13;
const SLOT: i32 = 18;
const LINE: i32 = 9;

#[derive(Debug, Clone)]
pub struct ClientGoal {
    pub id: String,
    pub name: String,
    pub icon: ItemStack,
    pub count: Option<u32>,
    pub completed_by: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    pub teams: Vec<WireTeam>,
    pub goals: Vec<ClientGoal>,
    pub ticks: i64,
    pub started: bool,
    pub running: bool,
}

impl Board {
    pub fn side(&self) -> i32 {
        (self.goals.len() as f64).sqrt().round() as i32
    }
    pub fn points(&self, team: usize) -> usize {
        self.goals.iter().filter(|g| g.completed_by == Some(team)).count()
    }
}

#[derive(Debug, Default)]
pub struct Client {
    pub board: Option<Board>,
    pub tooltips: HashMap<String, String>,
    /// The server spoke Lockout: we answered its version, as the Fabric client does.
    pub answered_version: bool,
    pub version_mismatch: Option<String>,
}

impl Client {
    /// Applies one of the server's payloads.
    pub fn apply(&mut self, payload: Payload) {
        let table = goals::table();
        match payload {
            Payload::GoalsTeams { teams, goals, running } => {
                let goals = goals
                    .into_iter()
                    .map(|g| {
                        let def = table.get(&g.id);
                        let icon = def.map_or_else(|| "minecraft:barrier".to_owned(), |d| d.icon_for(&g.data));
                        ClientGoal {
                            name: def.map_or_else(|| g.id.clone(), |d| d.name(&g.data)),
                            icon: ItemStack::new(&icon, 1),
                            count: def.and_then(|d| d.icon_count()),
                            completed_by: (g.completed_by >= 0).then_some(g.completed_by as usize),
                            id: g.id,
                        }
                    })
                    .collect();
                self.tooltips.clear();
                self.board = Some(Board {
                    teams,
                    goals,
                    ticks: 0,
                    started: false,
                    running,
                });
            }
            Payload::Start => {
                if let Some(b) = &mut self.board {
                    b.started = true;
                }
            }
            Payload::UpdateTimer { ticks } => {
                if let Some(b) = &mut self.board {
                    b.ticks = ticks;
                }
            }
            Payload::CompleteTask { goal, team } => {
                if let Some(b) = &mut self.board
                    && let Some(g) = b.goals.iter_mut().find(|g| g.id == goal)
                {
                    g.completed_by = (team >= 0 && (team as usize) < b.teams.len()).then_some(team as usize);
                }
            }
            Payload::End { .. } => {
                if let Some(b) = &mut self.board {
                    b.running = false;
                }
            }
            Payload::UpdateTooltip { goal, tooltip } => {
                self.tooltips.insert(goal, tooltip);
            }
            Payload::Version { .. } | Payload::CustomBoard { .. } => {}
        }
    }
}

/// A plugin message from the server. A Fabric Lockout server sends its version first and
/// drops players who don't answer it before the next ping, so the answer goes out from
/// here, before the game handles the packets that follow.
pub fn packet(
    client: &Mutex<Client>,
    host: &mut dyn Host,
    channel: &str,
    data: &[u8],
    reply: impl Fn(&str, Vec<u8>) -> ServerboundPlayPacket,
) {
    let Some(Ok(payload)) = Payload::decode(channel, data) else {
        return;
    };
    let Ok(mut c) = client.lock() else { return };
    if let Payload::Version { version } = &payload {
        if !c.answered_version {
            let channels: Vec<&str> = wire::SERVERBOUND.iter().chain(wire::CLIENTBOUND).copied().collect();
            let _ = host.send_packet(reply("minecraft:register", wire::register_body(&channels)));
            c.answered_version = true;
        }
        let answer = Payload::Version {
            version: wire::LOCKOUT_VERSION.into(),
        };
        let _ = host.send_packet(reply(answer.channel(), answer.encode()));
        if version != wire::LOCKOUT_VERSION {
            c.version_mismatch = Some(version.clone());
            host.client_action_bar(
                Text::colored(
                    format!(
                        "Wrong Lockout version: v{}. Server is using Lockout v{version}.",
                        wire::LOCKOUT_VERSION
                    ),
                    0xFF5555,
                ),
                false,
            );
        }
        return;
    }
    c.apply(payload);
}

pub fn tick(_client: &Mutex<Client>, _host: &mut dyn Host) {}

/// `mm:ss`, or `hh:mm:ss` past an hour, as `Utility.ticksToTimer`.
pub fn timer(ticks: i64) -> String {
    let t = ticks.abs() / 20;
    let (h, m, s) = ((t / 3600) % 24, (t / 60) % 60, t % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | rgb
}

fn text(s: impl Into<String>, x: i32, y: i32, color: u32) -> GuiDraw {
    GuiDraw::Text {
        text: Text::literal(s),
        x,
        y,
        color,
        shadow: true,
    }
}

fn fill(x0: i32, y0: i32, x1: i32, y1: i32, color: u32) -> GuiDraw {
    GuiDraw::Fill { x0, y0, x1, y1, color }
}

/// The key that lists the goals: the player's choice for `key.lockout.open_board`.
fn board_key(host: &dyn Host) -> KeyCode {
    let name = host.mod_key("lockout", "open_board", "key.keyboard.b");
    let letter = name
        .strip_prefix("key.keyboard.")
        .and_then(|l| (l.len() == 1).then(|| l.chars().next()).flatten());
    match letter {
        Some(c @ 'a'..='z') => [
            KeyCode::KeyA,
            KeyCode::KeyB,
            KeyCode::KeyC,
            KeyCode::KeyD,
            KeyCode::KeyE,
            KeyCode::KeyF,
            KeyCode::KeyG,
            KeyCode::KeyH,
            KeyCode::KeyI,
            KeyCode::KeyJ,
            KeyCode::KeyK,
            KeyCode::KeyL,
            KeyCode::KeyM,
            KeyCode::KeyN,
            KeyCode::KeyO,
            KeyCode::KeyP,
            KeyCode::KeyQ,
            KeyCode::KeyR,
            KeyCode::KeyS,
            KeyCode::KeyT,
            KeyCode::KeyU,
            KeyCode::KeyV,
            KeyCode::KeyW,
            KeyCode::KeyX,
            KeyCode::KeyY,
            KeyCode::KeyZ,
        ][(c as u8 - b'a') as usize],
        _ => KeyCode::KeyB,
    }
}

pub fn draw(client: &Client, host: &dyn Host, batch: &mut HudBatch) {
    let Some(board) = &client.board else { return };
    let side = board.side();
    if side == 0 {
        return;
    }
    let gui_width = host.client_state().map_or(batch.width, |s| s.gui_size[0]) as i32;
    let width = 2 * PADDING + side * SLOT;
    let height = PADDING + PADDING_BOTTOM + side * SLOT;
    let right = gui_width;
    let left = right - width;
    let mut draws = vec![fill(left, 0, right, height, 0xC010_1010)];

    let start_x = left + PADDING + 1;
    let start_y = PADDING + 1;
    let mut items = Vec::new();
    for (i, goal) in board.goals.iter().enumerate() {
        let (row, col) = (i as i32 / side, i as i32 % side);
        let (x, y) = (start_x + col * SLOT, start_y + row * SLOT);
        draws.push(fill(x - 1, y - 1, x + 17, y + 17, 0x408B_8B8B));
        if let Some(team) = goal.completed_by.and_then(|t| board.teams.get(t)) {
            draws.push(fill(x, y, x + 16, y + 16, argb(wire::color_rgb(&team.color))));
        }
        items.push((goal, x, y));
    }

    // The score of each team, then the timer at the right.
    let y = start_y + side * SLOT + 1;
    let mut x = start_x + 2;
    for (i, team) in board.teams.iter().enumerate() {
        if i > 0 {
            draws.push(text("-", x, y, argb(0xAAAAAA)));
            x += host.text_width("-") as i32;
        }
        let points = board.points(i).to_string();
        draws.push(text(points.clone(), x, y, argb(wire::color_rgb(&team.color))));
        x += host.text_width(&points) as i32;
    }
    let clock = timer(board.ticks);
    draws.push(text(
        clock.clone(),
        right - host.text_width(&clock) as i32 - 4,
        y,
        argb(0xFFFFFF),
    ));

    // The players, under the board.
    let names: Vec<(&str, &str)> = board
        .teams
        .iter()
        .flat_map(|t| t.players.iter().map(move |p| (p.as_str(), t.color.as_str())))
        .collect();
    let widest = names.iter().map(|(n, _)| host.text_width(n) as i32).max().unwrap_or(0);
    let mut y = y + 20;
    if !names.is_empty() {
        draws.push(fill(
            gui_width - widest - 4,
            y - 2,
            gui_width - 1,
            y + names.len() as i32 * LINE + 1,
            0x8000_0000,
        ));
        for (name, color) in &names {
            draws.push(text(
                *name,
                gui_width - host.text_width(name) as i32 - 2,
                y,
                argb(wire::color_rgb(color)),
            ));
            y += LINE;
        }
    }

    let listing = host.playing() && host.is_key_down(board_key(host));
    if listing {
        draws.extend(goal_list(client, board, host));
    }
    host.gui_paint(batch, &draws);
    for (goal, x, y) in items {
        let count = goal.count.map(|c| c.to_string());
        host.gui_draw_item_with(batch, &goal.icon, [x as f32, y as f32], count.as_deref());
    }
}

/// Every goal's name, the team that holds it and the progress the server sent.
fn goal_list(client: &Client, board: &Board, host: &dyn Host) -> Vec<GuiDraw> {
    let mut lines: Vec<(String, u32)> = vec![(format!("{} goals", board.goals.len()), 0xFFFFFF)];
    for goal in &board.goals {
        let owner = goal.completed_by.and_then(|t| board.teams.get(t));
        let color = owner.map_or(0xFFFFFF, |t| wire::color_rgb(&t.color));
        lines.push((goal.name.clone(), color));
        if let Some(tip) = client.tooltips.get(&goal.id) {
            for l in tip.lines().map(str::trim).filter(|l| !l.is_empty()) {
                lines.push((format!("  {l}"), 0xAAAAAA));
            }
        }
    }
    let width = lines.iter().map(|(l, _)| host.text_width(l) as i32).max().unwrap_or(0);
    let mut out = vec![fill(2, 2, 8 + width, 6 + lines.len() as i32 * LINE, 0xC000_0000)];
    for (i, (l, c)) in lines.into_iter().enumerate() {
        out.push(text(l, 5, 5 + i as i32 * LINE, argb(c)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::WireGoal;

    #[test]
    fn the_timer_reads_like_the_java_one() {
        assert_eq!(timer(-1200), "01:00");
        assert_eq!(timer(20 * 3725), "01:02:05");
    }

    #[test]
    fn payloads_build_and_update_the_board() {
        let mut c = Client::default();
        c.apply(Payload::GoalsTeams {
            teams: vec![
                WireTeam {
                    color: "red".into(),
                    players: vec!["Alex".into()],
                },
                WireTeam {
                    color: "blue".into(),
                    players: vec!["Steve".into()],
                },
            ],
            goals: vec![
                WireGoal {
                    id: "KILL_COLORED_SHEEP".into(),
                    data: "lime".into(),
                    completed_by: -1,
                },
                WireGoal {
                    id: "ENTER_NETHER".into(),
                    data: "null".into(),
                    completed_by: 1,
                },
                WireGoal {
                    id: "SOME_GOAL_FROM_A_NEWER_MOD".into(),
                    data: "null".into(),
                    completed_by: -1,
                },
                WireGoal {
                    id: "KILL_7_UNIQUE_HOSTILE_MOBS".into(),
                    data: "null".into(),
                    completed_by: -1,
                },
            ],
            running: true,
        });
        c.apply(Payload::CompleteTask {
            goal: "KILL_COLORED_SHEEP".into(),
            team: 0,
        });
        let b = c.board.as_ref().unwrap();
        assert_eq!(b.side(), 2);
        assert_eq!((b.points(0), b.points(1)), (1, 1));
        assert_eq!(b.goals[0].name, "Kill Lime Sheep");
        assert_eq!(&*b.goals[0].icon.item, "minecraft:lime_wool");
        assert_eq!(&*b.goals[2].icon.item, "minecraft:barrier");
        assert_eq!(b.goals[3].count, Some(7));
        c.apply(Payload::CompleteTask {
            goal: "ENTER_NETHER".into(),
            team: -1,
        });
        assert_eq!(c.board.as_ref().unwrap().points(1), 0);
    }
}
