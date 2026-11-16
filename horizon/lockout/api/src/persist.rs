//! `MatchPersistence`: the running match is saved in the world folder (`lockout-match.json`)
//! every 30 seconds and when the server stops, and restored when it starts again.
//!
//! The file has the Java mod's layout (`version`, `ticks`, `started`, `teams` with colour,
//! points and players, `goals` with id, data and `completedBy`, and `trackers`: `Lockout`'s
//! progress fields as `MatchPersistence` writes them, maps as `[key, value]` pairs, teams by
//! index, UUIDs as strings), so a world keeps its match, progress included, when it moves
//! between a Fabric and a Horizon server.
use crate::rules::Trackers;
use crate::server::{Match, Team};
use crate::wire::TEAM_COLORS;
use horizon_client::mod_api::serde_json::{self, Value, json};
use std::collections::HashMap;
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
    json!({
        "version": FORMAT_VERSION,
        "ticks": m.ticks,
        "started": m.started,
        "teams": teams,
        "goals": goals,
        "trackers": trackers_to_json(&m.trackers),
    })
}

fn by_team<V: Clone + Into<Value>>(map: &HashMap<usize, V>) -> Value {
    let mut pairs: Vec<(&usize, &V)> = map.iter().collect();
    pairs.sort_by_key(|(k, _)| **k);
    Value::Array(pairs.into_iter().map(|(k, v)| json!([k, v.clone().into()])).collect())
}

fn by_player<V: Clone + Into<Value>>(pairs: impl IntoIterator<Item = (u128, V)>) -> Value {
    Value::Array(
        pairs
            .into_iter()
            .map(|(k, v)| json!([uuid_string(k), v.into()]))
            .collect(),
    )
}

pub fn trackers_to_json(t: &Trackers) -> Value {
    let uuid = |u: Option<u128>| u.map_or(Value::Null, |u| Value::String(uuid_string(u)));
    json!({
        "bredAnimalTypes": by_team(&t.bred_animal_types),
        "killedHostileTypes": by_team(&t.killed_hostile_types),
        "killedRaidMobs": by_team(&t.killed_raid_mobs),
        "killedUndeadMobs": by_team(&t.killed_undead_mobs),
        "killedArthropods": by_team(&t.killed_arthropods),
        "foodTypesEaten": by_team(&t.food_types_eaten),
        "uniqueItemsCraftedByTeam": by_team(&t.unique_items_crafted_by_team),
        "compostedFoods": by_team(&t.composted_foods),
        "spiedMobs": by_team(&t.spied_mobs),
        "iceTypesMined": by_team(&t.ice_types_mined),
        "biomesVisited": by_team(&t.biomes_visited),
        "deathTypes": by_team(&t.death_types),
        "uniqueAdvancements": by_team(&t.unique_advancements),
        "damageTaken": by_team(&t.damage_taken),
        "damageDealt": by_team(&t.damage_dealt),
        "deaths": by_team(&t.deaths),
        "mobsKilled": by_team(&t.mobs_killed),
        "pumpkinWearTime": by_player(t.pumpkin_wear_time.iter().map(|(k, v)| (*k, *v))),
        "distanceSprinted": by_player(t.distance_sprinted.iter().map(|(k, v)| (*k, *v))),
        "uniqueCrafts": by_player(t.unique_crafts.iter().map(|(k, v)| (*k, v.clone()))),
        "mostUniqueCraftsPlayer": uuid(t.most_unique_crafts_player),
        "mostUniqueCrafts": t.most_unique_crafts,
        "levels": by_player(t.levels.iter().copied()),
        "mostLevelsPlayer": uuid(t.most_levels_player),
    })
}

/// A saved field's `[key, value]` pairs.
fn pairs<'a>(v: &'a Value, field: &str) -> impl Iterator<Item = (&'a Value, &'a Value)> {
    v.get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p.get(0)?, p.get(1)?)))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

pub fn trackers_from_json(v: &Value, teams: usize) -> Trackers {
    let team = |k: &Value| k.as_u64().map(|t| t as usize).filter(|t| *t < teams);
    let sets = |field: &str| -> HashMap<usize, Vec<String>> {
        pairs(v, field)
            .filter_map(|(k, v)| Some((team(k)?, strings(v))))
            .collect()
    };
    let ints = |field: &str| -> HashMap<usize, i32> {
        pairs(v, field)
            .filter_map(|(k, v)| Some((team(k)?, v.as_i64()? as i32)))
            .collect()
    };
    let doubles = |field: &str| -> HashMap<usize, f64> {
        pairs(v, field)
            .filter_map(|(k, v)| Some((team(k)?, v.as_f64()?)))
            .collect()
    };
    let player = |k: &Value| k.as_str().and_then(parse_uuid);
    let uuid = |field: &str| v.get(field).and_then(Value::as_str).and_then(parse_uuid);
    Trackers {
        bred_animal_types: sets("bredAnimalTypes"),
        killed_hostile_types: sets("killedHostileTypes"),
        killed_raid_mobs: sets("killedRaidMobs"),
        killed_undead_mobs: ints("killedUndeadMobs"),
        killed_arthropods: ints("killedArthropods"),
        food_types_eaten: sets("foodTypesEaten"),
        unique_items_crafted_by_team: sets("uniqueItemsCraftedByTeam"),
        composted_foods: sets("compostedFoods"),
        spied_mobs: sets("spiedMobs"),
        ice_types_mined: sets("iceTypesMined"),
        biomes_visited: sets("biomesVisited"),
        death_types: sets("deathTypes"),
        unique_advancements: sets("uniqueAdvancements"),
        damage_taken: doubles("damageTaken"),
        damage_dealt: doubles("damageDealt"),
        deaths: ints("deaths"),
        mobs_killed: ints("mobsKilled"),
        pumpkin_wear_time: pairs(v, "pumpkinWearTime")
            .filter_map(|(k, v)| Some((player(k)?, v.as_i64()?)))
            .collect(),
        distance_sprinted: pairs(v, "distanceSprinted")
            .filter_map(|(k, v)| Some((player(k)?, v.as_i64()? as i32)))
            .collect(),
        unique_crafts: pairs(v, "uniqueCrafts")
            .filter_map(|(k, v)| Some((player(k)?, strings(v))))
            .collect(),
        most_unique_crafts_player: uuid("mostUniqueCraftsPlayer"),
        most_unique_crafts: v.get("mostUniqueCrafts").and_then(Value::as_i64).unwrap_or(0) as i32,
        levels: pairs(v, "levels")
            .filter_map(|(k, v)| Some((player(k)?, v.as_i64()? as i32)))
            .collect(),
        most_levels_player: uuid("mostLevelsPlayer"),
    }
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
    if let Some(trackers) = v.get("trackers") {
        m.trackers = trackers_from_json(trackers, m.teams.len());
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
        m.trackers
            .killed_hostile_types
            .insert(0, vec!["minecraft:zombie".into()]);
        m.trackers.damage_taken.insert(1, 2.5);
        m.trackers.levels = vec![(2, 7), (1, 3)];
        m.trackers.most_levels_player = Some(2);
        let back = from_json(&to_json(&m)).unwrap();
        assert_eq!(back.ticks, 4321);
        assert!(!back.started, "restored matches start paused");
        assert_eq!(back.board[1].completed_by, Some(1));
        assert_eq!(back.teams[1].points, 1);
        assert_eq!(back.teams[0].players, vec![(1, "Alex".to_string())]);
        assert_eq!(back.trackers, m.trackers);
        // A file the Fabric mod wrote.
        let java = json!({
            "version": 1, "ticks": 100, "started": true,
            "teams": [{"color": "red", "points": 0, "players": [{"uuid": "00000000-0000-0000-0000-000000000001", "name": "Alex"}]}],
            "goals": [{"id": "ENTER_END", "data": "null", "completedBy": -1}],
            "trackers": {"mobsKilled": [[0, 3]], "foodTypesEaten": [[0, ["minecraft:apple"]]], "mostLevelsPlayer": null}
        });
        let m = from_json(&java).unwrap();
        assert_eq!(m.teams[0].players[0].0, 1);
        assert_eq!(m.board[0].id, "ENTER_END");
        assert_eq!(m.trackers.mobs_killed[&0], 3);
        assert_eq!(m.trackers.food_types_eaten[&0], vec!["minecraft:apple".to_string()]);
    }
}
