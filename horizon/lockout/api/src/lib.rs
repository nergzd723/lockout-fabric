//! Lockout for Horizon. The launcher links this crate into the game and calls [`hooks`].
//!
//! One mod object serves both of Horizon's games: the server's (single player's server
//! thread, a LAN host, a dedicated server), which runs the match, and the window's, which
//! draws the board and the screens. Both speak the Fabric mod's payloads (`wire`), so either
//! side also plays with the Fabric mod on the other.
pub mod board;
pub mod client;
pub mod compass;
pub mod draft;
pub mod goals;
pub mod pause;
pub mod persist;
pub mod screens;
pub mod server;
pub mod wire;

use horizon_client::mod_api::*;
use server::{Action, Event, Online, Span, State};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use wire::Payload;

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Lockout::default())
}

#[derive(Default)]
struct Lockout {
    server: Mutex<State>,
    client: Mutex<client::Client>,
    /// What the server learned of each player's death before it is told as one event:
    /// living_death gives the damage, player_died the death message's key.
    deaths: Mutex<HashMap<u128, PendingDeath>>,
    /// Animal entity id -> the player who fed it, so a birth credits the breeder.
    lovers: Mutex<HashMap<u32, u128>>,
    /// Players who started drinking milk with an effect on them.
    milk: Mutex<HashMap<u128, bool>>,
    /// Player entity ids, refreshed each tick, for the hooks that have no host to ask.
    entity_players: Mutex<HashMap<i32, u128>>,
    /// Events heard where no host was at hand, handled on the next server tick.
    queued: Mutex<Vec<Event>>,
    /// Frozen players (scouting, paused): damage to them is refused, as the Java mod makes
    /// them invulnerable.
    frozen: Mutex<HashSet<u128>>,
    /// Whether each player holds jump, from the input their client sends (scouting).
    jumping: Mutex<HashMap<u128, bool>>,
    /// The window's own jump key, for the player of a single player or LAN-hosting game,
    /// whose input the server isn't sent.
    local_jump: AtomicBool,
    /// The window's client answered the Lockout version (in this process).
    local_answered: AtomicBool,
    /// Players who haven't answered the version payload yet, and since when (server ticks).
    awaiting_version: Mutex<HashMap<u128, i64>>,
    /// Team chat lines heard (no host there), sent on the next tick.
    team_chat: Mutex<Vec<(u128, String, String)>>,
    /// The server's start-up work (config, a saved match, locating) is done.
    started_up: AtomicBool,
    /// Biomes and structures still to locate, one a tick.
    to_locate: Mutex<Vec<(bool, String)>>,
    pending: Mutex<Option<Request>>,
}

#[derive(Default, Clone)]
struct PendingDeath {
    damage_type: Option<String>,
    killer_kind: Option<String>,
    killer_player: Option<u128>,
    message_key: Option<String>,
}

/// A start or a draft someone asked for. Vanilla teams are read with `/team list`, which only
/// runs from the server tick, so the command leaves this for the next tick.
#[derive(Debug, Clone)]
struct Request {
    requester: Option<u128>,
    names: Vec<String>,
    by_team: bool,
    blackout: bool,
    draft: bool,
}

/// The server's commands, as the Java mod registers them (`LockoutInitializer`).
const COMMANDS: &[(&str, u8)] = &[
    ("lockout teams <team_names:text>", 2),
    ("lockout players <player_names:text>", 2),
    ("lockout draft teams <team_names:text>", 2),
    ("lockout draft players <player_names:text>", 2),
    ("lockout draft cancel", 2),
    ("blackout team <team_name:text>", 2),
    ("blackout players <player_names:text>", 2),
    ("blackout draft team <team_name:text>", 2),
    ("blackout draft players <player_names:text>", 2),
    ("blackout draft cancel", 2),
    ("chat team", 0),
    ("chat local", 0),
    ("GiveGoal <player_name:player_name> <goal_number:integer>", 2),
    ("SetStartTime <seconds:integer>", 2),
    ("RemoveCustomBoard", 2),
    (
        "SetDraftSettings <offer_size:integer> <pick_seconds:integer> <rerolls:integer>",
        2,
    ),
    ("SetScoutingTime <seconds:integer>", 2),
    ("SetBoardSize <board_size:integer>", 2),
];

const RED: u32 = 0xFF5555;
/// The Java server kicks a player whose client doesn't answer its version payload; Horizon
/// can't send the ping it races against, so it waits this long instead.
const VERSION_TIMEOUT_TICKS: i64 = 100;
const LOCATE_SEARCH: i32 = 750;

fn online(host: &dyn Host) -> Vec<NativeServerPlayer> {
    host.server_players()
}

fn roster(players: &[NativeServerPlayer]) -> Vec<Online> {
    players
        .iter()
        .map(|p| Online {
            uuid: p.uuid,
            name: p.name.clone(),
            dimension: p.dimension.clone(),
            position: p.position,
            xp_level: p.xp_level,
        })
        .collect()
}

fn uuid_of_entity(players: &[NativeServerPlayer], entity: i32) -> Option<u128> {
    (entity != 0)
        .then(|| players.iter().find(|p| p.entity_id == entity).map(|p| p.uuid))
        .flatten()
}

fn kind_of(host: &dyn Host, entity: u32) -> Option<String> {
    host.entity(entity).and_then(|e| host.entity_kind(e))
}

fn text_of(spans: &[Span]) -> Text {
    let mut out = Text::empty();
    for s in spans {
        let mut t = Text::colored(s.text.clone(), s.color);
        if s.italic {
            t = t.italic();
        }
        out = out.append(t);
    }
    out
}

/// `{id:"minecraft:compass", PlayerTracker custom data, "Tracking: <name>"}`.
fn compass_snbt(name: &str) -> String {
    let name = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{id:\"minecraft:compass\",count:1,components:{{\"minecraft:custom_data\":{{{}:\"lockout\"}},\"minecraft:custom_name\":{{text:\"{name}\",italic:0b,color:\"white\"}}}}}}",
        compass::TRACKER_KEY
    )
}

fn is_tracker(stack: &ItemStack) -> bool {
    &*stack.item == "minecraft:compass"
        && stack
            .custom_data()
            .is_some_and(|d| d.iter().any(|(k, _)| *k == compass::TRACKER_KEY.into()))
}

fn send(host: &mut dyn Host, player: u128, payload: &Payload) {
    let _ = host.send_custom_payload_to_player(player, payload.channel(), &payload.encode());
}

fn stack_view(stack: &ItemStack) -> server::Held {
    let extra = stack.extra.as_ref();
    server::Held {
        item: stack.item.to_string(),
        count: stack.count,
        potion: extra.and_then(|e| e.potion.as_deref().map(str::to_owned)),
        dyed: extra.and_then(|e| e.dyed_color),
    }
}

/// Brigadier's bounds messages (`argument.integer.low` / `.big`).
fn bounded(arg: Option<&str>, min: i64, max: i64) -> Result<i64, String> {
    let v: i64 = arg
        .and_then(|a| a.parse().ok())
        .ok_or_else(|| "Invalid integer".to_owned())?;
    if v < min {
        return Err(format!("Integer must not be less than {min}, found {v}"));
    }
    if v > max {
        return Err(format!("Integer must not be more than {max}, found {v}"));
    }
    Ok(v)
}

impl Lockout {
    /// Hands `event` to the match and carries out what it answers.
    fn event(&self, host: &mut dyn Host, event: Event) {
        let players: Vec<u128> = online(host).iter().map(|p| p.uuid).collect();
        let actions = match self.server.try_lock() {
            Ok(mut s) => s.handle(event, &players),
            Err(_) => return,
        };
        self.apply(host, actions, false);
    }

    /// Carries out `actions`. Payloads go at once; the rest waits for the server's tick
    /// unless this is it: inside one player's hook, the other players are out of reach.
    fn apply(&self, host: &mut dyn Host, actions: Vec<Action>, in_tick: bool) {
        let mut players: Option<Vec<NativeServerPlayer>> = None;
        let mut later = Vec::new();
        for action in actions {
            let all = players.get_or_insert_with(|| online(host)).clone();
            let name_of = |u: u128| all.iter().find(|p| p.uuid == u).map(|p| p.name.clone());
            match action {
                Action::Send { player, payload } => send(host, player, &payload),
                Action::SendAll { payload } => {
                    for p in &all {
                        send(host, p.uuid, &payload);
                    }
                }
                Action::Freeze { player, on, .. } if !in_tick => {
                    // Damage is refused at once; the movement freeze waits for the tick.
                    if let Ok(mut f) = self.frozen.lock() {
                        if on {
                            f.insert(player)
                        } else {
                            f.remove(&player)
                        };
                    }
                    later.push(Action::Freeze {
                        player,
                        modifier: "",
                        on,
                    });
                }
                other if !in_tick => later.push(other),
                Action::Tell { player, spans } => {
                    host.tell(player, text_of(&spans));
                }
                Action::Broadcast(spans) => host.broadcast(text_of(&spans)),
                Action::ActionBar { player, spans } => {
                    host.action_bar(player, text_of(&spans));
                }
                Action::Command(line) => {
                    host.run_command(&line);
                }
                Action::GameMode { player, mode } => {
                    host.set_game_mode(player, mode);
                }
                Action::Sound {
                    player,
                    sound,
                    volume,
                    pitch,
                } => {
                    host.play_sound_to(player, sound, "master", None, volume, pitch);
                }
                Action::Teleport { player, dimension, pos } => {
                    let (yaw, pitch) = all
                        .iter()
                        .find(|p| p.uuid == player)
                        .map_or((0.0, 0.0), |p| (p.yaw, p.pitch));
                    host.teleport_player(player, &dimension, pos, yaw, pitch);
                }
                Action::Flight { player, allow, flying } => {
                    host.set_flight(player, allow, flying);
                }
                Action::FlySpeed { player, speed } => {
                    host.set_flying_speed(player, speed);
                }
                Action::Freeze { player, modifier, on } => {
                    if let Ok(mut f) = self.frozen.lock() {
                        if on {
                            f.insert(player)
                        } else {
                            f.remove(&player)
                        };
                    }
                    if let Some(name) = name_of(player) {
                        let id = if modifier.is_empty() {
                            draft::SCOUTING_FREEZE
                        } else {
                            modifier
                        };
                        for attribute in ["minecraft:movement_speed", "minecraft:jump_strength"] {
                            if on {
                                host.run_command(&format!(
                                    "attribute {name} {attribute} modifier add {id} -1 add_multiplied_total"
                                ));
                            } else {
                                for id in [draft::SCOUTING_FREEZE, pause::PAUSE_FREEZE] {
                                    host.run_command(&format!("attribute {name} {attribute} modifier remove {id}"));
                                }
                            }
                        }
                    }
                }
                Action::Give { player, item } => {
                    let _ = host.give_stack(player, ItemStack::new(&item, 1));
                }
                Action::ClearItem { player, item } => {
                    if let Some(name) = name_of(player) {
                        host.run_command(&format!("clear {name} {item}"));
                    }
                }
                Action::GiveCompass { player, slot, name } => {
                    if let Some(stack) = host.item_from_snbt(&compass_snbt(&name)) {
                        match slot {
                            Some(slot) => {
                                let _ = host.set_inventory_slot(
                                    player,
                                    NativeInventoryKind::Player,
                                    slot,
                                    None,
                                    Some(stack),
                                );
                            }
                            None => {
                                let _ = host.give_stack(player, stack);
                            }
                        }
                    }
                }
                Action::CompassTarget { player, target } => {
                    host.set_compass_target(player, target.as_ref().map(|(d, p)| (d.as_str(), *p)));
                }
                Action::RenameCompass { player, name } => self.rename_compass(host, player, &name),
                Action::Save => self.save(host),
                Action::DeleteSave => {
                    if let Some(dir) = host.world_dir() {
                        persist::delete(&dir);
                    }
                }
                Action::Reset { player } => {
                    if let Some(name) = name_of(player) {
                        for c in [
                            format!("clear {name}"),
                            format!("effect clear {name}"),
                            format!("advancement revoke {name} everything"),
                        ] {
                            host.run_command(&c);
                        }
                    }
                    host.set_player_status(player, Some(f32::MAX), Some((20, 5.0, 0.0)), Some((0, 0.0, 0)));
                    host.set_player_stats(player, &serde_json::json!({"stats": {}}));
                    if let Some(e) = all
                        .iter()
                        .find(|p| p.uuid == player)
                        .and_then(|p| host.entity(p.entity_id as u32))
                    {
                        host.entity_set_fire(e, 0);
                    }
                }
                Action::SnapshotSprint => {
                    let base: HashMap<u128, i32> = all
                        .iter()
                        .map(|p| {
                            (
                                p.uuid,
                                host.player_stat(p.uuid, "minecraft:custom", "minecraft:sprint_one_cm")
                                    .unwrap_or(0),
                            )
                        })
                        .collect();
                    if let Ok(mut s) = self.server.lock() {
                        s.sprint_base = base;
                    }
                }
            }
        }
        if !later.is_empty()
            && let Ok(mut s) = self.server.lock()
        {
            s.deferred.extend(later);
        }
    }

    fn save(&self, host: &mut dyn Host) {
        let Some(dir) = host.world_dir() else { return };
        let Ok(mut s) = self.server.lock() else { return };
        let Some(m) = s.game.as_ref().filter(|m| m.running) else {
            return;
        };
        match persist::save(&dir, m) {
            Ok(()) => s.saved = true,
            Err(e) => host.log("WARN", &format!("Could not save the Lockout match: {e}")),
        }
    }

    /// Names the player's tracking compass after whom it tracks.
    fn rename_compass(&self, host: &mut dyn Host, player: u128, name: &str) {
        let Some(renamed) = host.item_from_snbt(&compass_snbt(name)) else {
            return;
        };
        let mut slots: Vec<(usize, ItemStack)> = host
            .player_stacks(player)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
            .collect();
        if let Some(off) = host.player_equipment(player, EquipmentSlot::OffHand) {
            slots.push((40, off));
        }
        for (slot, stack) in slots {
            if is_tracker(&stack) {
                let _ = host.set_inventory_slot(
                    player,
                    NativeInventoryKind::Player,
                    slot,
                    Some(&stack),
                    Some(renamed.clone()),
                );
            }
        }
    }

    fn command(&self, host: &mut dyn Host, requester: Option<u128>, line: &str) -> bool {
        let words: Vec<&str> = line.trim_start_matches('/').split_whitespace().collect();
        let Some(first) = words.first() else { return false };
        let rest = |n: usize| words.iter().skip(n).map(|w| w.to_string()).collect::<Vec<_>>();
        let reply = |host: &mut dyn Host, text: String| host.feedback(Text::literal(text));
        let fail = |host: &mut dyn Host, text: String| host.feedback(Text::colored(text, RED));
        let request = |names: Vec<String>, by_team: bool, blackout: bool, draft: bool| Request {
            requester,
            names,
            by_team,
            blackout,
            draft,
        };
        let w = |i: usize| words.get(i).copied();
        let req = match (*first, w(1), w(2)) {
            ("lockout", Some("teams"), _) => Some(request(rest(2), true, false, false)),
            ("lockout", Some("players"), _) => Some(request(rest(2), false, false, false)),
            ("lockout", Some("draft"), Some("teams")) => Some(request(rest(3), true, false, true)),
            ("lockout", Some("draft"), Some("players")) => Some(request(rest(3), false, false, true)),
            ("blackout", Some("team"), _) => Some(request(rest(2), true, true, false)),
            ("blackout", Some("players"), _) => Some(request(rest(2), false, true, false)),
            ("blackout", Some("draft"), Some("team")) => Some(request(rest(3), true, true, true)),
            ("blackout", Some("draft"), Some("players")) => Some(request(rest(3), false, true, true)),
            ("lockout" | "blackout", Some("draft"), Some("cancel")) => {
                let actions = self.server.lock().map(|mut s| s.cancel_draft()).unwrap_or_default();
                if actions.is_empty() {
                    fail(host, "No draft is running.".into());
                }
                self.apply(host, actions, false);
                return true;
            }
            ("chat", Some(kind @ ("team" | "local")), _) => {
                let Some(player) = requester else {
                    fail(host, "This is a player-only command.".into());
                    return true;
                };
                let team = kind == "team";
                let changed = self.server.lock().is_ok_and(|mut s| {
                    if team {
                        s.team_chat.insert(player)
                    } else {
                        s.team_chat.remove(&player)
                    }
                });
                let name = if team { "TEAM" } else { "LOCAL" };
                if changed {
                    reply(host, format!("You are now chatting in {name}."));
                } else {
                    reply(host, format!("You are already chatting in {name}."));
                }
                return true;
            }
            ("GiveGoal", Some(name), number) => {
                let number = match bounded(number, 1, (server::MAX_BOARD_SIZE * server::MAX_BOARD_SIZE) as i64) {
                    Ok(n) => n as usize,
                    Err(e) => {
                        fail(host, e);
                        return true;
                    }
                };
                let players = online(host);
                let target = players
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(name))
                    .map(|p| (p.uuid, p.name.clone()));
                let uuids: Vec<u128> = players.iter().map(|p| p.uuid).collect();
                let result = self.server.lock().map(|mut s| s.give_goal(target, number, &uuids));
                match result {
                    Ok(Ok((text, actions))) => {
                        reply(host, text);
                        self.apply(host, actions, false);
                    }
                    Ok(Err(e)) => fail(host, e),
                    Err(_) => {}
                }
                return true;
            }
            ("SetStartTime", Some(n), _) => {
                match bounded(Some(n), 5, 300) {
                    Ok(v) => {
                        if let Ok(mut s) = self.server.lock() {
                            s.settings.start_seconds = v;
                        }
                        reply(host, format!("Updated start time to {v}s."));
                    }
                    Err(e) => fail(host, e),
                }
                return true;
            }
            ("SetBoardSize", Some(n), _) => {
                match bounded(Some(n), server::MIN_BOARD_SIZE as i64, server::MAX_BOARD_SIZE as i64) {
                    Ok(v) => {
                        if let Ok(mut s) = self.server.lock() {
                            s.settings.board_size = v as usize;
                        }
                        reply(host, format!("Updated board size to {v}."));
                    }
                    Err(e) => fail(host, e),
                }
                return true;
            }
            ("SetDraftSettings", Some(a), Some(b)) => {
                let parsed = bounded(Some(a), 1, 5)
                    .and_then(|o| bounded(Some(b), 3, 120).and_then(|p| bounded(w(3), 0, 10).map(|r| (o, p, r))));
                match parsed {
                    Ok((offer, pick, rerolls)) => {
                        if let Ok(mut s) = self.server.lock() {
                            s.settings.draft.offer_size = offer as usize;
                            s.settings.draft.pick_seconds = pick as i32;
                            s.settings.draft.rerolls = rerolls as i32;
                        }
                        reply(
                            host,
                            format!(
                                "Drafts now offer {offer} goals per pick, with {pick}s per pick and {rerolls} reroll(s) per team."
                            ),
                        );
                    }
                    Err(e) => fail(host, e),
                }
                return true;
            }
            ("SetScoutingTime", Some(n), _) => {
                match bounded(Some(n), 0, 60) {
                    Ok(v) => {
                        if let Ok(mut s) = self.server.lock() {
                            s.settings.draft.scouting_seconds = v as i32;
                        }
                        reply(
                            host,
                            if v == 0 {
                                "Drafts start without scouting.".into()
                            } else {
                                format!("Drafts start with {v}s of scouting.")
                            },
                        );
                    }
                    Err(e) => fail(host, e),
                }
                return true;
            }
            ("RemoveCustomBoard", _, _) => {
                if let Ok(mut s) = self.server.lock() {
                    s.custom_board = None;
                }
                reply(host, "Removed custom board.".into());
                return true;
            }
            _ => return false,
        };
        if let (Some(req), Ok(mut pending)) = (req, self.pending.lock()) {
            *pending = Some(req);
        }
        true
    }

    /// `ServerStartedEventHandler` and `initializeServer`: the config's defaults, the match
    /// a restart interrupted, and what has to be located near spawn.
    fn start_up(&self, host: &mut dyn Host) {
        let config = client::read_config(host);
        if let Ok(mut s) = self.server.lock() {
            let n = |k: &str| config.get(k).and_then(|v| v.as_i64());
            if let Some(v) = n("default board size") {
                s.settings.board_size = (v as usize).clamp(server::MIN_BOARD_SIZE, server::MAX_BOARD_SIZE);
            }
            if let Some(v) = n("draft offer size") {
                s.settings.draft.offer_size = v.clamp(1, 5) as usize;
            }
            if let Some(v) = n("draft pick seconds") {
                s.settings.draft.pick_seconds = v.clamp(3, 120) as i32;
            }
            if let Some(v) = n("draft rerolls") {
                s.settings.draft.rerolls = v.clamp(0, 10) as i32;
            }
            if let Some(v) = n("draft scouting seconds") {
                s.settings.draft.scouting_seconds = v.clamp(0, 60) as i32;
            }
        }
        // `LockoutConfig.load` writes the file with every key when it is missing.
        let defaults = [
            ("default board size", serde_json::json!(5)),
            ("board position", serde_json::json!("right")),
            ("show NoiseRouter line", serde_json::json!(false)),
            ("draft offer size", serde_json::json!(2)),
            ("draft pick seconds", serde_json::json!(10)),
            ("draft rerolls", serde_json::json!(1)),
            ("draft scouting seconds", serde_json::json!(15)),
        ];
        for (k, v) in defaults {
            if config.get(k).is_none() {
                client::write_config(host, k, v);
            }
        }
        let table = goals::table();
        if let Ok(mut l) = self.to_locate.lock() {
            l.extend(goals::biomes_to_locate(table).into_iter().map(|b| (true, b)));
            l.extend(goals::structures_to_locate(table).into_iter().map(|s| (false, s)));
            l.reverse();
        }
        if let Some(dir) = host.world_dir() {
            match persist::load(&dir) {
                Some(Ok(m)) => {
                    host.log(
                        "INFO",
                        &format!(
                            "Restored the {} match from before the restart. It resumes once every player is back.",
                            m.mode_name()
                        ),
                    );
                    if let Ok(mut s) = self.server.lock() {
                        s.restore(m);
                    }
                }
                Some(Err(e)) => host.log("WARN", &format!("Could not restore the saved Lockout match: {e}")),
                None => {}
            }
        }
    }

    /// One `/locate`: whether the target is within the Java mod's search radius of spawn.
    fn locate(&self, host: &mut dyn Host, biome: bool, id: &str) {
        let result = host.run_command_captured(&format!("locate {} {id}", if biome { "biome" } else { "structure" }));
        let found = result.ran
            && result.output.first().is_some_and(|line| {
                let c = line.component();
                let ok = c
                    .get("translate")
                    .and_then(|v| v.as_str())
                    .is_some_and(|k| k.ends_with(".success"));
                let distance = c
                    .get("with")
                    .and_then(|w| w.as_array())
                    .and_then(|w| w.get(2))
                    .map(plain_of);
                ok && distance
                    .and_then(|d| d.trim().parse::<i32>().ok())
                    .is_some_and(|d| d < LOCATE_SEARCH)
            });
        if let Ok(mut s) = self.server.lock() {
            let world = s.world.get_or_insert_with(board::World::default);
            if found {
                if biome {
                    world.biomes.insert(id.to_owned())
                } else {
                    world.structures.insert(id.to_owned())
                };
            }
        }
    }

    /// Locates what is left, `limit` at most (all of it before a board is made).
    fn locate_some(&self, host: &mut dyn Host, limit: usize) -> bool {
        for _ in 0..limit {
            let next = self.to_locate.lock().ok().and_then(|mut l| l.pop());
            match next {
                Some((biome, id)) => self.locate(host, biome, &id),
                None => {
                    if let Ok(mut s) = self.server.lock() {
                        s.world.get_or_insert_with(board::World::default);
                    }
                    return true;
                }
            }
        }
        self.to_locate.lock().is_ok_and(|l| l.is_empty())
    }

    /// The server's tick: start-up, actions left for it, a start waiting for its teams, the
    /// version check, team chat, the match.
    fn server_tick(&self, host: &mut dyn Host) {
        if !self.started_up.swap(true, Ordering::SeqCst) {
            self.start_up(host);
        }
        self.locate_some(host, 1);
        let deferred = match self.server.lock() {
            Ok(mut s) => std::mem::take(&mut s.deferred),
            Err(_) => return,
        };
        self.apply(host, deferred, true);
        let pending = self.pending.lock().ok().and_then(|mut p| p.take());
        if let Some(request) = pending {
            self.locate_some(host, usize::MAX);
            self.begin(host, request);
        }
        self.version_check(host);
        self.send_team_chat(host);
        // Deaths heard last tick, now that both halves are in.
        let deaths: Vec<(u128, PendingDeath)> = self.deaths.lock().map(|mut d| d.drain().collect()).unwrap_or_default();
        for (player, d) in deaths {
            self.event(
                host,
                Event::Death {
                    player,
                    damage_type: d.damage_type.unwrap_or_else(|| "minecraft:generic".into()),
                    killer_kind: d.killer_kind,
                    killer_player: d.killer_player,
                    message_key: d.message_key,
                },
            );
        }
        let queued: Vec<Event> = self
            .queued
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for e in queued {
            self.event(host, e);
        }
        let players = online(host);
        if let Ok(mut m) = self.entity_players.lock() {
            *m = players
                .iter()
                .filter(|p| p.entity_id != 0)
                .map(|p| (p.entity_id, p.uuid))
                .collect();
        }
        let list = roster(&players);
        let scouts = self.scouts(host, &players);
        let actions = match self.server.lock() {
            Ok(mut s) => {
                let now = s.server_ticks;
                s.last_hurt_by.retain(|_, (_, t)| now - *t < 200);
                s.tick(&list, &scouts)
            }
            Err(_) => return,
        };
        self.apply(host, actions, true);
    }

    /// The drafting players' positions and jump keys, for scouting.
    fn scouts(&self, host: &dyn Host, players: &[NativeServerPlayer]) -> Vec<draft::Scout> {
        let scouting = self
            .server
            .try_lock()
            .is_ok_and(|s| s.draft.as_ref().is_some_and(|d| d.scouting.is_some()));
        if !scouting {
            return Vec::new();
        }
        let jumping = self.jumping.lock().map(|j| j.clone()).unwrap_or_default();
        let local = self.local_jump.load(Ordering::Relaxed);
        players
            .iter()
            .map(|p| draft::Scout {
                uuid: p.uuid,
                dimension: p.dimension.clone(),
                position: p.position,
                // A player the server gets no input from is the window's own.
                jumping: jumping.get(&p.uuid).copied().unwrap_or(local),
                has_spyglass: host
                    .player_stacks(p.uuid)
                    .unwrap_or_default()
                    .iter()
                    .flatten()
                    .any(|s| &*s.item == "minecraft:spyglass"),
            })
            .collect()
    }

    fn version_check(&self, host: &mut dyn Host) {
        let now = self.server.lock().map(|s| s.server_ticks).unwrap_or(0);
        let late: Vec<u128> = match self.awaiting_version.lock() {
            Ok(mut w) => {
                let late = w
                    .iter()
                    .filter(|(_, t)| now - **t > VERSION_TIMEOUT_TICKS)
                    .map(|(u, _)| *u)
                    .collect::<Vec<_>>();
                for u in &late {
                    w.remove(u);
                }
                late
            }
            Err(_) => return,
        };
        let players = online(host);
        for uuid in late {
            // The window's own player sends nothing: its client answered in this process.
            let local = players
                .iter()
                .find(|p| p.uuid == uuid)
                .is_some_and(|p| p.address.is_empty() || p.address.starts_with("local"));
            if local && self.local_answered.load(Ordering::Relaxed) || local {
                continue;
            }
            host.kick(
                uuid,
                &Text::literal(format!(
                    "Missing Lockout mod.\nServer is using Lockout v{}.",
                    wire::LOCKOUT_VERSION
                )),
            );
        }
    }

    /// `AllowChatMessageEventHandler`: team chat goes to the sender's Lockout team, or to
    /// their vanilla team before a match.
    fn send_team_chat(&self, host: &mut dyn Host) {
        let lines: Vec<(u128, String, String)> = self
            .team_chat
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for (sender, name, message) in lines {
            let line = |color: u32| {
                vec![
                    Span::new("[Team Chat] ", color),
                    Span::new(format!("<{name}> {message}"), 0xFFFFFF),
                ]
            };
            let lockout = self.server.lock().ok().and_then(|s| {
                let m = s.game.as_ref().filter(|m| m.running)?;
                let t = &m.teams[m.team_of(sender)?];
                Some((t.rgb(), t.players.iter().map(|(u, _)| *u).collect::<Vec<_>>()))
            });
            let (color, members) = match lockout {
                Some(team) => team,
                None => {
                    let Some(team) = vanilla_team_of(host, &name) else {
                        // No team: the line goes to everyone, as chat does.
                        host.broadcast(Text::literal(format!("<{name}> {message}")));
                        continue;
                    };
                    let color = team.color.as_deref().map_or(0xFFFFFF, wire::color_rgb);
                    let players = online(host);
                    let members = players
                        .iter()
                        .filter(|p| team.members.iter().any(|m| m.eq_ignore_ascii_case(&p.name)))
                        .map(|p| p.uuid)
                        .collect();
                    (color, members)
                }
            };
            for m in members {
                host.tell(m, text_of(&line(color)));
            }
        }
    }

    fn begin(&self, host: &mut dyn Host, request: Request) {
        let players = online(host);
        let list = roster(&players);
        let names: Vec<(u128, String)> = list.iter().map(|p| (p.uuid, p.name.clone())).collect();
        let teams = if request.by_team {
            let listed: Vec<Option<server::ListedTeam>> = request.names.iter().map(|n| list_team(host, n)).collect();
            server::teams_from(&request.names, &listed, &names, request.blackout).map(|(teams, recolored)| {
                for (team, color) in recolored {
                    host.run_command(&format!("team modify {team} color {color}"));
                }
                teams
            })
        } else {
            server::teams_of_players(&request.names, &names, request.blackout)
        };
        let teams = match teams {
            Ok(t) => t,
            Err(e) => {
                match request.requester {
                    Some(p) => {
                        host.tell(p, Text::colored(e, RED));
                    }
                    None => host.log("WARN", &e),
                }
                return;
            }
        };
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        if request.draft {
            let scouts: Vec<draft::Scout> = players
                .iter()
                .map(|p| draft::Scout {
                    uuid: p.uuid,
                    dimension: p.dimension.clone(),
                    position: p.position,
                    jumping: false,
                    has_spyglass: host
                        .player_stacks(p.uuid)
                        .unwrap_or_default()
                        .iter()
                        .flatten()
                        .any(|s| &*s.item == "minecraft:spyglass"),
                })
                .collect();
            let actions = self
                .server
                .lock()
                .map(|mut s| s.start_draft(teams, &list, &scouts, seed))
                .unwrap_or_default();
            self.apply(host, actions, true);
            return;
        }
        let actions = {
            let Ok(mut s) = self.server.lock() else { return };
            let mut actions = s.cancel_draft();
            let board = match &s.custom_board {
                Some(b) => Some(b.clone()),
                None => {
                    let world = s.world.clone().unwrap_or_default();
                    board::generate(
                        goals::table(),
                        s.settings.board_size,
                        teams.len(),
                        &world,
                        &mut board::Rng::new(seed),
                    )
                }
            };
            let Some(board) = board else {
                drop(s);
                if let Some(p) = request.requester {
                    host.tell(p, Text::colored("Not enough goals for a board this size.", RED));
                }
                return;
            };
            actions.extend(s.start(teams, board, &list, &HashMap::new()));
            actions.push(Action::SnapshotSprint);
            actions
        };
        self.apply(host, actions, true);
    }
}

/// A vanilla team's members and colour, read with `/team list <name>`; None when there is no
/// such team.
fn list_team(host: &mut dyn Host, name: &str) -> Option<server::ListedTeam> {
    let result = host.run_command_captured(&format!("team list {name}"));
    let line = result.output.first()?;
    if !result.ran
        && result.result == 0
        && line
            .translation_key()
            .is_some_and(|k| !k.starts_with("commands.team.list"))
    {
        return None;
    }
    let component = line.component();
    let key = component.get("translate").and_then(|v| v.as_str()).unwrap_or("");
    if !key.starts_with("commands.team.list.members") {
        return None;
    }
    let args = component
        .get("with")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let color = args.first().and_then(find_color);
    let members = if key.ends_with("members.empty") {
        Vec::new()
    } else {
        let text = args.get(2).map(plain_of).unwrap_or_else(|| {
            line.plain()
                .rsplit_once(": ")
                .map(|(_, m)| m.to_owned())
                .unwrap_or_default()
        });
        text.split(',')
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty())
            .collect()
    };
    Some(server::ListedTeam {
        name: name.to_owned(),
        color,
        members,
    })
}

/// The vanilla team a player is on: `/team list`, then each team's members.
fn vanilla_team_of(host: &mut dyn Host, player: &str) -> Option<server::ListedTeam> {
    let result = host.run_command_captured("team list");
    let line = result.output.first()?;
    let component = line.component();
    let names = component
        .get("with")
        .and_then(|w| w.as_array())
        .and_then(|w| w.get(1))
        .map(plain_of)?;
    names
        .split(',')
        .map(|n| n.trim().trim_start_matches('[').trim_end_matches(']').to_owned())
        .filter(|n| !n.is_empty())
        .filter_map(|n| list_team(host, &n))
        .find(|t| t.members.iter().any(|m| m.eq_ignore_ascii_case(player)))
}

fn find_color(v: &serde_json::Value) -> Option<String> {
    if let Some(c) = v.get("color").and_then(|c| c.as_str()) {
        return Some(c.to_owned());
    }
    let children = v
        .get("extra")
        .into_iter()
        .chain(v.get("with"))
        .filter_map(|e| e.as_array())
        .flatten();
    children.filter_map(find_color).next()
}

fn plain_of(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(a) => a.iter().map(plain_of).collect(),
        serde_json::Value::Object(o) => {
            let mut s = o.get("text").and_then(|t| t.as_str()).unwrap_or("").to_owned();
            if let Some(with) = o.get("with").and_then(|w| w.as_array()) {
                s.push_str(&with.iter().map(plain_of).collect::<Vec<_>>().join(", "));
            }
            if let Some(extra) = o.get("extra") {
                s.push_str(&plain_of(extra));
            }
            s
        }
        _ => String::new(),
    }
}

impl Hooks for Lockout {
    // ---- the server -------------------------------------------------------------------

    fn register_commands(&self, _host: &mut dyn Host, event: &mut RegisterCommandsEvent<'_>) {
        for (usage, level) in COMMANDS {
            event.commands.push(if *level == 0 {
                NativeCommand::new(usage)
            } else {
                NativeCommand::op(usage, *level)
            });
        }
    }

    fn perform_command(&self, host: &mut dyn Host, event: &mut PerformCommandEvent<'_>) -> Flow {
        if self.command(host, Some(event.player.uuid), event.command) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn perform_console_command(&self, host: &mut dyn Host, event: &mut PerformConsoleCommandEvent<'_>) -> Flow {
        if self.command(host, None, event.command) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn tick(&self, host: &mut dyn Host, _event: &mut TickEvent) {
        if host.window() {
            if let Ok(mut c) = self.client.lock() {
                client::tick(&mut c, host);
            }
        } else {
            self.server_tick(host);
        }
    }

    fn server_stopping(&self, host: &mut dyn Host, _event: &mut ServerStoppingEvent) {
        // Drafts aren't restored; a running match is saved.
        let actions = self.server.lock().map(|mut s| s.cancel_draft()).unwrap_or_default();
        self.apply(host, actions, true);
        self.save(host);
        if let Ok(mut s) = self.server.lock() {
            let settings = s.settings.clone();
            *s = State {
                settings,
                ..State::default()
            };
        }
        self.started_up.store(false, Ordering::SeqCst);
        if let Ok(mut l) = self.to_locate.lock() {
            l.clear();
        }
    }

    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        let channels: Vec<&str> = wire::CLIENTBOUND.iter().chain(wire::SERVERBOUND).copied().collect();
        let _ = host.send_custom_payload_to_player(event.player, "minecraft:register", &wire::register_body(&channels));
        send(
            host,
            event.player,
            &Payload::Version {
                version: wire::LOCKOUT_VERSION.into(),
            },
        );
        let now = self.server.lock().map(|s| s.server_ticks).unwrap_or(0);
        if let Ok(mut w) = self.awaiting_version.lock() {
            w.insert(event.player, now);
        }
        // A frozen player who left keeps no freeze: the pause freezes them again.
        if let Ok(mut f) = self.frozen.lock() {
            f.remove(&event.player);
        }
        if let Ok(mut s) = self.server.lock() {
            s.deferred.push(Action::Freeze {
                player: event.player,
                modifier: "",
                on: false,
            });
        }
        let list = roster(&online(host));
        let actions = self
            .server
            .lock()
            .map(|s| s.join(event.player, &list))
            .unwrap_or_default();
        self.apply(host, actions, false);
    }

    fn player_logged_out(&self, _host: &mut dyn Host, event: &mut PlayerLoggedOutEvent<'_>) {
        if let Ok(mut w) = self.awaiting_version.lock() {
            w.remove(&event.player);
        }
        if let Ok(mut s) = self.server.lock()
            && let Some(p) = &mut s.pause
        {
            p.left(event.player);
        }
    }

    fn custom_payload_from_client(&self, host: &mut dyn Host, event: &mut CustomPayloadFromClientEvent<'_>) {
        match Payload::decode(event.channel, event.data) {
            Some(Ok(Payload::Version { version })) => {
                if let Ok(mut w) = self.awaiting_version.lock() {
                    w.remove(&event.player);
                }
                if version != wire::LOCKOUT_VERSION {
                    host.kick(
                        event.player,
                        &Text::literal(format!(
                            "Wrong Lockout version: v{version}.\nServer is using Lockout v{}.",
                            wire::LOCKOUT_VERSION
                        )),
                    );
                }
            }
            Some(Ok(Payload::DraftAction { action })) => {
                let players = online(host);
                let list = roster(&players);
                let scouts = self.scouts(host, &players);
                let actions = self
                    .server
                    .lock()
                    .map(|mut s| s.draft_action(event.player, action, &list, &scouts))
                    .unwrap_or_default();
                self.apply(host, actions, true);
            }
            Some(Ok(Payload::CustomBoard { goals })) => {
                if host.player_permission(event.player).unwrap_or(0) < 2 {
                    host.tell(
                        event.player,
                        Text::colored("You do not have the permission for this command!", RED),
                    );
                    return;
                }
                let reply = match goals {
                    None => {
                        if let Ok(mut s) = self.server.lock() {
                            s.custom_board = None;
                        }
                        "Removed custom board.".to_owned()
                    }
                    Some(goals) => {
                        let invalid = server::invalid_goals(&goals);
                        if invalid.is_empty() {
                            if let Ok(mut s) = self.server.lock() {
                                s.custom_board = Some(goals);
                            }
                            "Set custom board.".to_owned()
                        } else {
                            format!("Invalid board. Could not create goals:\n{}", invalid.join("\n"))
                        }
                    }
                };
                host.tell(event.player, Text::literal(reply));
            }
            _ => {}
        }
    }

    fn player_input(&self, _host: &mut dyn Host, event: &mut PlayerInputEvent<'_>) {
        // `PlayerInput`'s flags: forward 1, backward 2, left 4, right 8, jump 16.
        if let Ok(mut j) = self.jumping.lock() {
            j.insert(event.player, event.flags & 16 != 0);
        }
    }

    fn player_chat_v2(&self, event: &mut PlayerChatV2Event<'_>) -> Flow {
        let team = self
            .server
            .try_lock()
            .is_ok_and(|s| s.team_chat.contains(&event.player));
        if !team {
            return Flow::Continue;
        }
        if let Ok(mut q) = self.team_chat.lock() {
            q.push((event.player, event.name.to_owned(), event.message.to_owned()));
        }
        Flow::Cancel
    }

    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        if is_tracker(event.stack)
            && let Some(p) = host.acting_player()
        {
            let actions = self
                .server
                .lock()
                .map(|mut s| {
                    let running = s.game.as_ref().is_some_and(|m| m.running);
                    match &mut s.compass {
                        Some(c) if running => c.cycle(p.uuid),
                        _ => Vec::new(),
                    }
                })
                .unwrap_or_default();
            self.apply(host, actions, false);
        }
        Flow::Continue
    }

    fn item_dropped_v2(&self, _host: &mut dyn Host, event: &mut ItemDroppedV2Event<'_>) -> Flow {
        let playing = self.server.try_lock().is_ok_and(|s| {
            s.game
                .as_ref()
                .is_some_and(|m| m.running && m.team_of(event.player).is_some())
        });
        if playing && is_tracker(event.stack) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn player_respawned(&self, host: &mut dyn Host, event: &mut PlayerRespawnedEvent<'_>) {
        let actions = self
            .server
            .lock()
            .map(|s| {
                let Some(m) = s.game.as_ref().filter(|m| m.running && !m.solo_blackout()) else {
                    return Vec::new();
                };
                if m.team_of(event.player).is_none() {
                    return Vec::new();
                }
                let Some(c) = &s.compass else { return Vec::new() };
                let slot = c.slots.get(&event.player).copied().unwrap_or(0);
                vec![Action::GiveCompass {
                    player: event.player,
                    slot: Some(slot),
                    name: c.label(event.player),
                }]
            })
            .unwrap_or_default();
        self.apply(host, actions, false);
    }

    fn player_inventory_changed(&self, host: &mut dyn Host, event: &mut PlayerInventoryChangedEvent<'_>) {
        let stacks = event.stacks.iter().map(|s| s.as_ref().map(stack_view)).collect();
        self.event(
            host,
            Event::Inventory {
                player: event.player,
                stacks,
            },
        );
    }

    fn player_advancement_done(&self, host: &mut dyn Host, event: &mut PlayerAdvancementDoneEvent<'_>) {
        self.event(
            host,
            Event::Advancement {
                player: event.player,
                id: event.advancement.to_owned(),
            },
        );
    }

    fn living_incoming_damage_v3(&self, event: &mut LivingIncomingDamageV3Event<'_>) -> Flow {
        if let Some(victim) = event.victim_uuid
            && self.frozen.lock().is_ok_and(|f| f.contains(&victim))
        {
            return Flow::Cancel;
        }
        if let (Some(attacker), Ok(mut s)) = (event.attacker_uuid, self.server.try_lock()) {
            let now = s.server_ticks;
            s.last_hurt_by.insert(event.victim, (attacker, now));
        }
        Flow::Continue
    }

    fn living_damage_post(&self, event: &mut LivingDamagePostEvent<'_>) {
        if event.amount <= 0.0 {
            return;
        }
        let (victim, attacker) = match self.entity_players.lock() {
            Ok(m) => (m.get(&event.entity).copied(), m.get(&event.causing).copied()),
            Err(_) => return,
        };
        if (victim.is_some() || attacker.is_some())
            && let Ok(mut q) = self.queued.lock()
        {
            q.push(Event::Damage {
                victim,
                attacker,
                damage_type: event.damage_type.to_owned(),
                amount: event.amount,
            });
        }
    }

    fn living_death(&self, host: &mut dyn Host, event: &mut LivingDeathEvent<'_>) -> Flow {
        let players = online(host);
        let hurt_by = self
            .server
            .try_lock()
            .ok()
            .and_then(|s| s.last_hurt_by.get(&event.entity).map(|(p, _)| *p));
        if event.kind == "minecraft:player" {
            if let Some(uuid) = uuid_of_entity(&players, event.entity as i32) {
                let killer_kind = (event.attacker != 0).then(|| kind_of(host, event.attacker)).flatten();
                if let Ok(mut d) = self.deaths.lock() {
                    let entry = d.entry(uuid).or_default();
                    entry.damage_type = Some(event.damage_type.to_owned());
                    entry.killer_kind = killer_kind;
                    entry.killer_player = hurt_by.filter(|k| *k != uuid);
                }
            }
            return Flow::Continue;
        }
        if let Some(player) = hurt_by {
            let color = (event.kind == "minecraft:sheep")
                .then(|| sheep_color(host, event.entity))
                .flatten();
            let dimension = players.iter().find(|p| p.uuid == player).map(|p| p.dimension.clone());
            self.event(
                host,
                Event::Kill {
                    player,
                    kind: event.kind.to_owned(),
                    color,
                    dimension,
                },
            );
        }
        Flow::Continue
    }

    fn player_died(&self, host: &mut dyn Host, event: &mut PlayerDiedEvent<'_>) {
        if let Ok(mut d) = self.deaths.lock() {
            d.entry(event.player).or_default().message_key = event.cause.map(str::to_owned);
        }
        // Where the tracking compass was, so it comes back there (`ServerPlayerEntityMixin.onDeath`).
        let mut slot = host
            .player_stacks(event.player)
            .unwrap_or_default()
            .iter()
            .position(|s| s.as_ref().is_some_and(is_tracker));
        if slot.is_none()
            && host
                .player_equipment(event.player, EquipmentSlot::OffHand)
                .as_ref()
                .is_some_and(is_tracker)
        {
            slot = Some(40);
        }
        if let (Some(slot), Ok(mut s)) = (slot, self.server.try_lock())
            && let Some(c) = &mut s.compass
        {
            c.slots.insert(event.player, slot);
        }
    }

    fn entity_enter_love_mode(&self, _host: &mut dyn Host, event: &mut EntityEnterLoveModeEvent<'_>) {
        if let (Some(p), Ok(mut l)) = (event.player, self.lovers.lock()) {
            l.insert(event.entity, p);
        }
    }

    fn entity_breed(&self, host: &mut dyn Host, event: &mut EntityBreedEvent) {
        let player = self
            .lovers
            .lock()
            .ok()
            .and_then(|l| l.get(&event.parent).or(l.get(&event.partner)).copied());
        if let (Some(player), Some(kind)) = (player, kind_of(host, event.parent)) {
            self.event(host, Event::Breed { player, kind });
        }
    }

    fn entity_tame(&self, host: &mut dyn Host, event: &mut EntityTameEvent<'_>) {
        if let Some(player) = event.owner {
            self.event(
                host,
                Event::Tame {
                    player,
                    kind: event.kind.to_owned(),
                },
            );
        }
    }

    fn mob_effect_added(&self, host: &mut dyn Host, event: &mut MobEffectAddedEvent<'_>) {
        let players = online(host);
        if let Some(player) = event.entity.and_then(|e| uuid_of_entity(&players, e)) {
            self.event(
                host,
                Event::EffectAdded {
                    player,
                    effect: event.effect.to_owned(),
                },
            );
        }
    }

    fn living_entity_use_item_start(&self, host: &mut dyn Host, event: &mut LivingEntityUseItemStartEvent<'_>) -> Flow {
        let players = online(host);
        let Some(player) = event.entity.and_then(|e| uuid_of_entity(&players, e)) else {
            return Flow::Continue;
        };
        let item = event.stack.item.to_string();
        if item == "minecraft:milk_bucket" {
            let has_effects = host.effects(player).is_some_and(|e| !e.is_empty());
            if let Ok(mut m) = self.milk.lock() {
                m.insert(player, has_effects);
            }
        }
        self.event(host, Event::UsedItem { player, item });
        Flow::Continue
    }

    fn living_entity_use_item_finish(&self, host: &mut dyn Host, event: &mut LivingEntityUseItemFinishEvent<'_>) {
        let players = online(host);
        let Some(player) = event.entity.and_then(|e| uuid_of_entity(&players, e)) else {
            return;
        };
        let server::Held { item, potion, .. } = stack_view(event.stack);
        if item == "minecraft:milk_bucket"
            && self
                .milk
                .lock()
                .ok()
                .and_then(|mut m| m.remove(&player))
                .unwrap_or(false)
        {
            self.event(host, Event::MilkAway { player });
        }
        self.event(host, Event::Consumed { player, item, potion });
    }

    fn player_stat_award(&self, host: &mut dyn Host, event: &mut PlayerStatAwardEvent<'_>) -> Flow {
        if event.value > 0 {
            self.event(
                host,
                Event::Stat {
                    player: event.player,
                    stat: event.stat.to_owned(),
                },
            );
        }
        Flow::Continue
    }

    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        if let Some(player) = host.acting_player() {
            let block = host.block(event.pos);
            self.event(
                host,
                Event::Mined {
                    player: player.uuid,
                    block,
                },
            );
        }
        Flow::Continue
    }

    fn player_changed_dimension(&self, host: &mut dyn Host, event: &mut PlayerChangedDimensionEvent<'_>) {
        self.event(
            host,
            Event::Dimension {
                player: event.player,
                dimension: event.to.to_owned(),
            },
        );
    }

    fn player_level_change(&self, host: &mut dyn Host, event: &mut PlayerLevelChangeEvent<'_>) {
        self.event(
            host,
            Event::XpLevel {
                player: event.player,
                level: event.to,
            },
        );
    }

    fn item_crafted(&self, host: &mut dyn Host, event: &mut ItemCraftedEvent<'_>) {
        self.event(
            host,
            Event::Crafted {
                player: event.player,
                item: event.stack.item.to_string(),
            },
        );
    }

    fn entity_mount(&self, host: &mut dyn Host, event: &mut EntityMountEvent<'_>) -> Flow {
        if !event.mounting || event.kind != "minecraft:player" {
            return Flow::Continue;
        }
        let players = online(host);
        if let Some(player) = uuid_of_entity(&players, event.entity as i32) {
            self.event(
                host,
                Event::Ride {
                    player,
                    vehicle: event.vehicle_kind.to_owned(),
                },
            );
        }
        Flow::Continue
    }

    fn player_name_entity(&self, host: &mut dyn Host, event: &mut PlayerNameEntityEvent<'_>) {
        if let Some(kind) = kind_of(host, event.entity_id) {
            self.event(
                host,
                Event::Named {
                    player: event.player,
                    kind,
                    name: event.entity_name.to_owned(),
                },
            );
        }
    }

    fn player_destroy_item(&self, host: &mut dyn Host, event: &mut PlayerDestroyItemEvent<'_>) {
        self.event(
            host,
            Event::ItemBroken {
                player: event.player,
                item: event.item.to_owned(),
            },
        );
    }

    fn block_place(&self, host: &mut dyn Host, event: &mut BlockPlaceEvent<'_>) -> Flow {
        self.event(
            host,
            Event::PlacedBlock {
                player: event.player,
                item: event.stack.item.to_string(),
            },
        );
        Flow::Continue
    }

    fn hanging_place(&self, host: &mut dyn Host, event: &mut HangingPlaceEvent<'_>) -> Flow {
        self.event(
            host,
            Event::PlacedHanging {
                player: event.player,
                kind: event.kind.to_owned(),
            },
        );
        Flow::Continue
    }

    fn food_level_changed(&self, host: &mut dyn Host, event: &mut FoodLevelChangedEvent<'_>) -> Flow {
        self.event(
            host,
            Event::Food {
                player: event.player,
                from: event.from,
                to: event.to,
            },
        );
        Flow::Continue
    }

    fn living_change_target(&self, event: &mut LivingChangeTargetEvent<'_>) -> Flow {
        if let (Some(player), Ok(mut q)) = (event.target, self.queued.lock()) {
            q.push(Event::Enraged {
                player,
                kind: event.kind.to_owned(),
            });
        }
        Flow::Continue
    }

    fn player_shield_disable(&self, host: &mut dyn Host, event: &mut PlayerShieldDisableEvent<'_>) {
        self.event(host, Event::ShieldDisabled { player: event.player });
    }

    fn player_item_frame_change(&self, host: &mut dyn Host, event: &mut PlayerItemFrameChangeEvent<'_>) {
        if event.item.ends_with("item_frame") {
            self.event(host, Event::ItemFrameInFrame { player: event.player });
        }
    }

    fn interact_entity_v2(&self, host: &mut dyn Host, event: &mut InteractEntityV2Event<'_>) -> Flow {
        let wolf_armor =
            event.kind == "minecraft:wolf" && event.stack.is_some_and(|s| &*s.item == "minecraft:wolf_armor");
        if let (true, Some(player)) = (wolf_armor, host.acting_player()) {
            self.event(host, Event::WolfArmor { player: player.uuid });
        }
        Flow::Continue
    }

    fn player_stonecutter_recipe_select(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerStonecutterRecipeSelectEvent<'_>,
    ) {
        self.event(host, Event::Stonecutter { player: event.player });
    }

    fn player_loom_pattern_select(&self, host: &mut dyn Host, event: &mut PlayerLoomPatternSelectEvent<'_>) {
        self.event(host, Event::Loom { player: event.player });
    }

    fn anvil_craft_post(&self, host: &mut dyn Host, event: &mut AnvilCraftPostEvent<'_>) {
        self.event(host, Event::Anvil { player: event.player });
    }

    fn projectile_impact(&self, host: &mut dyn Host, event: &mut ProjectileImpactEvent<'_>) -> Flow {
        if event.entity_kind == "minecraft:player" {
            let players = online(host);
            if let Some(victim) = event.entity.and_then(|e| uuid_of_entity(&players, e as i32)) {
                self.event(
                    host,
                    Event::HitBy {
                        victim,
                        projectile: event.kind.to_owned(),
                    },
                );
            }
        }
        Flow::Continue
    }

    fn item_picked_up_v2(&self, host: &mut dyn Host, event: &mut ItemPickedUpV2Event<'_>) -> Flow {
        self.event(
            host,
            Event::PickedUp {
                player: event.player,
                thrower: event.thrower.filter(|t| *t != event.player),
            },
        );
        Flow::Continue
    }

    /// Twice a second for each player: where they are, their effects and their sprinting.
    fn player_tick(&self, host: &mut dyn Host, _event: &mut PlayerTickEvent) {
        let Some(p) = host.acting_player() else { return };
        let ticks = match self.server.try_lock() {
            Ok(s) if s.game.as_ref().is_some_and(|m| m.started && m.running) => s.server_ticks,
            _ => return,
        };
        if (ticks + (p.uuid as i64 & 0xF)) % 10 != 0 {
            return;
        }
        let pos = [
            p.position[0].floor() as i32,
            p.position[1].floor() as i32,
            p.position[2].floor() as i32,
        ];
        let biome = host.biome(pos);
        let below = Some(host.block([pos[0], pos[1] - 1, pos[2]]));
        let me = host.entity(p.entity_id as u32);
        let in_water = me.and_then(|e| host.entity_is_in_water(e)).unwrap_or(false);
        let on_fire = me.and_then(|e| host.entity_fire_ticks(e)).is_some_and(|t| t > 0);
        self.event(
            host,
            Event::Position {
                player: p.uuid,
                dimension: p.dimension.clone(),
                y: p.position[1],
                biome,
                below,
                in_water,
                on_fire,
            },
        );
        let effects: Vec<String> = host
            .effects(p.uuid)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();
        self.event(
            host,
            Event::Effects {
                player: p.uuid,
                effects,
            },
        );
        let sprint = host
            .player_stat(p.uuid, "minecraft:custom", "minecraft:sprint_one_cm")
            .unwrap_or(0);
        let base = self
            .server
            .try_lock()
            .ok()
            .and_then(|s| s.sprint_base.get(&p.uuid).copied())
            .unwrap_or(0);
        self.event(
            host,
            Event::SprintCm {
                player: p.uuid,
                cm: sprint - base,
            },
        );
    }

    // ---- the window -------------------------------------------------------------------

    fn client_packet(&self, host: &mut dyn Host, event: &mut ClientPacketEvent<'_>) {
        let Ok(mut c) = self.client.lock() else { return };
        match event.packet {
            ClientboundPlayPacket::Login(login) => {
                c.entity_id = Some(login.player_id);
                c.name = None;
            }
            ClientboundPlayPacket::CustomPayload(p) => {
                if c.template.is_none() {
                    // The serverbound packet carries the same struct; building it from the one
                    // heard keeps this crate off horizon-protocol, which mods can't depend on.
                    c.template = Some(ServerboundPlayPacket::CustomPayload(p.clone()));
                }
                if p.channel == wire::VERSION {
                    self.local_answered.store(true, Ordering::Relaxed);
                }
                client::packet(&mut c, host, &p.channel, &p.data);
            }
            _ => {}
        }
    }

    fn commands(&self, _host: &mut dyn Host, event: &mut CommandsEvent<'_>) {
        event.commands.extend(client::COMMANDS);
    }

    fn command(&self, host: &mut dyn Host, event: &mut CommandEvent<'_>) -> Flow {
        let Ok(mut c) = self.client.lock() else {
            return Flow::Continue;
        };
        if client::command(&mut c, host, event.command) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        // The board key (`key.lockout.open_board`, B): the board during a match, else the
        // Board Builder.
        if !event.pressed || !host.playing() {
            return;
        }
        let key = host.mod_key(client::MOD_ID, "open_board", "key.keyboard.b");
        if event.key != key {
            return;
        }
        let Ok(mut c) = self.client.lock() else { return };
        let screen = if c.board.is_some() {
            screens::Screen::Board
        } else {
            screens::Screen::Builder
        };
        screens::open(&mut c, host, screen);
    }

    fn movement_input(&self, _host: &dyn Host, event: &mut MovementInputEvent) {
        self.local_jump.store(event.jump, Ordering::Relaxed);
        if self.client.try_lock().is_ok_and(|c| client::hold_still(&c)) {
            event.forward = 0.0;
            event.strafe = 0.0;
            event.jump = false;
        }
    }

    fn toast_add(&self, _host: &mut dyn Host, _event: &mut ToastAddEvent<'_>) -> Flow {
        // The Java client hides every toast while there is a board.
        if self.client.try_lock().is_ok_and(|c| c.board.is_some()) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn gui_layer_pre(&self, host: &mut dyn Host, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if event.layer == "minecraft:effects"
            && let Some(dx) = self.client.try_lock().ok().and_then(|c| client::effects_offset(&c))
        {
            host.gui_paint(
                event.batch,
                &[GuiDraw::Push(vec![horizon_client::hud_layout::PoseOp::Translate(
                    -dx, 0.0,
                )])],
            );
        }
        Flow::Continue
    }

    fn gui_layer_post(&self, host: &mut dyn Host, event: &mut GuiLayerPostEvent<'_>) {
        if event.layer == "minecraft:effects"
            && self
                .client
                .try_lock()
                .ok()
                .and_then(|c| client::effects_offset(&c))
                .is_some()
        {
            host.gui_paint(event.batch, &[GuiDraw::Pop]);
        }
    }

    fn render_hand_pose(&self, _host: &dyn Host, event: &mut RenderHandPoseEvent<'_>) {
        // `ItemInHandRendererMixin`: the tracking compass is held still, without the arm's
        // equip and swing movement.
        if event.stack.is_some_and(is_tracker) {
            let side = if event.x < 0.0 { -1.0 } else { 1.0 };
            event.x = side * 0.56;
            event.y = -0.52;
            event.z = -0.72;
        }
    }

    fn gui_render_post(&self, host: &dyn Host, event: &mut GuiRenderPostEvent<'_>) {
        let Ok(mut c) = self.client.lock() else { return };
        if !event.gui_hidden {
            if c.board.is_some() {
                client::draw_hud(&mut c, host, event.batch);
            } else {
                // Single player and a LAN host run the match in this process: draw it from
                // there if no payload has reached the window.
                let local = self
                    .server
                    .try_lock()
                    .ok()
                    .and_then(|s| s.game.as_ref().map(|m| (m.goals_teams_payload(), m.ticks, m.started)));
                if let Some((payload, ticks, started)) = local {
                    c.apply(payload);
                    c.apply(Payload::UpdateTimer { ticks });
                    if started {
                        c.apply(Payload::Start);
                    }
                    client::draw_hud(&mut c, host, event.batch);
                }
            }
        }
    }

    fn screen_render_post(&self, host: &dyn Host, event: &mut ScreenRenderPostEvent<'_>) {
        if let Ok(mut c) = self.client.lock() {
            screens::draw(&mut c, host, event.batch);
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn Host,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let Ok(mut c) = self.client.lock() else {
            return Flow::Continue;
        };
        if screens::mouse_pressed(&mut c, host, event.button) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_mouse_scrolled_pre(&self, host: &dyn Host, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
        let Ok(mut c) = self.client.lock() else {
            return Flow::Continue;
        };
        if screens::scrolled(&mut c, host, event.scroll[1]) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_key_pressed_pre(&self, host: &dyn Host, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        let Ok(mut c) = self.client.lock() else {
            return Flow::Continue;
        };
        if screens::key(&mut c, host, event.key) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_char_typed_pre(&self, host: &dyn Host, event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
        let Ok(mut c) = self.client.lock() else {
            return Flow::Continue;
        };
        if screens::typed(&mut c, host, event.c) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_closing(&self, host: &dyn Host, _event: &mut ScreenClosingEvent<'_>) {
        if host.mod_screen() == Some(client::MOD_ID)
            && let Ok(mut c) = self.client.lock()
        {
            screens::closed(&mut c);
        }
    }

    fn client_resources_reloaded(&self, _host: &mut dyn Host, _event: &mut ClientResourcesReloadedEvent) {
        if let Ok(mut c) = self.client.lock() {
            client::resources_reloaded(&mut c);
        }
    }

    fn client_disconnecting(&self, _host: &dyn Host, _event: &mut ClientDisconnectingEvent) {
        if let Ok(mut c) = self.client.lock() {
            c.reset();
        }
        self.local_answered.store(false, Ordering::Relaxed);
    }
}

/// A sheep's wool colour from its data: a name in 26.x saves, a dye id in older ones.
fn sheep_color(host: &mut dyn Host, entity: u32) -> Option<String> {
    let e = host.entity(entity)?;
    let nbt = host.entity_nbt(e)?;
    let (_, tag) = nbt.iter().find(|(k, _)| **k == "Color".into())?;
    match tag {
        Tag::Byte(b) => goals::DYES.get(*b as usize).map(|d| d.to_string()),
        Tag::String(s) => Some(String::from_utf16_lossy(&s.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A server with nobody on it that records what the mod sends and says.
    #[derive(Default)]
    struct Recorder {
        sent: Vec<(u128, String, Vec<u8>)>,
        feedback: Vec<String>,
        told: Vec<(u128, String)>,
        logged: RefCell<Vec<String>>,
    }

    impl Host for Recorder {
        fn send_custom_payload_to_player(&mut self, player: u128, channel: &str, data: &[u8]) -> Result<(), String> {
            self.sent.push((player, channel.to_owned(), data.to_vec()));
            Ok(())
        }
        fn feedback(&mut self, line: Text) {
            self.feedback.push(line.plain());
        }
        fn tell(&mut self, uuid: u128, line: Text) -> bool {
            self.told.push((uuid, line.plain()));
            true
        }
        fn player_permission(&self, _player: u128) -> Option<u8> {
            Some(2)
        }
        fn server_players(&self) -> Vec<NativeServerPlayer> {
            Vec::new()
        }
        fn window(&self) -> bool {
            false
        }
        fn log(&self, _level: &str, line: &str) {
            self.logged.borrow_mut().push(line.to_owned());
        }
        fn config_dir(&self) -> Result<std::path::PathBuf, String> {
            Err("no config here".into())
        }
        fn world_dir(&self) -> Option<std::path::PathBuf> {
            None
        }
        fn run_command(&mut self, _line: &str) -> bool {
            true
        }
        fn run_command_captured(&mut self, _line: &str) -> NativeCommandResult {
            NativeCommandResult::default()
        }
    }

    fn mods() -> [Box<dyn Hooks>; 1] {
        [hooks()]
    }

    #[test]
    fn a_joining_player_is_told_the_channels_and_the_version() {
        let mods = mods();
        let mut host = Recorder::default();
        dispatch_player_logged_in(&mut host, &mods, 7, "Alex");
        assert_eq!(host.sent[0].1, "minecraft:register");
        assert!(String::from_utf8_lossy(&host.sent[0].2).contains("lockout:lockout_goals_teams\0"));
        let (_, channel, data) = &host.sent[1];
        assert_eq!(
            Payload::decode(channel, data),
            Some(Ok(Payload::Version {
                version: wire::LOCKOUT_VERSION.into()
            }))
        );
    }

    #[test]
    fn console_commands_change_the_settings() {
        let mods = mods();
        let mut host = Recorder::default();
        let mut ran = false;
        dispatch_perform_console_command(&mut host, &mods, "SetBoardSize 9", &mut ran);
        assert!(ran);
        assert_eq!(host.feedback, vec!["Integer must not be more than 7, found 9"]);
        let mut ran = false;
        dispatch_perform_console_command(&mut host, &mods, "SetBoardSize 7", &mut ran);
        assert_eq!(host.feedback[1], "Updated board size to 7.");
        let mut ran = false;
        dispatch_perform_console_command(&mut host, &mods, "gamerule doDaylightCycle false", &mut ran);
        assert!(!ran, "other commands are left to vanilla");
    }

    #[test]
    fn a_start_with_an_unknown_player_is_refused_on_the_next_tick() {
        let mods = mods();
        let mut host = Recorder::default();
        let mut ran = false;
        dispatch_perform_console_command(&mut host, &mods, "lockout players Alex Steve", &mut ran);
        assert!(ran);
        dispatch_tick(&mut host, &mods);
        assert_eq!(host.logged.borrow().as_slice(), ["Player Alex is invalid."]);
    }

    #[test]
    fn a_fabric_client_can_set_a_custom_board() {
        let mods = mods();
        let mut host = Recorder::default();
        let board = Payload::CustomBoard {
            goals: Some(vec![("ENTER_END".into(), "null".into()); 9]),
        };
        dispatch_custom_payload_from_client(&mut host, &mods, 3, wire::CUSTOM_BOARD, &board.encode());
        assert_eq!(host.told, vec![(3, "Set custom board.".to_owned())]);
        let bad = Payload::CustomBoard {
            goals: Some(vec![("NOT_A_GOAL".into(), "null".into()); 9]),
        };
        dispatch_custom_payload_from_client(&mut host, &mods, 3, wire::CUSTOM_BOARD, &bad.encode());
        assert!(host.told[1].1.starts_with("Invalid board."));
    }
}
