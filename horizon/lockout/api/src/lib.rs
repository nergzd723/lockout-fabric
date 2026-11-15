//! Lockout for Horizon. The launcher links this crate into the game and calls [`hooks`].
//!
//! One mod object serves both of Horizon's games: the server's (single player's server
//! thread, a LAN host, a dedicated server), which runs the match, and the window's, which
//! draws the board. Both speak the Fabric mod's payloads (`wire`), so either side also
//! plays with the Fabric mod on the other.
pub mod board;
pub mod client;
pub mod goals;
pub mod server;
pub mod wire;

use horizon_client::mod_api::*;
use server::{Action, Event, StartRequest, State};
use std::collections::HashMap;
use std::sync::Mutex;
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
}

#[derive(Default, Clone)]
struct PendingDeath {
    damage_type: Option<String>,
    killer_kind: Option<String>,
    killer_player: Option<u128>,
    message_key: Option<String>,
}

const NAMESPACE_COMMANDS: &[(&str, u8)] = &[
    ("lockout teams <teams:text>", 2),
    ("lockout players <players:text>", 2),
    ("lockout located <enabled:boolean>", 2),
    ("lockout stop", 2),
    ("blackout team <team:word>", 2),
    ("blackout players <players:text>", 2),
    ("SetBoardSize <size:integer>", 2),
    ("SetStartTime <seconds:integer>", 2),
    ("RemoveCustomBoard", 2),
];

fn online(host: &dyn Host) -> Vec<NativeServerPlayer> {
    host.server_players()
}

fn uuid_of_entity(players: &[NativeServerPlayer], entity: i32) -> Option<u128> {
    (entity != 0)
        .then(|| players.iter().find(|p| p.entity_id == entity).map(|p| p.uuid))
        .flatten()
}

fn kind_of(host: &dyn Host, entity: u32) -> Option<String> {
    host.entity(entity).and_then(|e| host.entity_kind(e))
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
        let mut everyone: Option<Vec<u128>> = None;
        let mut later = Vec::new();
        for action in actions {
            match action {
                Action::Send { player, payload } => send(host, player, &payload),
                Action::SendAll { payload } => {
                    let all = everyone.get_or_insert_with(|| online(host).iter().map(|p| p.uuid).collect());
                    for p in all.clone() {
                        send(host, p, &payload);
                    }
                }
                other if !in_tick => later.push(other),
                Action::Tell { player, text, color } => {
                    host.tell(player, Text::colored(text, color));
                }
                Action::Broadcast { text, color } => host.broadcast(Text::colored(text, color)),
                Action::Command(line) => {
                    host.run_command(&line);
                }
                Action::GameMode { player, mode } => {
                    host.set_game_mode(player, mode);
                }
                Action::Sound { player, sound, pitch } => {
                    host.play_sound_to(player, sound, "master", None, 2.0, pitch);
                }
            }
        }
        if !later.is_empty()
            && let Ok(mut s) = self.server.lock()
        {
            s.deferred.extend(later);
        }
    }

    fn command(&self, host: &mut dyn Host, requester: Option<u128>, line: &str) -> bool {
        let words: Vec<&str> = line.trim_start_matches('/').split_whitespace().collect();
        let Some(first) = words.first() else { return false };
        let rest = |n: usize| words.iter().skip(n).map(|w| w.to_string()).collect::<Vec<_>>();
        let reply = |host: &mut dyn Host, text: String| host.feedback(Text::literal(text));
        let mut s = match self.server.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };
        match (first.to_ascii_lowercase().as_str(), words.get(1).copied()) {
            ("lockout", Some("teams")) => {
                s.pending_start = Some(StartRequest::Teams {
                    requester,
                    names: rest(2),
                    blackout: false,
                });
            }
            ("lockout", Some("players")) => {
                s.pending_start = Some(StartRequest::Players {
                    requester,
                    names: rest(2),
                    blackout: false,
                });
            }
            ("blackout", Some("team")) => {
                s.pending_start = Some(StartRequest::Teams {
                    requester,
                    names: rest(2),
                    blackout: true,
                });
            }
            ("blackout", Some("players")) => {
                s.pending_start = Some(StartRequest::Players {
                    requester,
                    names: rest(2),
                    blackout: true,
                });
            }
            ("lockout", Some("located")) => {
                let on = words.get(2).is_some_and(|w| *w == "true");
                s.settings.located_goals = on;
                drop(s);
                reply(
                    host,
                    format!(
                        "Goals that need a nearby biome or structure are {} random boards.",
                        if on { "on" } else { "left off" }
                    ),
                );
            }
            ("lockout", Some("stop")) => {
                let had = s.game.take().is_some();
                drop(s);
                if had {
                    reply(host, "Stopped the match.".into());
                    let all: Vec<u128> = online(host).iter().map(|p| p.uuid).collect();
                    for p in all {
                        send(
                            host,
                            p,
                            &Payload::End {
                                winners: Vec::new(),
                                time: 0,
                            },
                        );
                    }
                } else {
                    reply(host, "No match is running.".into());
                }
            }
            ("setboardsize", Some(n)) => match n.parse::<usize>() {
                Ok(n) if (server::MIN_BOARD_SIZE..=server::MAX_BOARD_SIZE).contains(&n) => {
                    s.settings.board_size = n;
                    drop(s);
                    reply(host, format!("Updated board size to {n}."));
                }
                _ => {
                    drop(s);
                    reply(host, "Board size must be between 3 and 7.".into());
                }
            },
            ("setstarttime", Some(n)) => match n.parse::<i64>() {
                Ok(n) if (5..=300).contains(&n) => {
                    s.settings.start_seconds = n;
                    drop(s);
                    reply(host, format!("Updated start time to {n}s."));
                }
                _ => {
                    drop(s);
                    reply(host, "Start time must be between 5 and 300 seconds.".into());
                }
            },
            ("removecustomboard", _) => {
                s.custom_board = None;
                drop(s);
                reply(host, "Removed custom board.".into());
            }
            _ => return false,
        }
        true
    }

    /// The server's tick: commands left for it, a start waiting for its teams, the clock.
    fn server_tick(&self, host: &mut dyn Host) {
        let (deferred, pending) = match self.server.lock() {
            Ok(mut s) => (std::mem::take(&mut s.deferred), s.pending_start.take()),
            Err(_) => return,
        };
        self.apply(host, deferred, true);
        if let Some(request) = pending {
            self.begin(host, request);
        }
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
        let uuids: Vec<u128> = players.iter().map(|p| p.uuid).collect();
        let xp: HashMap<u128, i32> = players.iter().map(|p| (p.uuid, p.xp_level)).collect();
        let actions = match self.server.lock() {
            Ok(mut s) => {
                let now = s.server_ticks;
                s.last_hurt_by.retain(|_, (_, t)| now - *t < 200);
                s.tick(&uuids, &xp)
            }
            Err(_) => return,
        };
        self.apply(host, actions, true);
    }

    fn begin(&self, host: &mut dyn Host, request: StartRequest) {
        let players = online(host);
        let roster: Vec<(u128, String)> = players.iter().map(|p| (p.uuid, p.name.clone())).collect();
        let (requester, teams) = match &request {
            StartRequest::Players {
                requester,
                names,
                blackout,
            } => (*requester, server::teams_of_players(names, &roster, *blackout)),
            StartRequest::Teams {
                requester,
                names,
                blackout,
            } => {
                let listed = if names.is_empty() {
                    Err("No teams listed.".to_owned())
                } else if *blackout && names.len() != 1 {
                    Err("Blackout takes one team.".to_owned())
                } else if !*blackout && names.len() < 2 {
                    Err("Not enough teams listed. Make sure you separate team names with spaces.".to_owned())
                } else {
                    names.iter().map(|n| list_team(host, n)).collect::<Result<Vec<_>, _>>()
                };
                (*requester, listed.and_then(|l| server::teams_from(l, &roster)))
            }
        };
        let fail = |host: &mut dyn Host, text: String| match requester {
            Some(p) => {
                host.tell(p, Text::colored(text, 0xFF5555));
            }
            None => host.log("WARN", &text),
        };
        let teams = match teams {
            Ok(t) => t,
            Err(e) => return fail(host, e),
        };
        let sprint: HashMap<u128, i32> = roster
            .iter()
            .map(|(u, _)| {
                (
                    *u,
                    host.player_stat(*u, "minecraft:custom", "minecraft:sprint_one_cm")
                        .unwrap_or(0),
                )
            })
            .collect();
        let actions = {
            let Ok(mut s) = self.server.lock() else { return };
            let goals = match &s.custom_board {
                Some(b) => Some(b.clone()),
                None => {
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_nanos() as u64);
                    let mut rng = board::Rng::new(seed);
                    board::generate(
                        goals::table(),
                        s.settings.board_size,
                        teams.len(),
                        s.settings.located_goals,
                        &mut rng,
                    )
                }
            };
            let Some(goals) = goals else {
                drop(s);
                return fail(host, "Not enough goals for a board this size.".into());
            };
            s.start(teams, goals, &roster, &sprint)
        };
        self.apply(host, actions, true);
    }
}

/// A vanilla team's members and colour, read with `/team list <name>`.
fn list_team(host: &mut dyn Host, name: &str) -> Result<(String, Option<String>, Vec<String>), String> {
    let result = host.run_command_captured(&format!("team list {name}"));
    let Some(line) = result.output.first() else {
        return Err(format!("Team {name} is invalid."));
    };
    if !result.ran {
        return Err(format!("Team {name} is invalid."));
    }
    let component = line.component();
    let key = component.get("translate").and_then(|v| v.as_str()).unwrap_or("");
    if key.ends_with("members.empty") {
        return Err(format!("Team {name} has no players."));
    }
    let args = component
        .get("with")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let color = args.first().and_then(find_color);
    let members_text = args.get(2).map(plain_of).unwrap_or_else(|| {
        // Fall back on the line's text: "Team [Red] has 2 members: Alex, Steve".
        line.plain()
            .rsplit_once(": ")
            .map(|(_, m)| m.to_owned())
            .unwrap_or_default()
    });
    let members: Vec<String> = members_text
        .split(',')
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .collect();
    Ok((name.to_owned(), color, members))
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

impl Hooks for Lockout {
    // ---- the server -------------------------------------------------------------------

    fn register_commands(&self, _host: &mut dyn Host, event: &mut RegisterCommandsEvent<'_>) {
        for (usage, level) in NAMESPACE_COMMANDS {
            event.commands.push(NativeCommand::op(usage, *level));
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
            client::tick(&self.client, host);
        } else {
            self.server_tick(host);
        }
    }

    fn server_stopping(&self, _host: &mut dyn Host, _event: &mut ServerStoppingEvent) {
        if let Ok(mut s) = self.server.lock() {
            let settings = s.settings.clone();
            *s = State {
                settings,
                ..State::default()
            };
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
        let payloads = self
            .server
            .lock()
            .ok()
            .and_then(|s| s.game.as_ref().map(|m| m.join_payloads(event.player)));
        for p in payloads.unwrap_or_default() {
            send(host, event.player, &p);
        }
    }

    fn custom_payload_from_client(&self, host: &mut dyn Host, event: &mut CustomPayloadFromClientEvent<'_>) {
        match Payload::decode(event.channel, event.data) {
            Some(Ok(Payload::Version { version })) if version != wire::LOCKOUT_VERSION => {
                host.tell(
                    event.player,
                    Text::colored(
                        format!(
                            "Wrong Lockout version: v{version}. Server is using Lockout v{}.",
                            wire::LOCKOUT_VERSION
                        ),
                        0xFF5555,
                    ),
                );
            }
            Some(Ok(Payload::CustomBoard { goals })) => {
                let allowed = host.player_permission(event.player).unwrap_or(0) >= 2;
                if !allowed {
                    host.tell(
                        event.player,
                        Text::colored("You do not have the permission for this command!", 0xFF5555),
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
            self.event(
                host,
                Event::Kill {
                    player,
                    kind: event.kind.to_owned(),
                    color,
                    dimension: players.iter().find(|p| p.uuid == player).map(|p| p.dimension.clone()),
                },
            );
        }
        Flow::Continue
    }

    fn player_died(&self, _host: &mut dyn Host, event: &mut PlayerDiedEvent<'_>) {
        if let Ok(mut d) = self.deaths.lock() {
            d.entry(event.player).or_default().message_key = event.cause.map(str::to_owned);
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
        if let ClientboundPlayPacket::CustomPayload(p) = event.packet {
            // The serverbound packet carries the same struct; building it from the one heard
            // keeps this crate off horizon-protocol, which mods can't depend on.
            let template = p.clone();
            let reply = |channel: &str, data: Vec<u8>| {
                let mut out = template.clone();
                out.channel = channel.into();
                out.data = data;
                ServerboundPlayPacket::CustomPayload(out)
            };
            client::packet(&self.client, host, &p.channel, &p.data, reply);
        }
    }

    fn gui_render_post(&self, host: &dyn Host, event: &mut GuiRenderPostEvent<'_>) {
        if event.gui_hidden {
            return;
        }
        let Ok(c) = self.client.lock() else { return };
        if c.board.is_some() {
            client::draw(&c, host, event.batch);
            return;
        }
        // Single player and a LAN host run the match in this process: draw it from there if
        // no payload has reached the window.
        let local = self
            .server
            .try_lock()
            .ok()
            .and_then(|s| s.game.as_ref().map(|m| (m.goals_teams_payload(), m.ticks, m.started)));
        if let Some((payload, ticks, started)) = local {
            let mut snapshot = client::Client::default();
            snapshot.apply(payload);
            snapshot.apply(Payload::UpdateTimer { ticks });
            if started {
                snapshot.apply(Payload::Start);
            }
            snapshot.tooltips = c.tooltips.clone();
            client::draw(&snapshot, host, event.batch);
        }
    }

    fn client_disconnecting(&self, _host: &dyn Host, _event: &mut ClientDisconnectingEvent) {
        if let Ok(mut c) = self.client.lock() {
            *c = client::Client::default();
        }
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
        assert_eq!(host.feedback, vec!["Board size must be between 3 and 7."]);
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
