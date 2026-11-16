//! `MatchPersistence`: the running match is saved in the world folder (`lockout-match.json`)
//! every 30 seconds and when the server stops, and restored when it starts again.
//!
//! The file has the Java mod's layout (`version`, `ticks`, `started`, `teams` with colour,
//! points and players, `goals` with id, data and `completedBy`), so a world keeps its match
//! when it moves between a Fabric and a Horizon server. The Java mod's own goal trackers are
//! Java field dumps this port can't read; its progress is kept under `horizon` instead, and
//! each side starts the other's counters from zero.
use crate::server::{Match, Progress, Team};
use crate::wire::TEAM_COLORS;
use horizon_client::mod_api::serde_json::{self, Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "lockout-match.json";
const FORMAT_VERSION: i64 = 1;

pub fn path(world_dir: &Path) -> PathBuf {
    world_dir.join(FILE_NAME)
}

/// `8-4-4-4-12` hex, as `UUID.toString`.
pub fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn parse_uuid(s: &str) -> Option<u128> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok()).flatten()
}

pub fn to_json(m: &Match) -> Value {
    let teams: Vec<Value> = m
        .teams
        .iter()
        .map(|t| {
            json!({
                "color": TEAM_COLORS[t.color].0,
                "points": t.points,
                "players": t.players.iter().map(|(u, n)| json!({"uuid": uuid_string(*u), "name": n})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let goals: Vec<Value> = m
        .board
        .iter()
        .map(|s| json!({"id": s.id, "data": s.data, "completedBy": s.completed_by.map_or(-1, |t| t as i64)}))
        .collect();
    let progress: Vec<Value> = m
        .progress
        .iter()
        .map(|((square, team), p)| json!({"square": square, "team": team, "seen": p.seen, "count": p.count}))
        .collect();
    json!({
        "version": FORMAT_VERSION,
        "ticks": m.ticks,
        "started": m.started,
        "teams": teams,
        "goals": goals,
        "trackers": {},
        "horizon": {"progress": progress},
    })
}

pub fn from_json(v: &Value) -> Result<Match, String> {
    let version = v.get("version").and_then(Value::as_i64).ok_or("no version")?;
    if version != FORMAT_VERSION {
        return Err(format!("Unknown save format version {version}"));
    }
    let mut teams = Vec::new();
    for t in v.get("teams").and_then(Value::as_array).ok_or("no teams")? {
        let color = t.get("color").and_then(Value::as_str).ok_or("team without a colour")?;
        let color = TEAM_COLORS
            .iter()
            .position(|(n, _)| *n == color)
            .ok_or("unknown team colour")?;
        let mut players = Vec::new();
        for p in t
            .get("players")
            .and_then(Value::as_array)
            .ok_or("team without players")?
        {
            let uuid = p
                .get("uuid")
                .and_then(Value::as_str)
                .and_then(parse_uuid)
                .ok_or("bad player uuid")?;
            let name = p.get("name").and_then(Value::as_str).ok_or("player without a name")?;
            players.push((uuid, name.to_owned()));
        }
        let mut team = Team::new(color, players);
        team.points = t.get("points").and_then(Value::as_u64).unwrap_or(0) as u32;
        teams.push(team);
    }
    let mut goals = Vec::new();
    let mut completed = Vec::new();
    for g in v.get("goals").and_then(Value::as_array).ok_or("no goals")? {
        let id = g.get("id").and_then(Value::as_str).ok_or("goal without an id")?;
        let data = g.get("data").and_then(Value::as_str).unwrap_or(crate::goals::DATA_NONE);
        goals.push((id.to_owned(), data.to_owned()));
        completed.push(g.get("completedBy").and_then(Value::as_i64).unwrap_or(-1));
    }
    let mut m = Match::new(teams, goals, 0);
    m.ticks = v.get("ticks").and_then(Value::as_i64).unwrap_or(0);
    for (square, by) in completed.into_iter().enumerate() {
        if by >= 0 && (by as usize) < m.teams.len() {
            m.board[square].completed_by = Some(by as usize);
        }
    }
    let progress = v
        .pointer("/horizon/progress")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for p in progress {
        let (Some(square), Some(team)) = (
            p.get("square").and_then(Value::as_u64),
            p.get("team").and_then(Value::as_u64),
        ) else {
            continue;
        };
        let seen: BTreeSet<String> = p
            .get("seen")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        let count = p.get("count").and_then(Value::as_f64).unwrap_or(0.0);
        m.progress
            .insert((square as usize, team as usize), Progress { seen, count });
    }
    // Restored matches wait for every player (`MatchPause`).
    m.started = false;
    Ok(m)
}

/// Writes atomically, as the Java mod does: a temporary file moved over the old one.
pub fn save(world_dir: &Path, m: &Match) -> Result<(), String> {
    let path = path(world_dir);
    let tmp = world_dir.join(format!("{FILE_NAME}.tmp"));
    let text = serde_json::to_string_pretty(&to_json(m)).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn load(world_dir: &Path) -> Option<Result<Match, String>> {
    let text = std::fs::read_to_string(path(world_dir)).ok()?;
    Some(
        serde_json::from_str::<Value>(&text)
            .map_err(|e| e.to_string())
            .and_then(|v| from_json(&v)),
    )
}

pub fn delete(world_dir: &Path) {
    let _ = std::fs::remove_file(path(world_dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::teams_of_players;

    #[test]
    fn uuids_read_back() {
        let u = 0x0123_4567_89ab_cdef_0011_2233_4455_6677u128;
        assert_eq!(uuid_string(u), "01234567-89ab-cdef-0011-223344556677");
        assert_eq!(parse_uuid(&uuid_string(u)), Some(u));
    }

    #[test]
    fn a_match_round_trips_and_reads_the_java_layout() {
        let teams = teams_of_players(
            &["Alex".into(), "Steve".into()],
            &[(1, "Alex".into()), (2, "Steve".into())],
            false,
        )
        .unwrap();
        let mut m = Match::new(
            teams,
            vec![
                ("ENTER_END".into(), "null".into()),
                ("KILL_COLORED_SHEEP".into(), "red".into()),
            ],
            0,
        );
        m.ticks = 4321;
        m.started = true;
        m.board[1].completed_by = Some(1);
        m.teams[1].points = 1;
        m.progress.insert(
            (0, 0),
            Progress {
                seen: ["minecraft:zombie".to_string()].into(),
                count: 2.5,
            },
        );
        let back = from_json(&to_json(&m)).unwrap();
        assert_eq!(back.ticks, 4321);
        assert!(!back.started, "restored matches start paused");
        assert_eq!(back.board[1].completed_by, Some(1));
        assert_eq!(back.teams[1].points, 1);
        assert_eq!(back.teams[0].players, vec![(1, "Alex".to_string())]);
        assert_eq!(back.progress[&(0, 0)].count, 2.5);
        // A file the Fabric mod wrote: Java tracker fields, no Horizon progress.
        let java = json!({
            "version": 1, "ticks": 100, "started": true,
            "teams": [{"color": "red", "points": 0, "players": [{"uuid": "00000000-0000-0000-0000-000000000001", "name": "Alex"}]}],
            "goals": [{"id": "ENTER_END", "data": "null", "completedBy": -1}],
            "trackers": {"mobsKilled": [[0, 3]]}
        });
        let m = from_json(&java).unwrap();
        assert_eq!(m.teams[0].players[0].0, 1);
        assert_eq!(m.board[0].id, "ENTER_END");
    }
}
