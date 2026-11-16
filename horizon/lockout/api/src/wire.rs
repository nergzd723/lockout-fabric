//! The Fabric mod's network payloads, byte for byte.
//!
//! Each payload is a plugin channel (`lockout:<name>`) whose bytes are what the Java
//! `StreamCodec`s write through Minecraft's `FriendlyByteBuf`: `writeInt` and `writeLong`
//! big-endian, `writeUtf` a VarInt byte length and UTF-8, `writeVarIntArray` a VarInt
//! count and VarInts, `writeBoolean` one byte. Keeping them identical lets a Horizon
//! client play on a Fabric Lockout server, and a Fabric client on a Horizon one.

/// The version the Fabric mod checks on join. A client or server with another version is
/// disconnected, so this must equal `mod_version` in `gradle.properties`.
pub const LOCKOUT_VERSION: &str = "0.10.4";

pub const GOALS_TEAMS: &str = "lockout:lockout_goals_teams";
pub const START: &str = "lockout:start_lockout";
pub const UPDATE_TOOLTIP: &str = "lockout:update_tooltip";
pub const COMPLETE_TASK: &str = "lockout:complete_task";
pub const END: &str = "lockout:end_lockout";
pub const UPDATE_TIMER: &str = "lockout:update_timer";
pub const VERSION: &str = "lockout:lockout_version";
pub const DRAFT_STATE: &str = "lockout:draft_state";
pub const CUSTOM_BOARD: &str = "lockout:set_custom_board";
pub const DRAFT_ACTION: &str = "lockout:draft_action";

/// Every clientbound channel, for `minecraft:register`.
pub const CLIENTBOUND: &[&str] = &[
    GOALS_TEAMS,
    START,
    UPDATE_TOOLTIP,
    COMPLETE_TASK,
    END,
    UPDATE_TIMER,
    VERSION,
    DRAFT_STATE,
];
/// Every serverbound channel this port speaks.
pub const SERVERBOUND: &[&str] = &[VERSION, CUSTOM_BOARD, DRAFT_ACTION];

/// `DraftActionPayload.REROLL`: the action that asks for a new offer.
pub const REROLL: i32 = -1;

/// `DraftStatePayload.Status`, by ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    Running,
    Finished,
    Cancelled,
}

/// A team in the draft payload, with the rerolls it has left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftTeam {
    pub team: WireTeam,
    pub rerolls: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireError(pub &'static str);

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Default)]
pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    pub fn int(&mut self, v: i32) {
        self.data.extend_from_slice(&v.to_be_bytes());
    }
    pub fn long(&mut self, v: i64) {
        self.data.extend_from_slice(&v.to_be_bytes());
    }
    pub fn boolean(&mut self, v: bool) {
        self.data.push(v as u8);
    }
    pub fn varint(&mut self, v: i32) {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.data.push(v as u8);
                return;
            }
            self.data.push((v & 0x7F | 0x80) as u8);
            v >>= 7;
        }
    }
    pub fn utf(&mut self, s: &str) {
        self.varint(s.len() as i32);
        self.data.extend_from_slice(s.as_bytes());
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError("length overflow"))?;
        let out = self.data.get(self.pos..end).ok_or(WireError("payload ended early"))?;
        self.pos = end;
        Ok(out)
    }
    pub fn int(&mut self) -> Result<i32, WireError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn long(&mut self) -> Result<i64, WireError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn boolean(&mut self) -> Result<bool, WireError> {
        Ok(self.take(1)?[0] != 0)
    }
    pub fn varint(&mut self) -> Result<i32, WireError> {
        let mut out = 0u32;
        for i in 0..5 {
            let b = self.take(1)?[0];
            out |= ((b & 0x7F) as u32) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(out as i32);
            }
        }
        Err(WireError("VarInt too big"))
    }
    pub fn utf(&mut self) -> Result<String, WireError> {
        let len = self.varint()?;
        if !(0..=32767 * 3).contains(&len) {
            return Err(WireError("bad string length"));
        }
        String::from_utf8(self.take(len as usize)?.to_vec()).map_err(|_| WireError("bad UTF-8"))
    }
    /// A sane upper bound on a list's length, so a broken payload can't make us allocate.
    fn count(&mut self, v: i32) -> Result<usize, WireError> {
        if v < 0 || v as usize > self.data.len() - self.pos.min(self.data.len()) + 1 {
            return Err(WireError("bad list length"));
        }
        Ok(v as usize)
    }
}

/// A team as the board payload carries it: its colour's name (`red`, `dark_blue`...) and
/// its players' names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireTeam {
    pub color: String,
    pub players: Vec<String>,
}

/// One square of the board: goal id, its data (`"null"` when it has none), and the index of
/// the team that holds it (-1 for none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGoal {
    pub id: String,
    pub data: String,
    pub completed_by: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    GoalsTeams {
        teams: Vec<WireTeam>,
        goals: Vec<WireGoal>,
        running: bool,
    },
    Start,
    UpdateTooltip {
        goal: String,
        tooltip: String,
    },
    CompleteTask {
        goal: String,
        team: i32,
    },
    End {
        winners: Vec<i32>,
        time: i64,
    },
    UpdateTimer {
        ticks: i64,
    },
    Version {
        version: String,
    },
    /// A Fabric client's `/SetCustomBoard` (Some) or `/RemoveCustomBoard` (None): the
    /// board's side as an int, then each goal's id and data.
    CustomBoard {
        goals: Option<Vec<(String, String)>>,
    },
    /// The whole draft (`DraftStatePayload`): sent whenever it changes and once a second.
    /// `picks` are (goal, data, team) in board order, `offer` what the picking team may take.
    DraftState {
        status: DraftStatus,
        teams: Vec<DraftTeam>,
        board_size: i32,
        picks: Vec<(String, String, i32)>,
        offer: Vec<(String, String)>,
        current_team: i32,
        ticks_left: i32,
        pick_ticks: i32,
    },
    /// A drafting player's pick (an offer index) or [`REROLL`] (`DraftActionPayload`).
    DraftAction {
        action: i32,
    },
}

impl Payload {
    pub fn channel(&self) -> &'static str {
        match self {
            Payload::GoalsTeams { .. } => GOALS_TEAMS,
            Payload::Start => START,
            Payload::UpdateTooltip { .. } => UPDATE_TOOLTIP,
            Payload::CompleteTask { .. } => COMPLETE_TASK,
            Payload::End { .. } => END,
            Payload::UpdateTimer { .. } => UPDATE_TIMER,
            Payload::Version { .. } => VERSION,
            Payload::CustomBoard { .. } => CUSTOM_BOARD,
            Payload::DraftState { .. } => DRAFT_STATE,
            Payload::DraftAction { .. } => DRAFT_ACTION,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Payload::GoalsTeams { teams, goals, running } => {
                w.int(teams.len() as i32);
                for team in teams {
                    w.int(team.players.len() as i32);
                    w.utf(&team.color);
                    for p in &team.players {
                        w.utf(p);
                    }
                }
                w.int(goals.len() as i32);
                for g in goals {
                    w.utf(&g.id);
                    w.utf(&g.data);
                    w.int(g.completed_by);
                }
                w.boolean(*running);
            }
            Payload::Start => {}
            Payload::UpdateTooltip { goal, tooltip } => {
                w.utf(goal);
                w.utf(tooltip);
            }
            Payload::CompleteTask { goal, team } => {
                w.utf(goal);
                w.int(*team);
            }
            Payload::End { winners, time } => {
                w.varint(winners.len() as i32);
                for v in winners {
                    w.varint(*v);
                }
                w.long(*time);
            }
            Payload::UpdateTimer { ticks } => w.long(*ticks),
            Payload::Version { version } => w.utf(version),
            Payload::CustomBoard { goals } => match goals {
                None => w.int(0),
                Some(goals) => {
                    w.int((goals.len() as f64).sqrt() as i32);
                    for (id, data) in goals {
                        w.utf(id);
                        w.utf(data);
                    }
                }
            },
            Payload::DraftState {
                status,
                teams,
                board_size,
                picks,
                offer,
                current_team,
                ticks_left,
                pick_ticks,
            } => {
                w.varint(*status as i32);
                w.varint(teams.len() as i32);
                for t in teams {
                    w.utf(&t.team.color);
                    w.varint(t.team.players.len() as i32);
                    for p in &t.team.players {
                        w.utf(p);
                    }
                    w.varint(t.rerolls);
                }
                w.varint(*board_size);
                w.varint(picks.len() as i32);
                for (id, data, team) in picks {
                    w.utf(id);
                    w.utf(data);
                    w.varint(*team);
                }
                w.varint(offer.len() as i32);
                for (id, data) in offer {
                    w.utf(id);
                    w.utf(data);
                }
                w.varint(*current_team);
                w.varint(*ticks_left);
                w.varint(*pick_ticks);
            }
            Payload::DraftAction { action } => w.varint(*action),
        }
        w.data
    }

    /// The payload on `channel`, or None for a channel that isn't Lockout's.
    pub fn decode(channel: &str, data: &[u8]) -> Option<Result<Payload, WireError>> {
        let mut r = Reader::new(data);
        let r = &mut r;
        let decoded = match channel {
            GOALS_TEAMS => (|| {
                let n = r.int()?;
                let n = r.count(n)?;
                let mut teams = Vec::with_capacity(n);
                for _ in 0..n {
                    let size = r.int()?;
                    let size = r.count(size)?;
                    let color = r.utf()?;
                    let mut players = Vec::with_capacity(size);
                    for _ in 0..size {
                        players.push(r.utf()?);
                    }
                    teams.push(WireTeam { color, players });
                }
                let n = r.int()?;
                let n = r.count(n)?;
                let mut goals = Vec::with_capacity(n);
                for _ in 0..n {
                    goals.push(WireGoal {
                        id: r.utf()?,
                        data: r.utf()?,
                        completed_by: r.int()?,
                    });
                }
                Ok(Payload::GoalsTeams {
                    teams,
                    goals,
                    running: r.boolean()?,
                })
            })(),
            START => Ok(Payload::Start),
            UPDATE_TOOLTIP => (|| {
                Ok(Payload::UpdateTooltip {
                    goal: r.utf()?,
                    tooltip: r.utf()?,
                })
            })(),
            COMPLETE_TASK => (|| {
                Ok(Payload::CompleteTask {
                    goal: r.utf()?,
                    team: r.int()?,
                })
            })(),
            END => (|| {
                let n = r.varint()?;
                let n = r.count(n)?;
                let mut winners = Vec::with_capacity(n);
                for _ in 0..n {
                    winners.push(r.varint()?);
                }
                Ok(Payload::End {
                    winners,
                    time: r.long()?,
                })
            })(),
            UPDATE_TIMER => (|| Ok(Payload::UpdateTimer { ticks: r.long()? }))(),
            VERSION => (|| Ok(Payload::Version { version: r.utf()? }))(),
            CUSTOM_BOARD => (|| {
                let side = r.int()?;
                if side == 0 {
                    return Ok(Payload::CustomBoard { goals: None });
                }
                if !(1..=16).contains(&side) {
                    return Err(WireError("bad board size"));
                }
                let mut goals = Vec::new();
                for _ in 0..side * side {
                    goals.push((r.utf()?, r.utf()?));
                }
                Ok(Payload::CustomBoard { goals: Some(goals) })
            })(),
            DRAFT_STATE => (|| {
                let status = match r.varint()? {
                    0 => DraftStatus::Running,
                    1 => DraftStatus::Finished,
                    2 => DraftStatus::Cancelled,
                    _ => return Err(WireError("bad draft status")),
                };
                let n = r.varint()?;
                let n = r.count(n)?;
                let mut teams = Vec::with_capacity(n);
                for _ in 0..n {
                    let color = r.utf()?;
                    let size = r.varint()?;
                    let size = r.count(size)?;
                    let mut players = Vec::with_capacity(size);
                    for _ in 0..size {
                        players.push(r.utf()?);
                    }
                    teams.push(DraftTeam {
                        team: WireTeam { color, players },
                        rerolls: r.varint()?,
                    });
                }
                let board_size = r.varint()?;
                let n = r.varint()?;
                let n = r.count(n)?;
                let mut picks = Vec::with_capacity(n);
                for _ in 0..n {
                    picks.push((r.utf()?, r.utf()?, r.varint()?));
                }
                let n = r.varint()?;
                let n = r.count(n)?;
                let mut offer = Vec::with_capacity(n);
                for _ in 0..n {
                    offer.push((r.utf()?, r.utf()?));
                }
                Ok(Payload::DraftState {
                    status,
                    teams,
                    board_size,
                    picks,
                    offer,
                    current_team: r.varint()?,
                    ticks_left: r.varint()?,
                    pick_ticks: r.varint()?,
                })
            })(),
            DRAFT_ACTION => (|| Ok(Payload::DraftAction { action: r.varint()? }))(),
            _ => return None,
        };
        Some(decoded)
    }
}

/// `minecraft:register`'s body: the channel names, each ended by a NUL.
pub fn register_body(channels: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in channels {
        out.extend_from_slice(c.as_bytes());
        out.push(0);
    }
    out
}

/// The 16 team colours in `ChatFormatting`/`TeamColor` order: name and RGB.
pub const TEAM_COLORS: [(&str, u32); 16] = [
    ("black", 0x000000),
    ("dark_blue", 0x0000AA),
    ("dark_green", 0x00AA00),
    ("dark_aqua", 0x00AAAA),
    ("dark_red", 0xAA0000),
    ("dark_purple", 0xAA00AA),
    ("gold", 0xFFAA00),
    ("gray", 0xAAAAAA),
    ("dark_gray", 0x555555),
    ("blue", 0x5555FF),
    ("green", 0x55FF55),
    ("aqua", 0x55FFFF),
    ("red", 0xFF5555),
    ("light_purple", 0xFF55FF),
    ("yellow", 0xFFFF55),
    ("white", 0xFFFFFF),
];

/// The order the Fabric mod hands colours to teams in (`Lockout.COLOR_ORDERS`).
pub const COLOR_ORDER: [usize; 16] = [12, 9, 10, 14, 6, 13, 11, 5, 3, 2, 15, 4, 7, 1, 8, 0];

pub fn color_rgb(name: &str) -> u32 {
    TEAM_COLORS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(0xFFFFFF, |(_, rgb)| *rgb)
}

/// `Red`, `Dark blue`: a team's name when it is named after its colour.
pub fn color_display(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf_and_varint_match_friendly_byte_buf() {
        let mut w = Writer::default();
        w.utf("lockout");
        w.varint(300);
        w.varint(-1);
        assert_eq!(&w.data[..8], b"\x07lockout");
        assert_eq!(&w.data[8..10], &[0xAC, 0x02]);
        assert_eq!(&w.data[10..], &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        let mut r = Reader::new(&w.data);
        assert_eq!(r.utf().unwrap(), "lockout");
        assert_eq!(r.varint().unwrap(), 300);
        assert_eq!(r.varint().unwrap(), -1);
    }

    #[test]
    fn complete_task_is_string_then_big_endian_int() {
        let p = Payload::CompleteTask {
            goal: "ENTER_END".into(),
            team: 1,
        };
        assert_eq!(p.encode(), b"\x09ENTER_END\x00\x00\x00\x01");
    }

    #[test]
    fn every_payload_round_trips() {
        let all = [
            Payload::GoalsTeams {
                teams: vec![WireTeam {
                    color: "red".into(),
                    players: vec!["Alex".into(), "Steve".into()],
                }],
                goals: vec![WireGoal {
                    id: "KILL_COLORED_SHEEP".into(),
                    data: "lime".into(),
                    completed_by: -1,
                }],
                running: true,
            },
            Payload::Start,
            Payload::UpdateTooltip {
                goal: "EAT_5_UNIQUE_FOOD".into(),
                tooltip: " \nFoods: 1/5".into(),
            },
            Payload::CompleteTask {
                goal: "ENTER_NETHER".into(),
                team: -1,
            },
            Payload::End {
                winners: vec![0, 2],
                time: 1_700_000_000_000,
            },
            Payload::UpdateTimer { ticks: -200 },
            Payload::Version {
                version: LOCKOUT_VERSION.into(),
            },
            Payload::CustomBoard { goals: None },
            Payload::DraftAction { action: REROLL },
            Payload::DraftState {
                status: DraftStatus::Running,
                teams: vec![DraftTeam {
                    team: WireTeam {
                        color: "aqua".into(),
                        players: vec!["Alex".into()],
                    },
                    rerolls: 1,
                }],
                board_size: 5,
                picks: vec![("ENTER_END".into(), "null".into(), 0)],
                offer: vec![
                    ("KILL_WITCH".into(), "null".into()),
                    ("KILL_COLORED_SHEEP".into(), "red".into()),
                ],
                current_team: 0,
                ticks_left: 140,
                pick_ticks: 200,
            },
            Payload::CustomBoard {
                goals: Some(vec![("ENTER_END".into(), "null".into()); 9]),
            },
        ];
        for p in all {
            let back = Payload::decode(p.channel(), &p.encode()).unwrap().unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn a_truncated_payload_is_an_error_not_a_panic() {
        let bytes = Payload::UpdateTimer { ticks: 5 }.encode();
        assert!(Payload::decode(UPDATE_TIMER, &bytes[..3]).unwrap().is_err());
        assert!(
            Payload::decode(GOALS_TEAMS, &[0x7F, 0xFF, 0xFF, 0xFF])
                .unwrap()
                .is_err()
        );
        assert!(Payload::decode("other:channel", &bytes).is_none());
    }
}
