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
pub mod names;
pub mod pause;
pub mod persist;
pub mod rules;
pub mod screens;
pub mod server;
pub mod wire;

use goals::{Deed, Trigger};
use horizon_client::mod_api::*;
use server::{Action, Event, Online, Span, State};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
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
    /// The server's tick count, for the hooks that can't lock the state.
    now: AtomicI64,
    /// A match is running, and (`immune`) hasn't started: no player can be hurt.
    live: AtomicBool,
    immune: AtomicBool,
    /// Hits heard, waiting for `living_damage_post` to say they landed.
    hits: Mutex<HashMap<u32, Hit>>,
    /// Who last hurt each entity.
    credit: Mutex<HashMap<u32, Credit>>,
    /// Entity ids of players holding up a shield.
    blocking: Mutex<HashSet<i32>>,
    /// Each player's effects last tick.
    effects: Mutex<HashMap<u128, Vec<String>>>,
    /// The menu each player has open.
    menus: Mutex<HashMap<u128, OpenMenu>>,
    takes: Mutex<Vec<Take>>,
    checks: Mutex<Vec<Check>>,
    /// End crystals a player hit, and when.
    crystals: Mutex<HashMap<u32, (u128, i64)>>,
    /// Players an ender pearl teleported, and when.
    pearls: Mutex<HashMap<u128, i64>>,
    /// Dying players' drops about to join the level: dimension, where, whose, when.
    death_drops: Mutex<Vec<DeathDrop>>,
    /// Item entities a player dropped as they died, and whose they were.
    dropped: Mutex<HashMap<u32, u128>>,
    /// The board was drawn this frame.
    hud_drawn: AtomicBool,
    /// The local player was held still at the last check (`InputMixin`), for when the client's
    /// state is busy.
    hold: AtomicBool,
}

#[derive(Default, Clone)]
struct PendingDeath {
    damage_type: Option<String>,
    killer_kind: Option<String>,
    killer_player: Option<u128>,
    message_key: Option<String>,
    direct_kind: Option<String>,
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
    ("lockout cancel", 2),
    ("blackout team <team_name:text>", 2),
    ("blackout players <player_names:text>", 2),
    ("blackout draft team <team_name:text>", 2),
    ("blackout draft players <player_names:text>", 2),
    ("blackout draft cancel", 2),
    ("blackout cancel", 2),
    ("chat team", 0),
    ("chat local", 0),
    // Horizon takes only lowercase command words (the Java mod's `GiveGoal`... are left out of the
    // command tree, and a player's unregistered command never reaches a mod), so these are the
    // Java mod's names in lowercase, which it registers too. The console takes either case.
    ("givegoal <player_name:player_name> <goal_number:integer>", 2),
    ("setstarttime <seconds:integer>", 2),
    ("removecustomboard", 2),
    (
        "setdraftsettings <offer_size:integer> <pick_seconds:integer> <rerolls:integer>",
        2,
    ),
    ("setscoutingtime <seconds:integer>", 2),
    ("setboardsize <board_size:integer>", 2),
];

const RED: u32 = 0xFF5555;
/// The Java server kicks a player whose client doesn't answer its version payload; Horizon
/// can't send the ping it races against, so it waits this long instead.
const VERSION_TIMEOUT_TICKS: i64 = 100;
const LOCATE_SEARCH: i32 = 750;

/// The players on the server when the list was last in reach: [`online`]'s answer inside a
/// player's turn.
static LAST_ONLINE: Mutex<Vec<NativeServerPlayer>> = Mutex::new(Vec::new());

/// The players on the server. Inside a player's turn (their hook or command) Horizon's server lists
/// no one, its player list being out of reach, so this is then the list from the last time it was
/// in reach (the tick before), with the acting player as they are now: without it, what goes to
/// every player from there (a completed goal, the tooltips) reached nobody.
fn online(host: &dyn Host) -> Vec<NativeServerPlayer> {
    if let Some(me) = host.acting_player() {
        let mut list = LAST_ONLINE.lock().map(|l| l.clone()).unwrap_or_default();
        match list.iter_mut().find(|p| p.uuid == me.uuid) {
            Some(p) => *p = me,
            None => list.push(me),
        }
        return list;
    }
    let list = host.server_players();
    if let Ok(mut last) = LAST_ONLINE.lock() {
        last.clone_from(&list);
    }
    list
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
            dead: p.health <= 0.0,
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
        enchanted: extra.is_some_and(|e| !e.enchantments.is_empty()),
        base_color: extra.is_some_and(|e| e.base_color.is_some()),
        bundle: stack.item.ends_with("bundle").then(|| {
            extra
                .map(|e| e.bundle_contents.iter().map(|s| s.item.to_string()).collect())
                .unwrap_or_default()
        }),
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
                Action::Freeze { player, modifier, on } if !in_tick => {
                    // Damage is refused at once; the movement freeze waits for the tick.
                    if let Ok(mut f) = self.frozen.lock() {
                        if on {
                            f.insert(player)
                        } else {
                            f.remove(&player)
                        };
                    }
                    later.push(Action::Freeze { player, modifier, on });
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
                    category,
                    volume,
                    pitch,
                } => {
                    host.play_sound_to(player, sound, category, None, volume, pitch);
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
        // The command's name in any case: `GiveGoal` from the console (or a Fabric player's habit),
        // `givegoal` as Horizon registers it.
        let root = first.to_ascii_lowercase();
        let req = match (root.as_str(), w(1), w(2)) {
            ("lockout", Some("teams"), _) => Some(request(rest(2), true, false, false)),
            ("lockout", Some("players"), _) => Some(request(rest(2), false, false, false)),
            ("lockout", Some("draft"), Some("teams")) => Some(request(rest(3), true, false, true)),
            ("lockout", Some("draft"), Some("players")) => Some(request(rest(3), false, false, true)),
            ("blackout", Some("team"), _) => Some(request(rest(2), true, true, false)),
            ("blackout", Some("players"), _) => Some(request(rest(2), false, true, false)),
            ("blackout", Some("draft"), Some("team")) => Some(request(rest(3), true, true, true)),
            ("blackout", Some("draft"), Some("players")) => Some(request(rest(3), false, true, true)),
            ("lockout" | "blackout", Some("cancel"), None) => {
                let list = roster(&online(host));
                let actions = self.server.lock().ok().and_then(|mut s| s.cancel_match(&list));
                match actions {
                    Some(actions) => self.apply(host, actions, false),
                    None => fail(host, "No match or draft is running.".into()),
                }
                return true;
            }
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
            ("givegoal", Some(name), number) => {
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
            ("setstarttime", Some(n), _) => {
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
            ("setboardsize", Some(n), _) => {
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
            ("setdraftsettings", Some(a), Some(b)) => {
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
            ("setscoutingtime", Some(n), _) => {
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
            ("removecustomboard", _, _) => {
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
                    direct_kind: d.direct_kind,
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
        if let Ok(mut b) = self.blocking.lock() {
            *b = players.iter().filter(|p| p.blocking).map(|p| p.entity_id).collect();
        }
        let (live, polled) = self.server.try_lock().map_or((false, false), |s| {
            let live = s.game.as_ref().is_some_and(|m| m.running);
            (live, live && s.pause.is_none())
        });
        // Last tick's clicks, then `EndServerTickEventHandler`'s goal loop.
        self.run_checks(host, &players);
        if polled {
            self.poll(host, &players);
        } else if let Ok(mut e) = self.effects.lock() {
            e.clear();
        }
        let list = roster(&players);
        let scouts = self.scouts(host, &players);
        let (actions, now, ticks) = match self.server.lock() {
            Ok(mut s) => {
                let actions = s.tick(&list, &scouts);
                let ticks = s.game.as_ref().map(|m| m.ticks);
                (actions, s.server_ticks, ticks)
            }
            Err(_) => return,
        };
        self.now.store(now, Ordering::Relaxed);
        let (running, started) = self
            .server
            .try_lock()
            .map(|s| {
                let m = s.game.as_ref().filter(|m| m.running);
                (m.is_some(), m.is_some_and(|m| m.started))
            })
            .unwrap_or((live, false));
        self.live.store(running, Ordering::Relaxed);
        self.immune.store(running && !started, Ordering::Relaxed);
        if let Ok(mut c) = self.credit.lock() {
            c.retain(|_, c| {
                [c.player.map(|p| p.1), c.mob.map(|m| m.1)]
                    .into_iter()
                    .flatten()
                    .any(|t| now - t <= 100)
            });
        }
        if let Ok(mut h) = self.hits.lock() {
            h.clear();
        }
        if let Ok(mut c) = self.crystals.lock() {
            c.retain(|_, (_, t)| now - *t <= 2);
        }
        if let Ok(mut p) = self.pearls.lock() {
            p.retain(|_, t| now - *t <= 2);
        }
        if let Ok(mut d) = self.death_drops.lock() {
            d.retain(|(_, _, _, t)| now - *t <= 1);
        }
        if !running && let Ok(mut d) = self.dropped.lock() {
            d.clear();
        }
        self.apply(host, actions, true);
        if running && ticks.is_some_and(|t| t % 20 == 0) {
            self.scan_leashes(host, &players);
        }
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
            if local {
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
            actions.extend(s.start(teams, board, &list));
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

/// A dying player's drops: the dimension, where they leave, whose, and when.
type DeathDrop = (String, [f64; 3], u128, i64);

/// A hit `living_incoming_damage_v3` heard, waiting for `living_damage_post` to say it landed.
#[derive(Debug, Clone)]
struct Hit {
    /// The victim's UUID when it is a player.
    victim: Option<u128>,
    attacker: Option<u128>,
    attacker_entity: Option<u32>,
    direct: Option<u32>,
    damage_type: String,
    amount: f64,
}

/// Who last hurt an entity, and when (server ticks): `LivingEntity.lastHurtByPlayer`, the
/// mob that did (a tamed wolf credits its owner), and the hit's direct entity.
#[derive(Debug, Clone, Copy, Default)]
struct Credit {
    player: Option<(u128, i64)>,
    mob: Option<(u32, i64)>,
    direct: Option<(u32, i64)>,
}

/// The menu a player has open, and (a cartography table's) what sits in its extra slot.
#[derive(Debug, Clone)]
struct OpenMenu {
    kind: String,
    extra: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum Taken {
    /// A brewing stand's bottle.
    Potion,
    Map {
        locked: bool,
    },
    Deed(Deed),
}

/// A click on a station's result, checked on the next tick.
#[derive(Debug, Clone)]
struct Take {
    player: u128,
    taken: Taken,
    thrown: bool,
    before: HashMap<(String, Option<String>), u32>,
}

#[derive(Debug, Clone)]
enum BlockCheck {
    Composter { item: String, food: bool, level: i32 },
    Campfire,
    Bookshelf,
    Pot,
    Sign { crimson: bool },
    Loot,
}

/// What a block held before a click.
#[derive(Debug, Clone, PartialEq)]
enum BlockState {
    None,
    Level(i32),
    /// Items in it, and (a pot) the most its stack holds.
    Count(u32, u32),
    /// A sign's glowing sides.
    Glowing(bool, bool),
    Loot(String),
}

/// Something to look at on the next tick, once a click has done what it does.
#[derive(Debug, Clone)]
enum Check {
    ArmorStand {
        player: u128,
        stand: u32,
    },
    Wolf {
        player: u128,
        wolf: u32,
    },
    Allay {
        player: u128,
        allay: u32,
        cooldown: i64,
    },
    Block {
        player: u128,
        dimension: String,
        pos: [i32; 3],
        kind: BlockCheck,
        before: BlockState,
    },
}

const ABANDONED_CAMP_LOOT: [&str; 3] = [
    "minecraft:barrels/abandoned_camp_barrel",
    "minecraft:chests/abandoned_camp_common_chest",
    "minecraft:chests/abandoned_camp_secret_chest",
];

fn portal_crossing(from: &str, to: &str) -> bool {
    let pair = ["minecraft:overworld", "minecraft:the_nether"];
    from != to && pair.contains(&from) && pair.contains(&to)
}

fn tag_str(t: &Tag) -> Option<String> {
    match t {
        Tag::String(s) => Some(String::from_utf16_lossy(&s.0)),
        _ => None,
    }
}

fn tag_int(t: &Tag) -> Option<i64> {
    match t {
        Tag::Byte(v) => Some(*v as i64),
        Tag::Short(v) => Some(*v as i64),
        Tag::Int(v) => Some(*v as i64),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}

/// `[I; a, b, c, d]` as `UUIDUtil` writes one.
fn tag_uuid(t: &Tag) -> Option<u128> {
    match t {
        Tag::IntArray(v) if v.len() == 4 => {
            Some(v.iter().fold(0u128, |acc, part| (acc << 32) | (*part as u32 as u128)))
        }
        Tag::String(_) => tag_str(t).and_then(|s| persist::parse_uuid(&s)),
        _ => None,
    }
}

fn entity_tag(host: &mut dyn Host, dimension: &str, id: u32) -> Option<Tag> {
    host.entity_nbt_in(dimension, id).map(Tag::Compound)
}

/// A tamed animal's owner (`TamableAnimal.getOwner`), for a kill its wolf made.
fn tamed_owner(host: &mut dyn Host, players: &[NativeServerPlayer], id: u32) -> Option<u128> {
    let tag = match host.entity(id) {
        Some(e) => host.entity_nbt(e).map(Tag::Compound),
        None => None,
    }
    .or_else(|| {
        ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"]
            .iter()
            .find_map(|d| entity_tag(host, d, id))
    })?;
    let owner = tag.get("Owner").and_then(tag_uuid)?;
    players.iter().any(|p| p.uuid == owner).then_some(owner)
}

fn wolf_armored(host: &dyn Host, wolf: u32) -> bool {
    host.entity(wolf)
        .and_then(|e| host.living(e))
        .and_then(|l| host.living_equipment(l, EquipmentSlot::Body))
        .is_some_and(|s| &*s.item == "minecraft:wolf_armor")
}

fn allay_cooldown(host: &mut dyn Host, dimension: &str, allay: u32) -> i64 {
    entity_tag(host, dimension, allay)
        .and_then(|t| t.get("DuplicationCooldown").and_then(tag_int))
        .unwrap_or(0)
}

/// Item counts by (item, potion), for telling what a click took.
fn carried(inventory: &[Option<ItemStack>], cursor: Option<&ItemStack>) -> HashMap<(String, Option<String>), u32> {
    let mut out = HashMap::new();
    for s in inventory.iter().flatten().chain(cursor) {
        *out.entry(potion_key(s)).or_default() += s.count;
    }
    out
}

fn potion_key(s: &ItemStack) -> (String, Option<String>) {
    let held = stack_view(s);
    (held.item, held.potion)
}

/// What a block holds now, for a click's before and after.
fn block_state(host: &mut dyn Host, dimension: &str, pos: [i32; 3], kind: &BlockCheck) -> BlockState {
    let tag = host.block_entity_tag(dimension, pos);
    let items = |t: &Tag| match t.get("Items") {
        Some(Tag::List(l)) => l.len() as u32,
        _ => 0,
    };
    match kind {
        BlockCheck::Composter { .. } => BlockState::None,
        BlockCheck::Campfire | BlockCheck::Bookshelf => {
            tag.map_or(BlockState::None, |t| BlockState::Count(items(&t), 0))
        }
        BlockCheck::Pot => {
            let Some(item) = tag.as_ref().and_then(|t| t.get("item").cloned()) else {
                return BlockState::Count(0, 0);
            };
            let id = item.get("id").and_then(tag_str).unwrap_or_default();
            let count = item.get("count").and_then(tag_int).unwrap_or(1) as u32;
            let max = match host.component(&ItemStack::new(&id, 1), "minecraft:max_stack_size") {
                Ok(Some(t)) => tag_int(t).unwrap_or(64) as u32,
                _ => 64,
            };
            BlockState::Count(count, max)
        }
        BlockCheck::Sign { .. } => tag.map_or(BlockState::None, |t| {
            let glowing = |side: &str| {
                t.get(side)
                    .and_then(|s| s.get("has_glowing_text"))
                    .and_then(tag_int)
                    .is_some_and(|v| v != 0)
            };
            BlockState::Glowing(glowing("front_text"), glowing("back_text"))
        }),
        BlockCheck::Loot => tag
            .and_then(|t| t.get("LootTable").and_then(tag_str))
            .filter(|l| ABANDONED_CAMP_LOOT.contains(&l.as_str()))
            .map_or(BlockState::None, BlockState::Loot),
    }
}

/// A block that stops a look (`ClipContext.Block.COLLIDER`): anything with a collision box.
fn blocks_sight(block: &str) -> bool {
    let path = block.trim_start_matches("minecraft:");
    const OPEN: &[&str] = &[
        "air",
        "water",
        "lava",
        "bubble_column",
        "short_grass",
        "tall_grass",
        "fern",
        "large_fern",
        "dead_bush",
        "seagrass",
        "tall_seagrass",
        "kelp",
        "kelp_plant",
        "sugar_cane",
        "vine",
        "glow_lichen",
        "sculk_vein",
        "cobweb",
        "fire",
        "soul_fire",
        "light",
        "structure_void",
        "redstone_wire",
        "tripwire",
        "lever",
        "wheat",
        "carrots",
        "potatoes",
        "beetroots",
        "nether_wart",
        "sweet_berry_bush",
        "cave_vines",
        "cave_vines_plant",
        "twisting_vines",
        "twisting_vines_plant",
        "weeping_vines",
        "weeping_vines_plant",
        "hanging_roots",
        "spore_blossom",
        "pink_petals",
        "leaf_litter",
        "wildflowers",
        "bush",
        "firefly_bush",
        "short_dry_grass",
        "tall_dry_grass",
        "nether_sprouts",
        "warped_roots",
        "crimson_roots",
        "snow",
        "string",
    ];
    !(OPEN.contains(&path)
        || path.ends_with("_air")
        || path.ends_with("torch")
        || path.ends_with("_sign")
        || path.ends_with("_banner")
        || path.ends_with("_button")
        || path.ends_with("_pressure_plate")
        || path.ends_with("rail")
        || path.ends_with("_sapling")
        || path.ends_with("_tulip")
        || path.ends_with("_mushroom")
        || path.ends_with("_fungus")
        || path.ends_with("_coral")
        || path.ends_with("_coral_fan")
        || [
            "dandelion",
            "poppy",
            "blue_orchid",
            "allium",
            "azure_bluet",
            "oxeye_daisy",
            "cornflower",
            "lily_of_the_valley",
            "wither_rose",
            "torchflower",
            "pitcher_plant",
            "sunflower",
            "lilac",
            "rose_bush",
            "peony",
            "open_eyeblossom",
            "closed_eyeblossom",
            "golden_dandelion",
        ]
        .contains(&path))
}

/// The slab test: where along `from + t * (to - from)`, t in 0..=1, the ray enters `aabb`.
fn ray_hits(from: [f64; 3], to: [f64; 3], aabb: [[f64; 3]; 2]) -> Option<f64> {
    if (0..3).all(|i| from[i] >= aabb[0][i] && from[i] <= aabb[1][i]) {
        return Some(0.0);
    }
    let (mut near, mut far) = (0.0f64, 1.0f64);
    for i in 0..3 {
        let d = to[i] - from[i];
        if d.abs() < 1e-9 {
            if from[i] < aabb[0][i] || from[i] > aabb[1][i] {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((aabb[0][i] - from[i]) / d, (aabb[1][i] - from[i]) / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        near = near.max(t0);
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// The mob a player's spyglass is on (`UsingItemCriterionMixin`, the vanilla `looking_at`
/// check): the nearest entity along the look within 100 blocks, when it is a mob in sight.
fn spied_mob(host: &mut dyn Host, p: &NativeServerPlayer) -> Option<String> {
    const RANGE: f64 = 100.0;
    let (yaw, pitch) = (p.yaw.to_radians() as f64, p.pitch.to_radians() as f64);
    let view = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let eye = [p.position[0], p.position[1] + p.eye_height as f64, p.position[2]];
    let end = [
        eye[0] + view[0] * RANGE,
        eye[1] + view[1] * RANGE,
        eye[2] + view[2] * RANGE,
    ];
    let min = [0, 1, 2].map(|i| eye[i].min(end[i]) - 1.0);
    let max = [0, 1, 2].map(|i| eye[i].max(end[i]) + 1.0);
    let mut nearest: Option<(f64, u32)> = None;
    for id in host.entities_in(&p.dimension, min, max) {
        if id as i32 == p.entity_id {
            continue;
        }
        let Some(e) = host.entity(id) else { continue };
        let Some(aabb) = host.entity_bounding_box(e) else {
            continue;
        };
        if let Some(t) = ray_hits(eye, end, aabb)
            && nearest.is_none_or(|(n, _)| t < n)
        {
            nearest = Some((t, id));
        }
    }
    let (_, id) = nearest?;
    let e = host.entity(id)?;
    let kind = host.entity_kind(e)?;
    // `instanceof Mob`: living, and not a player, an armor stand or a mannequin.
    if host.living(e).is_none()
        || ["minecraft:player", "minecraft:armor_stand", "minecraft:mannequin"].contains(&kind.as_str())
    {
        return None;
    }
    let target = host.entity_eye_position(e)?;
    let level = host.entity(p.entity_id as u32).and_then(|me| host.entity_level(me))?;
    let distance = (0..3).map(|i| (target[i] - eye[i]).powi(2)).sum::<f64>().sqrt();
    let steps = (distance / 0.2).ceil() as i32;
    for s in 1..steps {
        let f = s as f64 / steps as f64;
        let at = [0, 1, 2].map(|i| (eye[i] + (target[i] - eye[i]) * f).floor() as i32);
        if host.level_block(level, at).is_some_and(|b| blocks_sight(&b)) {
            return None;
        }
    }
    Some(kind)
}

/// Mobs leashed to `player` within 16 blocks (`EndServerTickEventHandler.checkLeashedMobs`).
fn leashed_kinds(host: &mut dyn Host, p: &NativeServerPlayer) -> Vec<String> {
    let min = [p.position[0] - 16.3, p.position[1] - 16.0, p.position[2] - 16.3];
    let max = [p.position[0] + 16.3, p.position[1] + 17.8, p.position[2] + 16.3];
    let mut kinds = Vec::new();
    for id in host.entities_in(&p.dimension, min, max) {
        let Some(tag) = entity_tag(host, &p.dimension, id) else {
            continue;
        };
        let holder = tag.get("leash").and_then(|l| l.get("UUID")).and_then(tag_uuid);
        if holder == Some(p.uuid)
            && let Some(kind) = host.entity(id).and_then(|e| host.entity_kind(e))
            && !kinds.contains(&kind)
        {
            kinds.push(kind);
        }
    }
    kinds
}

/// The HUD layers drawn after the sleep overlay, in order.
const AFTER_SLEEP: [&str; 6] = [
    "minecraft:overlay_message",
    "minecraft:title",
    "minecraft:scoreboard_sidebar",
    "minecraft:subtitle_overlay",
    "minecraft:chat",
    "minecraft:tab_list",
];

impl Lockout {
    /// The board in the corner, once a frame, as a HUD layer (`Utility.drawBingoBoard`).
    fn draw_board_hud(&self, host: &dyn Host, batch: &mut HudBatch) {
        if self.hud_drawn.swap(true, Ordering::Relaxed) {
            return;
        }
        let Ok(mut c) = self.client.lock() else { return };
        if c.board.is_some() && !c.from_local {
            client::draw_hud(&mut c, host, batch);
            return;
        }
        // Single player and a LAN host run the match in this process: draw it from there if no
        // payload has reached the window.
        let local = self
            .server
            .try_lock()
            .ok()
            .and_then(|s| s.game.as_ref().map(|m| (m.goals_teams_payload(), m.ticks, m.started)));
        if let Some((payload, ticks, started)) = local {
            c.apply(payload);
            c.from_local = true;
            c.apply(Payload::UpdateTimer { ticks });
            if started {
                c.apply(Payload::Start);
            }
            client::draw_hud(&mut c, host, batch);
        }
    }

    fn did(&self, host: &mut dyn Host, player: u128, deed: Deed) {
        self.event(host, Event::Did { player, deed });
    }

    fn hit_crystal(&self, crystal: u32, player: u128) {
        if let Ok(mut c) = self.crystals.lock() {
            c.insert(crystal, (player, self.now.load(Ordering::Relaxed)));
        }
    }

    /// `LivingEntity.getKillCredit` as a player: whoever hurt it in the last 100 ticks, or
    /// the owner of the tamed animal that did, if that came later.
    fn kill_credit(&self, host: &mut dyn Host, players: &[NativeServerPlayer], entity: u32, now: i64) -> Option<u128> {
        let credit = self.credit.lock().ok()?.get(&entity).copied()?;
        let player = credit.player.filter(|(_, t)| now - t <= 100);
        let pet = credit
            .mob
            .filter(|(_, t)| now - t <= 100)
            .and_then(|(m, t)| tamed_owner(host, players, m).map(|o| (o, t)));
        match (player, pet) {
            (Some(p), Some(w)) => Some(if w.1 > p.1 { w.0 } else { p.0 }),
            (p, w) => p.or(w).map(|(u, _)| u),
        }
    }

    /// `EndServerTickEventHandler`'s per-player polling, and the effects each player gained
    /// since last tick (`StatusEffectMixin`).
    fn poll(&self, host: &mut dyn Host, players: &[NativeServerPlayer]) {
        for p in players {
            let mut stacks: Vec<Option<server::Held>> = host
                .player_stacks(p.uuid)
                .unwrap_or_default()
                .iter()
                .map(|s| s.as_ref().map(stack_view))
                .collect();
            stacks.resize(36, None);
            for slot in [
                EquipmentSlot::Feet,
                EquipmentSlot::Legs,
                EquipmentSlot::Chest,
                EquipmentSlot::Head,
                EquipmentSlot::OffHand,
            ] {
                stacks.push(host.player_equipment(p.uuid, slot).as_ref().map(stack_view));
            }
            stacks.push(host.menu_cursor(p.uuid).as_ref().map(stack_view));
            let me = host.entity(p.entity_id as u32);
            let level = me.and_then(|e| host.entity_level(e));
            let feet = [
                p.position[0].floor() as i32,
                p.position[1].floor() as i32,
                p.position[2].floor() as i32,
            ];
            let block = |host: &dyn Host, at: [i32; 3]| {
                level
                    .and_then(|l| host.level_block(l, at))
                    .unwrap_or_else(|| "minecraft:air".into())
            };
            let inside = block(host, feet);
            let below = block(host, [feet[0], feet[1] - 1, feet[2]]);
            let vehicle = host.vehicle_of(p.entity_id as u32).and_then(|v| kind_of(host, v));
            let food = host.player_food(p.uuid).map_or(20, |f| f.0);
            let on_fire = me.and_then(|e| host.entity_fire_ticks(e)).is_some_and(|t| t > 0);
            self.event(
                host,
                Event::Tick(Box::new(rules::Polled {
                    player: p.uuid,
                    name: p.name.clone(),
                    stacks,
                    selected: p.selected_slot as usize,
                    dimension: p.dimension.clone(),
                    y: p.position[1],
                    inside,
                    below,
                    vehicle,
                    food,
                    on_fire,
                })),
            );
            let now: Vec<String> = host
                .effects(p.uuid)
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.id)
                .collect();
            let before = self.effects.lock().ok().and_then(|mut e| e.insert(p.uuid, now.clone()));
            if let Some(before) = before {
                for effect in now.iter().filter(|e| !before.contains(e)) {
                    self.event(
                        host,
                        Event::EffectStarted {
                            player: p.uuid,
                            effect: effect.clone(),
                            active: now.len(),
                        },
                    );
                }
            }
        }
    }

    /// The clicks of last tick, now that they did what they do.
    fn run_checks(&self, host: &mut dyn Host, players: &[NativeServerPlayer]) {
        let checks = self
            .checks
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default();
        for check in checks {
            match check {
                Check::ArmorStand { player, stand } => {
                    let full = host.entity(stand).and_then(|e| host.living(e)).is_some_and(|l| {
                        [
                            EquipmentSlot::Head,
                            EquipmentSlot::Chest,
                            EquipmentSlot::Legs,
                            EquipmentSlot::Feet,
                        ]
                        .into_iter()
                        .all(|s| host.living_equipment(l, s).is_some())
                    });
                    if full {
                        self.did(host, player, Deed::FillArmorStand);
                    }
                }
                Check::Wolf { player, wolf } => {
                    if wolf_armored(host, wolf) {
                        self.did(host, player, Deed::WolfArmor);
                    }
                }
                Check::Allay {
                    player,
                    allay,
                    cooldown,
                } => {
                    let dimension = players
                        .iter()
                        .find(|p| p.uuid == player)
                        .map(|p| p.dimension.clone())
                        .unwrap_or_default();
                    if cooldown <= 0 && allay_cooldown(host, &dimension, allay) > 0 {
                        self.did(host, player, Deed::DuplicateAllay);
                    }
                }
                Check::Block {
                    player,
                    dimension,
                    pos,
                    kind,
                    before,
                } => {
                    if let BlockCheck::Composter { item, food, level } = &kind {
                        let now: i32 = host
                            .entity(players.iter().find(|p| p.uuid == player).map_or(0, |p| p.entity_id) as u32)
                            .and_then(|e| host.entity_level(e))
                            .and_then(|l| host.level_block_property(l, pos, "level"))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(*level);
                        if now > *level {
                            self.event(
                                host,
                                Event::Composted {
                                    player,
                                    item: item.clone(),
                                    food: *food,
                                },
                            );
                        }
                        continue;
                    }
                    let after = block_state(host, &dimension, pos, &kind);
                    let deed = match (&kind, &before, &after) {
                        (BlockCheck::Campfire, BlockState::Count(b, _), BlockState::Count(a, _))
                            if a > b && *a >= 4 =>
                        {
                            Some(Deed::FillCampfire)
                        }
                        (BlockCheck::Bookshelf, BlockState::Count(b, _), BlockState::Count(a, _))
                            if a > b && *a >= 6 =>
                        {
                            Some(Deed::FillChiseledBookshelf)
                        }
                        (BlockCheck::Pot, BlockState::Count(b, _), BlockState::Count(a, max)) if a > b && a >= max => {
                            Some(Deed::FillDecoratedPot)
                        }
                        (BlockCheck::Sign { crimson }, BlockState::Glowing(f0, b0), BlockState::Glowing(f1, b1))
                            if (*f1 && !*f0) || (*b1 && !*b0) =>
                        {
                            Some(if *crimson {
                                Deed::GlowInkCrimson
                            } else {
                                Deed::GlowInkWarped
                            })
                        }
                        (BlockCheck::Loot, BlockState::Loot(_), BlockState::None) => Some(Deed::LootAbandonedCamp),
                        _ => None,
                    };
                    if let Some(deed) = deed {
                        self.did(host, player, deed);
                    }
                }
            }
        }
        let takes = self
            .takes
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default();
        for take in takes {
            let inventory = host.player_stacks(take.player).unwrap_or_default();
            let cursor = host.menu_cursor(take.player);
            let mut after = carried(&inventory, cursor.as_ref());
            if let Some(off) = host.player_equipment(take.player, EquipmentSlot::OffHand) {
                *after.entry(potion_key(&off)).or_default() += off.count;
            }
            let gained: Vec<&(String, Option<String>)> = after
                .iter()
                .filter(|(k, n)| take.before.get(*k).is_none_or(|b| *n > b))
                .map(|(k, _)| k)
                .collect();
            if gained.is_empty() && !take.thrown {
                continue;
            }
            match take.taken {
                Taken::Potion => {
                    for (item, potion) in gained.into_iter().cloned() {
                        if item.ends_with("potion") {
                            self.event(
                                host,
                                Event::Brewed {
                                    player: take.player,
                                    item,
                                    potion,
                                },
                            );
                        }
                    }
                }
                Taken::Map { locked } => {
                    self.did(host, take.player, Deed::UseCartographyTable);
                    if locked {
                        self.did(host, take.player, Deed::LockMap);
                    }
                }
                Taken::Deed(deed) => self.did(host, take.player, deed),
            }
        }
    }

    /// Every second, the mobs each lockout player holds on a lead.
    fn scan_leashes(&self, host: &mut dyn Host, players: &[NativeServerPlayer]) {
        let wanted: Vec<u128> = match self.server.try_lock() {
            Ok(s) => match s.game.as_ref() {
                Some(m)
                    if m.board.iter().any(|sq| {
                        sq.completed_by.is_none()
                            && matches!(sq.trigger, Trigger::LeashUnique { .. } | Trigger::LeashIronGolem)
                    }) =>
                {
                    players
                        .iter()
                        .map(|p| p.uuid)
                        .filter(|u| m.team_of(*u).is_some())
                        .collect()
                }
                _ => return,
            },
            Err(_) => return,
        };
        for p in players.iter().filter(|p| wanted.contains(&p.uuid)) {
            let kinds = leashed_kinds(host, p);
            if !kinds.is_empty() {
                self.event(host, Event::Leashed { player: p.uuid, kinds });
            }
        }
    }
}

/// The 1.16 bartering table (`LockoutInitializer`'s `LootTableEvents.REPLACE`), in 26.3's
/// loot table format.
fn bartering_table() -> serde_json::Value {
    use serde_json::json;
    let enchanted = |item: &str, weight: i32| {
        json!({"type": "minecraft:item", "name": item, "weight": weight,
            "modifier": {"type": "minecraft:enchant_randomly", "options": "minecraft:soul_speed"}})
    };
    let potion = |item: &str| {
        json!({"type": "minecraft:item", "name": item, "weight": 10,
            "modifier": {"type": "minecraft:set_potion", "id": "minecraft:fire_resistance"}})
    };
    let counted = |item: &str, min: i32, max: i32, weight: i32| {
        json!({"type": "minecraft:item", "name": item, "weight": weight,
            "modifier": {"type": "minecraft:set_count", "count": {"type": "minecraft:uniform", "min": min, "max": max}}})
    };
    let entries = vec![
        enchanted("minecraft:book", 5),
        enchanted("minecraft:iron_boots", 8),
        potion("minecraft:potion"),
        potion("minecraft:splash_potion"),
        counted("minecraft:iron_nugget", 9, 36, 10),
        counted("minecraft:quartz", 8, 16, 20),
        counted("minecraft:glowstone_dust", 5, 12, 20),
        counted("minecraft:magma_cream", 2, 6, 20),
        counted("minecraft:ender_pearl", 4, 8, 20),
        counted("minecraft:string", 8, 24, 20),
        counted("minecraft:fire_charge", 1, 5, 40),
        counted("minecraft:gravel", 8, 16, 40),
        counted("minecraft:leather", 4, 10, 40),
        counted("minecraft:nether_brick", 4, 16, 40),
        json!({"type": "minecraft:item", "name": "minecraft:obsidian", "weight": 40}),
        counted("minecraft:crying_obsidian", 1, 3, 40),
        counted("minecraft:soul_sand", 4, 16, 40),
    ];
    json!({
        "type": "minecraft:barter",
        "pools": [{"rolls": 1, "entries": entries}],
        "random_sequence": "minecraft:gameplay/piglin_bartering",
    })
}

impl Hooks for Lockout {
    // ---- the server -------------------------------------------------------------------

    /// Piglins barter as in 1.16 (`LockoutInitializer`'s `LootTableEvents.REPLACE`).
    fn loot_table_load(&self, event: &mut LootTableLoadEvent<'_>) {
        if event.id == "minecraft:gameplay/piglin_bartering" {
            *event.table = bartering_table();
        }
    }

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

    fn hears_projectile_impacts(&self, _host: &dyn Host, event: &mut HearsProjectileImpactsEvent) {
        if self.live.load(Ordering::Relaxed) {
            event.hears = true;
        }
    }

    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        let Some(p) = host.acting_player() else {
            return Flow::Continue;
        };
        if is_tracker(event.stack) {
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
        // `CrossbowItemMixin`: a crossbow loaded with a rocket is about to shoot it.
        if &*event.stack.item == "minecraft:crossbow"
            && event
                .stack
                .extra
                .as_ref()
                .is_some_and(|e| e.charged.iter().any(|c| &*c.item == "minecraft:firework_rocket"))
        {
            self.did(host, p.uuid, Deed::ShootFireworkFromCrossbow);
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

    fn player_advancement_done(&self, host: &mut dyn Host, event: &mut PlayerAdvancementDoneEvent<'_>) {
        let id = event.advancement.to_owned();
        let title = names::advancement(&id).or_else(|| host.advancement_display(&id).map(|d| d.title.plain()));
        self.event(
            host,
            Event::Advancement {
                player: event.player,
                id,
                title,
            },
        );
    }

    fn player_advancement_criterion_grant(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerAdvancementCriterionGrantEvent<'_>,
    ) {
        self.event(
            host,
            Event::Criterion {
                player: event.player,
                advancement: event.advancement.to_owned(),
                criterion: event.criterion.to_owned(),
            },
        );
    }

    /// `PlayerAdvancementTrackerMixin.onBroadcastInChat`: a spectator's advancements aren't
    /// announced while a match runs.
    fn broadcast_message(&self, host: &dyn Host, event: &mut BroadcastMessageEvent<'_>) -> Flow {
        if !self.live.load(Ordering::Relaxed)
            || !event
                .line
                .translation_key()
                .is_some_and(|k| k.starts_with("chat.type.advancement."))
        {
            return Flow::Continue;
        }
        let component = event.line.component();
        let Some(who) = component.get("with").and_then(|w| w.get(0)) else {
            return Flow::Continue;
        };
        let name = who
            .get("insertion")
            .and_then(|n| n.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| plain_of(who));
        let Some(player) = online(host).into_iter().find(|p| p.name == name) else {
            return Flow::Continue;
        };
        let spectating = self
            .server
            .try_lock()
            .is_ok_and(|s| s.game.as_ref().is_some_and(|m| m.team_of(player.uuid).is_none()));
        if spectating { Flow::Cancel } else { Flow::Continue }
    }

    fn living_incoming_damage_v3(&self, event: &mut LivingIncomingDamageV3Event<'_>) -> Flow {
        let player = event.kind == "minecraft:player";
        if let Some(victim) = event.victim_uuid
            && player
            && self.frozen.lock().is_ok_and(|f| f.contains(&victim))
        {
            return Flow::Cancel;
        }
        // `PlayerMixin.onStartMatch`: no player is hurt until the match has started.
        if player && self.immune.load(Ordering::Relaxed) {
            return Flow::Cancel;
        }
        if let Ok(mut h) = self.hits.lock() {
            h.insert(
                event.victim,
                Hit {
                    victim: event.victim_uuid.filter(|_| player),
                    attacker: event.attacker_uuid,
                    attacker_entity: event.attacker,
                    direct: event.direct,
                    damage_type: event.damage_type.to_owned(),
                    amount: event.amount as f64,
                },
            );
        }
        Flow::Continue
    }

    /// The hit landed (`hurtServer` returned true): it counts, and its player is remembered
    /// for the kill (`lastHurtByPlayer`, 100 ticks).
    fn living_damage_post(&self, event: &mut LivingDamagePostEvent<'_>) {
        let victim = event.entity as u32;
        let Some(hit) = self.hits.lock().ok().and_then(|mut h| h.remove(&victim)) else {
            return;
        };
        // A shield that took the whole hit makes `hurtServer` answer false.
        if event.amount <= 0.0 && self.blocking.lock().is_ok_and(|b| b.contains(&event.entity)) {
            return;
        }
        let now = self.now.load(Ordering::Relaxed);
        if let Ok(mut credit) = self.credit.lock() {
            let entry = credit.entry(victim).or_default();
            match (hit.attacker, hit.attacker_entity) {
                (Some(p), _) => entry.player = Some((p, now)),
                (None, Some(mob)) => entry.mob = Some((mob, now)),
                _ => {}
            }
            if let Some(d) = hit.direct {
                entry.direct = Some((d, now));
            }
        }
        if (hit.victim.is_some() || hit.attacker.is_some())
            && let Ok(mut q) = self.queued.lock()
        {
            q.push(Event::Damage {
                victim: hit.victim,
                attacker: hit.attacker,
                arrow: ["minecraft:arrow", "minecraft:trident"].contains(&hit.damage_type.as_str()),
                damage_type: hit.damage_type,
                amount: hit.amount,
            });
        }
    }

    fn living_death(&self, host: &mut dyn Host, event: &mut LivingDeathEvent<'_>) -> Flow {
        let players = online(host);
        let now = self.now.load(Ordering::Relaxed);
        let credit = self.kill_credit(host, &players, event.entity, now);
        if event.kind == "minecraft:player" {
            if let Some(uuid) = uuid_of_entity(&players, event.entity as i32) {
                let killer_kind = (event.attacker != 0).then(|| kind_of(host, event.attacker)).flatten();
                let direct = self
                    .credit
                    .lock()
                    .ok()
                    .and_then(|c| c.get(&event.entity).and_then(|c| c.direct))
                    .filter(|(_, t)| now - t <= 1)
                    .map(|(d, _)| d);
                let direct_kind = direct.and_then(|d| kind_of(host, d));
                if let Ok(mut d) = self.deaths.lock() {
                    let entry = d.entry(uuid).or_default();
                    entry.damage_type = Some(event.damage_type.to_owned());
                    entry.killer_kind = killer_kind;
                    entry.killer_player = credit.filter(|k| *k != uuid);
                    entry.direct_kind = direct_kind;
                }
            }
            return Flow::Continue;
        }
        if let Some(player) = credit {
            let color = (event.kind == "minecraft:sheep")
                .then(|| sheep_color(host, event.entity))
                .flatten();
            let dimension = players
                .iter()
                .find(|p| p.uuid == player)
                .map(|p| p.dimension.clone())
                .unwrap_or_default();
            self.event(
                host,
                Event::Kill {
                    player,
                    kind: event.kind.to_owned(),
                    color,
                    dimension,
                    damage_type: event.damage_type.to_owned(),
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

    fn living_entity_use_item_start(&self, host: &mut dyn Host, event: &mut LivingEntityUseItemStartEvent<'_>) -> Flow {
        if host.window() || event.entity.is_some() {
            return Flow::Continue;
        }
        let Some(p) = host.acting_player() else {
            return Flow::Continue;
        };
        match &*event.stack.item {
            // `ConsumeItemCriterionMixin` asks whether there were effects to drink away.
            "minecraft:milk_bucket" => {
                let had = self
                    .effects
                    .lock()
                    .is_ok_and(|e| e.get(&p.uuid).is_some_and(|e| !e.is_empty()));
                if let Ok(mut m) = self.milk.lock() {
                    m.insert(p.uuid, had);
                }
            }
            // `GoatHornItemMixin`: a horn is only used off its cooldown.
            "minecraft:goat_horn" => self.did(host, p.uuid, Deed::TootGoatHorn),
            _ => {}
        }
        Flow::Continue
    }

    /// `UsingItemCriterionMixin`: every tick a spyglass is up, the mob it looks at.
    fn living_entity_use_item_tick(&self, host: &mut dyn Host, event: &mut LivingEntityUseItemTickEvent<'_>) -> Flow {
        if host.window() || event.entity.is_some() || &*event.stack.item != "minecraft:spyglass" {
            return Flow::Continue;
        }
        let wanted = self.server.try_lock().is_ok_and(|s| {
            s.game.as_ref().is_some_and(|m| {
                m.board.iter().any(|sq| {
                    sq.completed_by.is_none() && matches!(sq.trigger, Trigger::Spy { .. } | Trigger::SpyUnique { .. })
                })
            })
        });
        if !wanted {
            return Flow::Continue;
        }
        let Some(p) = host.acting_player() else {
            return Flow::Continue;
        };
        if let Some(kind) = spied_mob(host, &p) {
            self.event(host, Event::Spied { player: p.uuid, kind });
        }
        Flow::Continue
    }

    fn living_entity_use_item_finish(&self, host: &mut dyn Host, event: &mut LivingEntityUseItemFinishEvent<'_>) {
        if host.window() || event.entity.is_some() {
            return;
        }
        let Some(p) = host.acting_player() else { return };
        let server::Held { item, potion, .. } = stack_view(event.stack);
        let food = host.component(event.stack, "minecraft:food").ok().flatten().is_some();
        if item == "minecraft:milk_bucket"
            && self
                .milk
                .lock()
                .ok()
                .and_then(|mut m| m.remove(&p.uuid))
                .unwrap_or(false)
        {
            self.event(host, Event::MilkAway { player: p.uuid });
        }
        self.event(
            host,
            Event::Consumed {
                player: p.uuid,
                item,
                potion,
                food,
            },
        );
    }

    fn player_stat_award(&self, host: &mut dyn Host, event: &mut PlayerStatAwardEvent<'_>) -> Flow {
        if event.value > 0 {
            self.event(
                host,
                Event::Stat {
                    player: event.player,
                    stat_type: event.stat_type.to_owned(),
                    stat: event.stat.to_owned(),
                    value: event.value,
                },
            );
        }
        Flow::Continue
    }

    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        let Some(player) = host.acting_player() else {
            return Flow::Continue;
        };
        let block = host.block(event.pos);
        // `CreakingHeartBlockEntityMixin`: breaking a heart while its Creaking is out kills it.
        let creaking = block == "minecraft:creaking_heart"
            && host
                .block_entity_tag(&player.dimension, event.pos)
                .is_some_and(|t| t.get("creaking").is_some());
        self.event(
            host,
            Event::Mined {
                player: player.uuid,
                block,
            },
        );
        if creaking {
            self.did(host, player.uuid, Deed::KillCreaking);
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
        // `ThrownEnderpearlMixin`: a pearl that went through a portal pulled its owner along.
        let now = self.now.load(Ordering::Relaxed);
        let pearl = self
            .pearls
            .lock()
            .ok()
            .and_then(|mut p| p.remove(&event.player))
            .is_some_and(|t| now - t <= 2);
        if pearl && portal_crossing(event.from, event.to) {
            self.did(host, event.player, Deed::PearlThroughPortal);
        }
    }

    fn player_teleport(&self, event: &mut PlayerTeleportEvent<'_>) -> Flow {
        if event.cause == "ender_pearl" {
            if portal_crossing(event.from_dimension, event.to_dimension) {
                if let Ok(mut q) = self.queued.lock() {
                    q.push(Event::Did {
                        player: event.player,
                        deed: Deed::PearlThroughPortal,
                    });
                }
            } else if let Ok(mut p) = self.pearls.lock() {
                p.insert(event.player, self.now.load(Ordering::Relaxed));
            }
        }
        Flow::Continue
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
        if event.stack.count == 0 {
            return;
        }
        self.event(
            host,
            Event::Crafted {
                player: event.player,
                item: event.stack.item.to_string(),
            },
        );
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
        self.did(host, event.player, Deed::ShieldDisabled);
    }

    /// `ItemFrameEntityMixin`: an item frame put in a plain (not glow) item frame.
    fn player_item_frame_change(&self, host: &mut dyn Host, event: &mut PlayerItemFrameChangeEvent<'_>) {
        if event.action == "put"
            && event.item == "minecraft:item_frame"
            && kind_of(host, event.entity_id).as_deref() == Some("minecraft:item_frame")
        {
            self.did(host, event.player, Deed::ItemFrameInItemFrame);
        }
    }

    fn interact_entity_v2(&self, host: &mut dyn Host, event: &mut InteractEntityV2Event<'_>) -> Flow {
        let Some(p) = host.acting_player() else {
            return Flow::Continue;
        };
        let holding = event.stack.map(|s| &*s.item);
        let id = event.entity as u32;
        let check = match (event.kind, holding) {
            // `WolfEntityMixin`: the armour went on (next tick shows it).
            ("minecraft:wolf", Some("minecraft:wolf_armor")) => Some(Check::Wolf {
                player: p.uuid,
                wolf: id,
            }),
            // `AllayMixin`: a dancing Allay given a shard duplicates, and starts its cooldown.
            ("minecraft:allay", Some("minecraft:amethyst_shard")) => Some(Check::Allay {
                player: p.uuid,
                allay: id,
                cooldown: allay_cooldown(host, &p.dimension, id),
            }),
            _ => None,
        };
        if let (Some(check), Ok(mut c)) = (check, self.checks.lock()) {
            if let Check::Wolf { .. } = check
                && wolf_armored(host, id)
            {
                return Flow::Continue;
            }
            c.push(check);
        }
        Flow::Continue
    }

    /// `ArmorStandMixin`: the stand is checked once the click is done.
    fn armor_stand_manipulate(&self, host: &mut dyn Host, event: &mut ArmorStandManipulateEvent<'_>) -> Flow {
        let spectator = host.acting_player().is_some_and(|p| p.game_mode == 3);
        if !spectator && let Ok(mut c) = self.checks.lock() {
            c.push(Check::ArmorStand {
                player: event.player,
                stand: event.stand_id as u32,
            });
        }
        Flow::Continue
    }

    fn player_toggle_entity_age_lock(&self, host: &mut dyn Host, event: &mut PlayerToggleEntityAgeLockEvent<'_>) {
        self.did(host, event.player, Deed::GoldenDandelion);
    }

    fn anvil_craft_post(&self, host: &mut dyn Host, event: &mut AnvilCraftPostEvent<'_>) {
        self.did(host, event.player, Deed::UseAnvil);
    }

    fn player_container_open(&self, _host: &mut dyn Host, event: &mut PlayerContainerOpenEvent<'_>) {
        if let Ok(mut m) = self.menus.lock() {
            m.insert(
                event.player,
                OpenMenu {
                    kind: event.menu.trim_start_matches("minecraft:").to_owned(),
                    extra: None,
                },
            );
        }
    }

    fn player_container_close(&self, _host: &mut dyn Host, event: &mut PlayerContainerCloseEvent<'_>) {
        if let Ok(mut m) = self.menus.lock() {
            m.remove(&event.player);
        }
    }

    /// A click on a station's result (`SlotMixin`, `GrindstoneScreenHandlerOutputSlotMixin`,
    /// `SmithingScreenHandlerMixin`, `BrewingStandScreenHandlerPotionSlotMixin`): whether it
    /// took something shows on the next tick, in the player's inventory and cursor.
    fn inventory_click(&self, host: &mut dyn Host, event: &mut InventoryClickEvent<'_>) -> Flow {
        let Ok(mut menus) = self.menus.lock() else {
            return Flow::Continue;
        };
        let Some(menu) = menus.get_mut(&event.player) else {
            return Flow::Continue;
        };
        let cursor = host.menu_cursor(event.player);
        let inventory = host.player_stacks(event.player).unwrap_or_default();
        if menu.kind == "cartography_table" {
            // What sits in the extra slot: a glass pane there locks the map.
            let from_inventory = |slot: i32| -> Option<String> {
                let i = match slot {
                    3..=29 => slot as usize + 6,
                    30..=38 => slot as usize - 30,
                    _ => return None,
                };
                inventory.get(i).cloned().flatten().map(|s| s.item.to_string())
            };
            match (event.slot, event.mode) {
                (1, 0) => menu.extra = cursor.as_ref().map(|c| c.item.to_string()),
                (1, 1) => menu.extra = None,
                (s, 1) if from_inventory(s).as_deref() == Some("minecraft:glass_pane") => {
                    menu.extra = Some("minecraft:glass_pane".into());
                }
                _ => {}
            }
        }
        let taken = match (menu.kind.as_str(), event.slot) {
            ("brewing_stand", 0..=2) => Some(Taken::Potion),
            ("stonecutter", 1) => Some(Taken::Deed(Deed::UseStonecutter)),
            ("loom", 3) => Some(Taken::Deed(Deed::UseLoom)),
            ("cartography_table", 2) => Some(Taken::Map {
                locked: menu.extra.as_deref() == Some("minecraft:glass_pane"),
            }),
            ("grindstone", 2) => Some(Taken::Deed(Deed::UseGrindstone)),
            ("smithing", 3) => Some(Taken::Deed(Deed::UseSmithingTable)),
            _ => None,
        };
        drop(menus);
        // Pick up, shift-click, hotbar swap, throw.
        let Some(taken) = taken.filter(|_| matches!(event.mode, 0 | 1 | 2 | 4)) else {
            return Flow::Continue;
        };
        let mut before = carried(&inventory, cursor.as_ref());
        if let Some(off) = host.player_equipment(event.player, EquipmentSlot::OffHand) {
            *before.entry(potion_key(&off)).or_default() += off.count;
        }
        if let Ok(mut t) = self.takes.lock() {
            t.push(Take {
                player: event.player,
                taken,
                thrown: event.mode == 4,
                before,
            });
        }
        Flow::Continue
    }

    fn right_click_block_v2(&self, host: &mut dyn Host, event: &mut RightClickBlockV2Event) -> Flow {
        let Some(p) = host.acting_player() else {
            return Flow::Continue;
        };
        let Some(level) = host.entity(p.entity_id as u32).and_then(|e| host.entity_level(e)) else {
            return Flow::Continue;
        };
        let pos = event.pos;
        let Some(block) = host.level_block(level, pos) else {
            return Flow::Continue;
        };
        let held = host
            .entity(p.entity_id as u32)
            .and_then(|e| host.living(e))
            .and_then(|l| host.living_hand_item(l, event.hand));
        let item = held.as_ref().map(|s| s.item.to_string()).unwrap_or_default();
        let prop = |host: &dyn Host, key: &str| host.level_block_property(level, pos, key);
        let path = block.trim_start_matches("minecraft:");
        // `UseBlockEventHandler`: flint and steel or a fire charge on a candle that can be lit.
        if path.ends_with("candle")
            && prop(host, "lit").as_deref() == Some("false")
            && prop(host, "waterlogged").as_deref() == Some("false")
            && (item == "minecraft:flint_and_steel" || item == "minecraft:fire_charge")
        {
            self.did(host, p.uuid, Deed::LightCandle);
        }
        let kind = match path {
            "composter" => {
                let level_now: i32 = prop(host, "level").and_then(|l| l.parse().ok()).unwrap_or(0);
                if level_now == 8 {
                    // `ComposterBlockMixin.emptyFullComposterMixin`.
                    self.did(host, p.uuid, Deed::UseComposter);
                    None
                } else if level_now < 7 && held.is_some() {
                    let food = held
                        .as_ref()
                        .is_some_and(|s| host.component(s, "minecraft:food").ok().flatten().is_some());
                    Some(BlockCheck::Composter {
                        item: item.clone(),
                        food,
                        level: level_now,
                    })
                } else {
                    None
                }
            }
            "campfire" | "soul_campfire" => Some(BlockCheck::Campfire),
            "chiseled_bookshelf" => Some(BlockCheck::Bookshelf),
            "decorated_pot" => Some(BlockCheck::Pot),
            p if (p.starts_with("crimson_") || p.starts_with("warped_"))
                && p.ends_with("_sign")
                && item == "minecraft:glow_ink_sac" =>
            {
                Some(BlockCheck::Sign {
                    crimson: p.starts_with("crimson_"),
                })
            }
            "chest" | "barrel" | "trapped_chest" => Some(BlockCheck::Loot),
            _ => None,
        };
        if let Some(kind) = kind {
            let before = match &kind {
                BlockCheck::Composter { level, .. } => BlockState::Level(*level),
                _ => block_state(host, &p.dimension, pos, &kind),
            };
            if !matches!((&kind, &before), (BlockCheck::Loot, BlockState::None))
                && let Ok(mut c) = self.checks.lock()
            {
                c.push(Check::Block {
                    player: p.uuid,
                    dimension: p.dimension.clone(),
                    pos,
                    kind,
                    before,
                });
            }
        }
        Flow::Continue
    }

    fn projectile_impact(&self, host: &mut dyn Host, event: &mut ProjectileImpactEvent<'_>) -> Flow {
        let players = online(host);
        let shooter = players
            .iter()
            .find(|p| p.entity_id == event.owner as i32 && event.owner != 0);
        match (event.entity, event.entity_kind, shooter) {
            // `PlayerMixin.onCollide`: a player's snowball or egg on another player.
            (Some(e), "minecraft:player", Some(shooter))
                if event.kind == "minecraft:snowball" || event.kind == "minecraft:egg" =>
            {
                if let Some(victim) = players.iter().find(|p| p.entity_id == e as i32) {
                    self.event(
                        host,
                        Event::HitBy {
                            shooter: shooter.uuid,
                            shooter_name: shooter.name.clone(),
                            victim: victim.uuid,
                            victim_name: victim.name.clone(),
                            projectile: event.kind.to_owned(),
                        },
                    );
                }
            }
            (Some(e), "minecraft:end_crystal", Some(shooter)) => self.hit_crystal(e, shooter.uuid),
            _ => {}
        }
        Flow::Continue
    }

    fn attack_entity(&self, host: &mut dyn Host, event: &mut AttackEntityEvent<'_>) -> Flow {
        if event.kind == "minecraft:end_crystal"
            && let Some(p) = host.acting_player()
        {
            self.hit_crystal(event.entity as u32, p.uuid);
        }
        Flow::Continue
    }

    /// `EndCrystalMixin`: a crystal a player hit blew up.
    fn explosion_detonate(&self, host: &mut dyn Host, event: &mut ExplosionDetonateEvent<'_>) {
        let Some((id, "minecraft:end_crystal")) = event.source else {
            return;
        };
        let now = self.now.load(Ordering::Relaxed);
        let player = self
            .crystals
            .lock()
            .ok()
            .and_then(|mut c| c.remove(&id))
            .filter(|(_, t)| now - t <= 1)
            .map(|(p, _)| p);
        if let Some(player) = player {
            self.did(host, player, Deed::ExplodeEndCrystal);
        }
    }

    fn item_picked_up_v2(&self, host: &mut dyn Host, event: &mut ItemPickedUpV2Event<'_>) -> Flow {
        // `DroppedItemOwners`: who dropped it, death drops included.
        let owner = event.thrower.or_else(|| {
            let dropped = self.dropped.lock().ok()?;
            dropped
                .iter()
                .find(|(id, _)| {
                    host.entity(**id)
                        .and_then(|e| host.entity_position(e))
                        .is_some_and(|at| (0..3).all(|i| (at[i] - event.position[i]).abs() < 0.01))
                })
                .map(|(_, owner)| *owner)
        });
        if let Some(owner) = owner.filter(|o| *o != event.player) {
            self.event(
                host,
                Event::PickedUp {
                    player: event.player,
                    owner,
                },
            );
        }
        Flow::Continue
    }

    fn loot_drops(&self, host: &mut dyn Host, event: &mut LootDropsEvent<'_>) {
        let c = event.context;
        // A dying player's inventory: its items are told as they join the level.
        if let (Some((_, "minecraft:player")), Some(owner), false) = (c.entity, c.player, c.used) {
            if let Ok(mut d) = self.death_drops.lock() {
                d.push((
                    c.dimension.to_owned(),
                    c.origin,
                    owner,
                    self.now.load(Ordering::Relaxed),
                ));
            }
            return;
        }
        // `BrushableBlockEntityMixin`: a suspicious block brushed clean.
        if c.used
            && c.block.is_some()
            && c.tool.is_some_and(|t| &*t.item == "minecraft:brush")
            && let Some(player) = c.player
        {
            self.did(host, player, Deed::Brush);
        }
    }

    fn entity_join_level(&self, _host: &mut dyn Host, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
        if event.kind != "minecraft:item" || event.loaded_from_disk {
            return Flow::Continue;
        }
        let now = self.now.load(Ordering::Relaxed);
        let owner = self.death_drops.lock().ok().and_then(|d| {
            d.iter()
                .find(|(dim, at, _, t)| {
                    now - t <= 1 && dim == event.dimension && (0..3).all(|i| (at[i] - event.pos[i]).abs() < 1.5)
                })
                .map(|(_, _, owner, _)| *owner)
        });
        if let (Some(owner), Ok(mut d)) = (owner, self.dropped.lock()) {
            d.insert(event.entity, owner);
        }
        Flow::Continue
    }

    /// `SummonedEntityCriterionMixin`: every player near a Copper Golem as it is built.
    fn mob_spawned(&self, host: &mut dyn Host, event: &mut MobSpawnedEvent<'_>) {
        if event.kind != "minecraft:copper_golem" || event.natural {
            return;
        }
        for p in online(host) {
            let d = [
                p.position[0] - event.pos[0],
                p.position[1] - event.pos[1],
                p.position[2] - event.pos[2],
            ];
            if p.dimension == event.dimension
                && d[0].abs() <= 5.25
                && d[2].abs() <= 5.25
                && (-6.8..=6.0).contains(&d[1])
            {
                self.did(host, p.uuid, Deed::ConstructCopperGolem);
            }
        }
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
        // `InputMixin`: before the start a playing client moves not at all; only sneak is kept.
        let hold = match self.client.try_lock() {
            Ok(c) => {
                let hold = client::hold_still(&c);
                self.hold.store(hold, Ordering::Relaxed);
                hold
            }
            Err(_) => self.hold.load(Ordering::Relaxed),
        };
        if hold {
            event.forward = 0.0;
            event.strafe = 0.0;
            event.jump = false;
        }
    }

    fn toast_add(&self, _host: &mut dyn Host, event: &mut ToastAddEvent<'_>) -> Flow {
        // The toast mixins: advancement, recipe, system and tutorial toasts are hidden while
        // there is a board. Horizon can only refuse a toast as it is queued, not hide one
        // already shown, so on a Lockout server the tutorial's hints (queued as the world opens,
        // before the board) are refused from the start.
        let hidden = ["advancement", "recipe", "system", "tutorial"].contains(&event.kind);
        let refuse = self
            .client
            .try_lock()
            .is_ok_and(|c| hidden && (c.board.is_some() || (event.kind == "tutorial" && c.lockout_server)));
        if refuse { Flow::Cancel } else { Flow::Continue }
    }

    fn gui_layer_pre(&self, host: &mut dyn Host, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        // The layers after the sleep overlay: the board goes below the first that is drawn, in
        // case the sleep overlay's own pass isn't told.
        if AFTER_SLEEP.contains(&event.layer) {
            self.draw_board_hud(host, event.batch);
        }
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
        // `LockoutClient`: the board is a HUD element after the sleep overlay
        // (`attachElementAfter(VanillaHudElements.SLEEP)`), so screens, chat and titles cover it.
        if event.layer == "minecraft:sleep_overlay" {
            self.draw_board_hud(host, event.batch);
        }
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

    /// The end of the frame: the board is drawn again next frame.
    fn gui_render_post(&self, _host: &dyn Host, _event: &mut GuiRenderPostEvent<'_>) {
        self.hud_drawn.store(false, Ordering::Relaxed);
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
    fn the_cancel_commands_answer_when_nothing_runs() {
        let mods = mods();
        let mut host = Recorder::default();
        for line in [
            "lockout draft cancel",
            "blackout draft cancel",
            "lockout cancel",
            "blackout cancel",
        ] {
            let mut ran = false;
            dispatch_perform_console_command(&mut host, &mods, line, &mut ran);
            assert!(ran, "{line}");
        }
        assert_eq!(
            host.feedback,
            vec![
                "No draft is running.",
                "No draft is running.",
                "No match or draft is running.",
                "No match or draft is running."
            ]
        );
    }

    #[test]
    fn piglins_barter_as_in_1_16() {
        let mods = mods();
        let mut table = serde_json::json!({"pools": []});
        dispatch_loot_table_load(&mods, "minecraft:gameplay/piglin_bartering", &mut table);
        let entries = table["pools"][0]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 17);
        let weights: i64 = entries.iter().map(|e| e["weight"].as_i64().unwrap()).sum();
        assert_eq!(weights, 423);
        assert_eq!(table["random_sequence"], "minecraft:gameplay/piglin_bartering");
        let mut other = serde_json::json!({"pools": []});
        dispatch_loot_table_load(&mods, "minecraft:entities/zombie", &mut other);
        assert_eq!(other, serde_json::json!({"pools": []}));
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
