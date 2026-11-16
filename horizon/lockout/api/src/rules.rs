//! What completes each goal, as the Java mod decides it: one arm per event, each following
//! the handler or mixin it ports (`AfterDeathEventHandler`, `EndServerTickEventHandler`,
//! `PlayerMixin`, ...), with the same team progress the Java mod keeps on `Lockout` (and saves
//! under the same names), the same chat lines and the same tooltips.
use crate::goals::{Counter, Deed, HOSTILE, LEATHER_RGB, OpponentEvent, Trigger};
use crate::names;
use crate::server::{Action, Held, Match, Span, State, legacy_color};
use crate::wire::Payload;
use std::collections::{HashMap, HashSet};

/// `LinkedHashSet.add`: false when it was there.
pub fn insert(set: &mut Vec<String>, value: &str) -> bool {
    if set.iter().any(|v| v == value) {
        false
    } else {
        set.push(value.to_owned());
        true
    }
}

/// The progress fields of the Java mod's `Lockout`, keyed by team index (or player UUID).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trackers {
    pub bred_animal_types: HashMap<usize, Vec<String>>,
    pub killed_hostile_types: HashMap<usize, Vec<String>>,
    pub killed_raid_mobs: HashMap<usize, Vec<String>>,
    pub killed_undead_mobs: HashMap<usize, i32>,
    pub killed_arthropods: HashMap<usize, i32>,
    pub food_types_eaten: HashMap<usize, Vec<String>>,
    pub unique_items_crafted_by_team: HashMap<usize, Vec<String>>,
    pub composted_foods: HashMap<usize, Vec<String>>,
    pub spied_mobs: HashMap<usize, Vec<String>>,
    pub ice_types_mined: HashMap<usize, Vec<String>>,
    pub biomes_visited: HashMap<usize, Vec<String>>,
    pub death_types: HashMap<usize, Vec<String>>,
    pub unique_advancements: HashMap<usize, Vec<String>>,
    pub damage_taken: HashMap<usize, f64>,
    pub damage_dealt: HashMap<usize, f64>,
    pub deaths: HashMap<usize, i32>,
    pub mobs_killed: HashMap<usize, i32>,
    pub pumpkin_wear_time: HashMap<u128, i64>,
    pub distance_sprinted: HashMap<u128, i32>,
    pub unique_crafts: HashMap<u128, Vec<String>>,
    pub most_unique_crafts_player: Option<u128>,
    pub most_unique_crafts: i32,
    /// `LinkedHashMap`: the order breaks ties.
    pub levels: Vec<(u128, i32)>,
    pub most_levels_player: Option<u128>,
}

/// One online player, every server tick (`EndServerTickEventHandler`'s polling).
#[derive(Debug, Clone, Default)]
pub struct Polled {
    pub player: u128,
    pub name: String,
    /// The 41 inventory slots (`Inventory` numbering: 36..40 the armour, feet first, 40 the
    /// off hand), with the cursor's stack after them when a menu holds one.
    pub stacks: Vec<Option<Held>>,
    pub selected: usize,
    pub dimension: String,
    pub y: f64,
    /// The block at the player's feet, and the one below it.
    pub inside: String,
    pub below: String,
    pub vehicle: Option<String>,
    pub food: i32,
    pub on_fire: bool,
}

/// What a hook heard, in the terms goals care about. Player ids are UUIDs.
#[derive(Debug, Clone)]
pub enum Event {
    Tick(Box<Polled>),
    /// An advancement completed; `title` when it has a display.
    Advancement {
        player: u128,
        id: String,
        title: Option<String>,
    },
    Criterion {
        player: u128,
        advancement: String,
        criterion: String,
    },
    /// A mob died with `player` as its kill credit.
    Kill {
        player: u128,
        kind: String,
        color: Option<String>,
        dimension: String,
        damage_type: String,
    },
    Death {
        player: u128,
        damage_type: String,
        killer_kind: Option<String>,
        killer_player: Option<u128>,
        message_key: Option<String>,
        direct_kind: Option<String>,
    },
    Breed {
        player: u128,
        kind: String,
    },
    Tame {
        player: u128,
        kind: String,
    },
    /// A new effect (`MobEffect.onEffectStarted`); `active` counts it.
    EffectStarted {
        player: u128,
        effect: String,
        active: usize,
    },
    Consumed {
        player: u128,
        item: String,
        potion: Option<String>,
        food: bool,
    },
    MilkAway {
        player: u128,
    },
    Stat {
        player: u128,
        stat_type: String,
        stat: String,
        value: i32,
    },
    Mined {
        player: u128,
        block: String,
    },
    Dimension {
        player: u128,
        dimension: String,
    },
    XpLevel {
        player: u128,
        level: i32,
    },
    /// A crafting grid's result taken.
    Crafted {
        player: u128,
        item: String,
    },
    Named {
        player: u128,
        kind: String,
        name: String,
    },
    Enraged {
        player: u128,
        kind: String,
    },
    /// A potion taken out of a brewing stand.
    Brewed {
        player: u128,
        item: String,
        potion: Option<String>,
    },
    Composted {
        player: u128,
        item: String,
        food: bool,
    },
    Spied {
        player: u128,
        kind: String,
    },
    /// The mob types leashed to the player now.
    Leashed {
        player: u128,
        kinds: Vec<String>,
    },
    /// A hit that landed: `victim` and `attacker` when they are players.
    Damage {
        victim: Option<u128>,
        attacker: Option<u128>,
        arrow: bool,
        damage_type: String,
        amount: f64,
    },
    HitBy {
        shooter: u128,
        shooter_name: String,
        victim: u128,
        victim_name: String,
        projectile: String,
    },
    PickedUp {
        player: u128,
        owner: u128,
    },
    Did {
        player: u128,
        deed: Deed,
    },
}

/// Who a tooltip is for.
#[derive(Debug, Clone, Copy)]
pub enum Viewer {
    Player(usize, u128),
    Spectator,
}

const MAX_LINE_SIZE: usize = 45;

/// `HasTooltipInfo.commaSeparatedList`.
pub fn comma_separated(values: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut sb = String::new();
    let len = |s: &str| s.encode_utf16().count();
    for (i, val) in values.iter().enumerate() {
        let is_last = i + 1 == values.len();
        let words: Vec<&str> = val.split(' ').collect();
        for (j, word) in words.iter().enumerate() {
            let is_last_word = j + 1 == words.len();
            let can_fit = len(&sb) + len(word) + usize::from(is_last_word && !is_last) <= MAX_LINE_SIZE;
            if !can_fit {
                lines.push(format!("§7 §o{}", sb.trim()));
                sb.clear();
            }
            sb.push_str(word);
            if !is_last && is_last_word {
                sb.push(',');
            }
            sb.push(' ');
        }
    }
    if !sb.is_empty() {
        lines.push(format!("§7 §o{}", sb.trim()));
    }
    lines
}

/// `Utility.ticksToTimer`.
pub fn ticks_to_timer(ticks: i64) -> String {
    let ticks = ticks.abs();
    let second = (ticks / 20) % 60;
    let minute = (ticks / 20 / 60) % 60;
    let hour = (ticks / 20 / 60 / 60) % 24;
    if hour > 0 {
        format!("{hour:02}:{minute:02}:{second:02}")
    } else {
        format!("{minute:02}:{second:02}")
    }
}

fn holds(stacks: &[Option<Held>], item: &str) -> u32 {
    stacks
        .iter()
        .take(41)
        .flatten()
        .filter(|h| h.item == item)
        .map(|h| h.count)
        .sum()
}

/// Head, chest, legs, feet, as the Java goals list them.
fn armor(stacks: &[Option<Held>]) -> [Option<&Held>; 4] {
    let at = |i: usize| stacks.get(i).and_then(Option::as_ref);
    [at(39), at(38), at(37), at(36)]
}

/// The items that complete an inventory goal now (`ObtainItemsGoal.satisfiedBy`).
fn satisfied(trigger: &Trigger, stacks: &[Option<Held>]) -> bool {
    let main = || stacks.iter().take(36);
    match trigger {
        Trigger::ObtainAll { items, amount } => {
            !items.is_empty() && items.iter().all(|i| holds(stacks, i) >= (*amount).max(1))
        }
        Trigger::ObtainSome { items, amount } => {
            items.iter().filter(|i| holds(stacks, i) > 0).count() >= *amount as usize
        }
        Trigger::ObtainAny { items, amount } => items.iter().any(|i| holds(stacks, i) >= *amount),
        Trigger::StackOf64 => main().flatten().any(|h| h.count >= 64),
        Trigger::FillInventoryUnique => {
            let mut seen = HashSet::new();
            main().all(|s| s.as_ref().is_some_and(|h| seen.insert(h.item.as_str()))) && seen.len() == 36
        }
        Trigger::ShieldWithBanner => main()
            .chain(stacks.get(40))
            .flatten()
            .any(|h| h.item == "minecraft:shield" && h.base_color),
        Trigger::WearAll { items } => {
            let mut left: Vec<&String> = items.iter().collect();
            armor(stacks).iter().flatten().any(|h| {
                if let Some(i) = left.iter().position(|i| **i == h.item) {
                    left.remove(i);
                }
                left.is_empty()
            })
        }
        Trigger::WearAny { items } => armor(stacks).iter().flatten().any(|h| items.contains(&h.item)),
        Trigger::WearUniqueMaterials => {
            let slots = ["helmet", "chestplate", "leggings", "boots"];
            let mut materials = HashSet::new();
            armor(stacks).iter().zip(slots).all(|(h, slot)| {
                h.and_then(|h| armor_material(&h.item, slot))
                    .is_some_and(|m| materials.insert(m))
            })
        }
        Trigger::WearDyed { item, rgb } => armor(stacks)
            .iter()
            .flatten()
            .any(|h| &h.item == item && h.dyed == Some(*rgb)),
        Trigger::WearUniqueDyedLeather => {
            let all = Trigger::WearAll {
                items: ["helmet", "chestplate", "leggings", "boots"]
                    .map(|p| format!("minecraft:leather_{p}"))
                    .to_vec(),
            };
            let colors: HashSet<i32> = armor(stacks)
                .iter()
                .flatten()
                .filter_map(|h| h.dyed)
                .filter(|c| *c != LEATHER_RGB)
                .collect();
            satisfied(&all, stacks) && colors.len() == 4
        }
        Trigger::WearEnchanted => armor(stacks).iter().all(|h| h.is_some_and(|h| h.enchanted)),
        _ => false,
    }
}

/// The equipment asset of a piece in its slot (`WearUniqueArmorGoal`): `iron` for an iron
/// helmet on the head, `turtle_scute` for a turtle shell.
fn armor_material<'a>(item: &'a str, slot: &str) -> Option<&'a str> {
    let path = item.strip_prefix("minecraft:")?;
    if path == "turtle_helmet" {
        return (slot == "helmet").then_some("turtle_scute");
    }
    let (material, piece) = path.rsplit_once('_')?;
    (piece == slot).then_some(if material == "chainmail" { "chainmail" } else { material })
}

/// A bundle holding 16 or more stacks, every one of them a bundle (`BundleItemMixin`).
fn bundle_of_bundles(stacks: &[Option<Held>]) -> bool {
    stacks.iter().flatten().any(|h| {
        h.bundle
            .as_ref()
            .is_some_and(|c| c.len() >= 16 && c.iter().all(|i| i.ends_with("bundle")))
    })
}

impl Match {
    fn name(&self, player: u128) -> String {
        self.teams
            .iter()
            .flat_map(|t| &t.players)
            .find(|(u, _)| *u == player)
            .map_or_else(String::new, |(_, n)| n.clone())
    }

    /// The board's open squares and what completes them.
    fn open(&self) -> Vec<(usize, Trigger)> {
        self.board
            .iter()
            .enumerate()
            .filter(|(_, s)| s.completed_by.is_none())
            .map(|(i, s)| (i, s.trigger.clone()))
            .collect()
    }

    fn all(&self) -> Vec<(usize, Trigger)> {
        self.board
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.trigger.clone()))
            .collect()
    }

    fn is_open(&self, square: usize) -> bool {
        self.board[square].completed_by.is_none()
    }

    /// A `UniqueTrackedGoal`'s values for a team, as it shows and counts them.
    fn tracked(&self, trigger: &Trigger, team: usize) -> Option<(&'static str, Vec<String>, usize)> {
        let t = &self.trackers;
        let get = |m: &HashMap<usize, Vec<String>>| m.get(&team).cloned().unwrap_or_default();
        let entities = |v: Vec<String>| v.iter().map(|e| names::entity(e)).collect::<Vec<_>>();
        let items = |v: Vec<String>| v.iter().map(|e| names::item(e)).collect::<Vec<_>>();
        let paths = |v: Vec<String>| v.iter().map(|e| names::capitalized_path(e)).collect::<Vec<_>>();
        Some(match trigger {
            Trigger::UniqueAdvancements { amount } => (
                "Advancements",
                get(&t.unique_advancements)
                    .iter()
                    .map(|id| {
                        self.titles
                            .get(id)
                            .cloned()
                            .or_else(|| names::advancement(id))
                            .unwrap_or_else(|| id.clone())
                    })
                    .collect(),
                *amount,
            ),
            Trigger::KillUniqueHostile { amount } => (
                "Unique Hostile Mobs Killed",
                entities(get(&t.killed_hostile_types)),
                *amount,
            ),
            Trigger::BreedUnique { amount } => ("Animals bred", entities(get(&t.bred_animal_types)), *amount),
            Trigger::EatUnique { amount } => ("Unique Food types", items(get(&t.food_types_eaten)), *amount),
            Trigger::UniqueBiomes { amount } => ("Biomes visited", paths(get(&t.biomes_visited)), *amount),
            Trigger::BiomesAll { biomes } => (
                "Cave Biomes visited",
                paths(
                    get(&t.biomes_visited)
                        .into_iter()
                        .filter(|b| biomes.contains(b))
                        .collect(),
                ),
                biomes.len(),
            ),
            Trigger::EatAll { items: soups } => (
                "Soups eaten",
                items(
                    get(&t.food_types_eaten)
                        .into_iter()
                        .filter(|f| soups.contains(f))
                        .collect(),
                ),
                soups.len(),
            ),
            Trigger::MineUnique { amount, .. } => (
                "Ice types mined",
                get(&t.ice_types_mined).iter().map(|b| names::block(b)).collect(),
                *amount,
            ),
            Trigger::CraftUnique { amount } => (
                "Unique Items crafted",
                items(get(&t.unique_items_crafted_by_team)),
                *amount,
            ),
            Trigger::CompostUnique { amount } => ("Unique Foods composted", items(get(&t.composted_foods)), *amount),
            Trigger::SpyUnique { amount } => ("Mobs spied on", entities(get(&t.spied_mobs)), *amount),
            Trigger::DieUniqueWays { amount } => ("Ways of dying", paths(get(&t.death_types)), *amount),
            Trigger::KillAll { entities: mobs } => ("Raid mobs killed", entities(get(&t.killed_raid_mobs)), mobs.len()),
            _ => return None,
        })
    }

    fn spectator_lines(&self, value: impl Fn(usize) -> String) -> Vec<String> {
        let mut lines = vec![" ".to_owned()];
        for (i, team) in self.teams.iter().enumerate() {
            lines.push(format!(
                "{}{}§r: {}",
                legacy_color(team.color),
                team.display_name(),
                value(i)
            ));
        }
        lines.push(" ".to_owned());
        lines
    }

    /// The goal's tooltip (`HasTooltipInfo.getTooltip` / `getSpectatorTooltip`); None for a
    /// goal without one.
    pub fn tooltip(&self, square: usize, viewer: Viewer) -> Option<String> {
        let trigger = &self.board[square].trigger;
        let t = &self.trackers;
        let lines: Vec<String> = if let Some((label, _, amount)) = self.tracked(trigger, 0) {
            match viewer {
                Viewer::Player(team, _) => {
                    let (_, values, _) = self.tracked(trigger, team)?;
                    let mut lines = vec![" ".to_owned(), format!("{label}: {}/{amount}", values.len())];
                    lines.extend(comma_separated(&values));
                    lines.push(" ".to_owned());
                    lines
                }
                Viewer::Spectator => self.spectator_lines(|team| {
                    let n = self.tracked(trigger, team).map_or(0, |(_, v, _)| v.len());
                    format!("{n}/{amount}")
                }),
            }
        } else {
            match trigger {
                Trigger::KillCount { counter, amount, .. } => {
                    let map = match counter {
                        Counter::All => &t.mobs_killed,
                        Counter::Undead => &t.killed_undead_mobs,
                        Counter::Arthropods => &t.killed_arthropods,
                    };
                    let label = match counter {
                        Counter::All => "Mobs killed",
                        Counter::Undead => "Undead Mobs killed",
                        Counter::Arthropods => "Arthropods killed",
                    };
                    let n = |team: usize| map.get(&team).copied().unwrap_or(0);
                    match viewer {
                        Viewer::Player(team, _) => {
                            vec![" ".into(), format!("{label}: {}/{amount}", n(team)), " ".into()]
                        }
                        Viewer::Spectator => self.spectator_lines(|team| format!("{}/{amount}", n(team))),
                    }
                }
                Trigger::DealDamage { .. } => {
                    let d = |team: usize| (t.damage_dealt.get(&team).copied().unwrap_or(0.0) as i32).min(400);
                    match viewer {
                        Viewer::Player(team, _) => vec![" ".into(), format!("Damage: {}/400", d(team)), " ".into()],
                        Viewer::Spectator => self.spectator_lines(|team| format!("{}/400", d(team))),
                    }
                }
                Trigger::TakeDamage { .. } => {
                    let d = |team: usize| t.damage_taken.get(&team).copied().unwrap_or(0.0) as i32;
                    match viewer {
                        Viewer::Player(team, _) => {
                            vec![" ".into(), format!("Damage: {}/200", d(team).min(200)), " ".into()]
                        }
                        // The Java mod caps the spectators' line at 400.
                        Viewer::Spectator => self.spectator_lines(|team| format!("{}/200", d(team).min(400))),
                    }
                }
                Trigger::WearPumpkin { ticks } => {
                    let worn = |p: u128| t.pumpkin_wear_time.get(&p).copied().unwrap_or(0).min(*ticks);
                    match viewer {
                        Viewer::Player(team, player) => {
                            let time = ticks_to_timer(worn(player));
                            let mut lines = vec![" ".to_owned(), format!("Time worn: {time}")];
                            let players = &self.teams[team].players;
                            if players.len() > 1 {
                                lines.push(" ".into());
                                // As the Java mod: teammates are shown with the viewer's time.
                                for (u, n) in players {
                                    if *u != player {
                                        lines.push(format!("{n}: {time}"));
                                    }
                                }
                            }
                            lines.push(" ".into());
                            lines
                        }
                        Viewer::Spectator => {
                            let mut lines = vec![" ".to_owned()];
                            for team in &self.teams {
                                for (u, n) in &team.players {
                                    lines.push(format!(
                                        "{}{n}§r: {}",
                                        legacy_color(team.color),
                                        ticks_to_timer(worn(*u))
                                    ));
                                }
                            }
                            lines.push(" ".into());
                            lines
                        }
                    }
                }
                _ => return None,
            }
        };
        Some(lines.join("\n"))
    }
}

impl State {
    fn game(&self) -> Option<&Match> {
        self.game.as_ref()
    }

    fn team(&self, player: u128) -> Option<usize> {
        self.game()?.team_of(player)
    }

    /// `LockoutTeamServer.sendTooltipUpdate`: each online player of the team gets their own
    /// tooltip, and the spectators theirs when `spectators`.
    pub fn tooltip_update(&self, square: usize, team: usize, spectators: bool, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = self.game() else { return };
        let goal = m.board[square].id.clone();
        for (p, _) in &m.teams[team].players {
            if !online.contains(p) {
                continue;
            }
            if let Some(tooltip) = m.tooltip(square, Viewer::Player(team, *p)) {
                out.push(Action::Send {
                    player: *p,
                    payload: Payload::UpdateTooltip {
                        goal: goal.clone(),
                        tooltip,
                    },
                });
            }
        }
        if spectators && let Some(tooltip) = m.tooltip(square, Viewer::Spectator) {
            for s in online.iter().filter(|p| m.team_of(**p).is_none()) {
                out.push(Action::Send {
                    player: *s,
                    payload: Payload::UpdateTooltip {
                        goal: goal.clone(),
                        tooltip: tooltip.clone(),
                    },
                });
            }
        }
    }

    /// `Lockout.completeGoal(goal, player)`: "<player> completed <goal>."
    pub fn complete_player(&mut self, square: usize, player: u128, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = self.game() else { return };
        let Some(team) = m.team_of(player) else { return };
        if !m.is_open(square) || !m.started {
            return;
        }
        let message = format!("{} completed {}.", m.name(player), m.board[square].name);
        self.complete(square, team, message, online, out);
    }

    /// `Lockout.completeGoal(goal, team)`: "<team> completed <goal>."
    pub fn complete_team(&mut self, square: usize, team: usize, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = self.game() else { return };
        let message = format!("{} completed {}.", m.teams[team].display_name(), m.board[square].name);
        self.complete(square, team, message, online, out);
    }

    /// `Lockout.complete1v1Goal(goal, player, isWinner, message)`.
    pub fn complete_1v1_player(
        &mut self,
        square: usize,
        player: u128,
        winner: bool,
        message: String,
        online: &[u128],
        out: &mut Vec<Action>,
    ) {
        if let Some(team) = self.team(player) {
            self.complete_1v1(square, team, winner, message, online, out);
        }
    }

    /// `Lockout.complete1v1Goal(goal, team, isWinner, message)`: the goal goes to `team` or
    /// to the first other team; only those two teams hear of it.
    pub fn complete_1v1(
        &mut self,
        square: usize,
        team: usize,
        winner: bool,
        message: String,
        online: &[u128],
        out: &mut Vec<Action>,
    ) {
        let Some(m) = self.game() else { return };
        if !m.is_open(square) || !m.started || !m.running {
            return;
        }
        let Some(opponent) = (0..m.teams.len()).find(|t| *t != team) else {
            return;
        };
        let (won, lost) = if winner { (team, opponent) } else { (opponent, team) };
        self.complete_quietly(square, won, out);
        let Some(m) = self.game() else { return };
        for (p, _) in &m.teams[won].players {
            out.push(crate::server::tell(*p, message.clone(), 0x55FF55));
        }
        for (p, _) in &m.teams[lost].players {
            out.push(crate::server::tell(*p, message.clone(), 0xFF5555));
        }
        for s in online.iter().filter(|p| m.team_of(**p).is_none()) {
            out.push(crate::server::tell(*s, message.clone(), 0xFFFFFF));
        }
        self.finish_completion(square, won, out);
    }

    /// A `UniqueTrackedGoal`'s `evaluate`: the tooltip, then the goal once there are enough.
    fn evaluate_tracked(&mut self, square: usize, team: usize, online: &[u128], out: &mut Vec<Action>) {
        let Some(m) = self.game() else { return };
        if !m.is_open(square) {
            return;
        }
        let trigger = m.board[square].trigger.clone();
        let Some((_, values, amount)) = m.tracked(&trigger, team) else {
            return;
        };
        self.tooltip_update(square, team, true, online, out);
        if values.len() >= amount {
            self.complete_team(square, team, online, out);
        }
    }

    /// `Lockout.updateGoalCompletion`: whoever held the goal loses it (no packet), and the
    /// player's team takes it.
    pub(crate) fn update_goal_completion(
        &mut self,
        square: usize,
        player: u128,
        online: &[u128],
        out: &mut Vec<Action>,
    ) {
        if let Some(m) = &mut self.game
            && let Some(t) = m.board[square].completed_by.take()
        {
            m.teams[t].points -= 1;
        }
        self.complete_player(square, player, online, out);
    }

    /// `Lockout.recalculateXPGoal`, every tick with every online player's level (0 while dead).
    pub fn recalculate_xp(&mut self, levels: &[(u128, i32)], online: &[u128], out: &mut Vec<Action>) {
        let squares: Vec<usize> = match self.game() {
            Some(m) => m
                .all()
                .into_iter()
                .filter(|(_, t)| *t == Trigger::MostXpLevels)
                .map(|(i, _)| i)
                .collect(),
            None => return,
        };
        for square in squares {
            let Some(m) = &mut self.game else { return };
            for (p, l) in levels {
                match m.trackers.levels.iter_mut().find(|(u, _)| u == p) {
                    Some(entry) => entry.1 = *l,
                    None => m.trackers.levels.push((*p, *l)),
                }
            }
            let mut largest = 0;
            let mut leaders: Vec<u128> = Vec::new();
            for (u, l) in &m.trackers.levels {
                if *l == largest {
                    leaders.push(*u);
                } else if *l > largest {
                    leaders.clear();
                    leaders.push(*u);
                    largest = *l;
                }
            }
            if largest == 0 {
                if m.trackers.most_levels_player.take().is_some() {
                    self.clear(square, out);
                }
                continue;
            }
            if !m.trackers.most_levels_player.is_some_and(|p| leaders.contains(&p)) {
                m.trackers.most_levels_player = Some(leaders[0]);
                self.update_goal_completion(square, leaders[0], online, out);
            }
        }
    }

    /// The goals `event` completes or moves along.
    pub fn handle(&mut self, event: Event, online: &[u128]) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(m) = self.game() else { return out };
        if !m.running || self.pause.is_some() {
            return out;
        }
        let o = &mut out;
        match event {
            Event::Tick(p) => self.polled(&p, online, o),
            Event::Advancement { player, id, title } => {
                let Some(team) = self.team(player) else { return out };
                if let Some(title) = &title
                    && let Some(m) = &mut self.game
                {
                    m.titles.insert(id.clone(), title.clone());
                }
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::Advancement { ids } if ids.contains(&id) => {
                            self.complete_player(sq, player, online, o)
                        }
                        Trigger::UniqueAdvancements { amount } if title.is_some() => {
                            let size = self.with_trackers(|t| {
                                let set = t.unique_advancements.entry(team).or_default();
                                insert(set, &id);
                                set.len()
                            });
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Criterion {
                player,
                advancement,
                criterion,
            } => {
                if advancement != "minecraft:adventure/adventuring_time" {
                    return out;
                }
                let biome = if criterion.contains(':') {
                    criterion
                } else {
                    format!("minecraft:{criterion}")
                };
                if let Some(team) = self.team(player)
                    && self.with_trackers(|t| insert(t.biomes_visited.entry(team).or_default(), &biome))
                {
                    for (sq, trigger) in self.game().map(Match::all).unwrap_or_default() {
                        if matches!(trigger, Trigger::BiomesAll { .. } | Trigger::UniqueBiomes { .. }) {
                            self.evaluate_tracked(sq, team, online, o);
                        }
                    }
                }
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    if let Trigger::Biome { biomes } = trigger
                        && biomes.contains(&biome)
                    {
                        self.complete_player(sq, player, online, o);
                    }
                }
            }
            Event::Kill {
                player,
                kind,
                color,
                dimension,
                damage_type,
            } => {
                let Some(team) = self.team(player) else { return out };
                self.with_trackers(|t| *t.mobs_killed.entry(team).or_default() += 1);
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    if !self.game().is_some_and(|m| m.is_open(sq)) {
                        continue;
                    }
                    match trigger {
                        Trigger::Kill {
                            entities,
                            color: c,
                            dimension: d,
                            damage_type: dt,
                        } => {
                            if entities.contains(&kind)
                                && (c.is_none() || c == color)
                                && d.as_ref().is_none_or(|d| *d == dimension)
                                && dt.as_ref().is_none_or(|dt| *dt == damage_type)
                            {
                                self.complete_player(sq, player, online, o);
                            }
                        }
                        Trigger::KillAll { entities } if entities.contains(&kind) => {
                            let size = self.with_trackers(|t| {
                                let set = t.killed_raid_mobs.entry(team).or_default();
                                insert(set, &kind);
                                set.len()
                            });
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= entities.len() {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        Trigger::KillUniqueHostile { amount } if HOSTILE.contains(&kind.as_str()) => {
                            let size = self.with_trackers(|t| {
                                let set = t.killed_hostile_types.entry(team).or_default();
                                insert(set, &kind);
                                set.len()
                            });
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        Trigger::KillCount {
                            counter: Counter::All,
                            amount,
                            ..
                        } => {
                            let size = self.with_trackers(|t| t.mobs_killed.get(&team).copied().unwrap_or(0));
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount as i32 {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        Trigger::KillCount {
                            counter,
                            entities,
                            amount,
                        } if entities.contains(&kind) => {
                            let size = self.with_trackers(|t| {
                                let map = if counter == Counter::Undead {
                                    &mut t.killed_undead_mobs
                                } else {
                                    &mut t.killed_arthropods
                                };
                                let n = map.entry(team).or_default();
                                *n += 1;
                                *n
                            });
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount as i32 {
                                self.complete_player(sq, player, online, o);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Death {
                player,
                damage_type,
                killer_kind,
                killer_player,
                message_key,
                direct_kind,
            } => {
                let Some(team) = self.team(player) else { return out };
                let deaths = self.with_trackers(|t| {
                    let n = t.deaths.entry(team).or_default();
                    *n += 1;
                    *n
                });
                if self.with_trackers(|t| insert(t.death_types.entry(team).or_default(), &damage_type)) {
                    for (sq, trigger) in self.game().map(Match::all).unwrap_or_default() {
                        if matches!(trigger, Trigger::DieUniqueWays { .. }) {
                            self.evaluate_tracked(sq, team, online, o);
                        }
                    }
                }
                let name = self.game().map(|m| m.name(player)).unwrap_or_default();
                let display = self.game().map(|m| m.teams[team].display_name()).unwrap_or_default();
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::Opponent(OpponentEvent::Dies { times: 1 }) => {
                            self.complete_1v1_player(sq, player, false, format!("{name} died."), online, o);
                        }
                        Trigger::Opponent(OpponentEvent::Dies { times }) if deaths >= times => {
                            self.complete_1v1_player(sq, player, false, format!("{display} died 3 times."), online, o);
                        }
                        Trigger::DieTo { damage_types } if damage_types.contains(&damage_type) => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::DieToEntity { entities }
                            if killer_kind.as_ref().is_some_and(|k| entities.contains(k)) =>
                        {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::DieFalling { keys } if message_key.as_ref().is_some_and(|k| keys.contains(k)) => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::DieToTntMinecart if direct_kind.as_deref() == Some("minecraft:tnt_minecart") => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::KillOtherPlayer => {
                            if let Some(k) = killer_player
                                && k != player
                                && self.team(k) != Some(team)
                            {
                                self.complete_player(sq, k, online, o);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Breed { player, kind } => {
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::Breed { entities } if entities.contains(&kind) => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::BreedUnique { amount } => {
                            let Some(team) = self.team(player) else { continue };
                            let size = self.with_trackers(|t| {
                                let set = t.bred_animal_types.entry(team).or_default();
                                insert(set, &kind);
                                set.len()
                            });
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Tame { player, kind } => {
                if let Some((sq, _)) = self
                    .game()
                    .map(Match::open)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|(_, t)| matches!(t, Trigger::Tame { entities } if entities.contains(&kind)))
                {
                    self.complete_player(sq, player, online, o);
                }
            }
            Event::EffectStarted { player, effect, active } => {
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::Effect { effects } if effects.contains(&effect) => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::EffectsAtOnce { amount } if active >= amount => {
                            self.complete_player(sq, player, online, o);
                        }
                        _ => {}
                    }
                }
            }
            Event::Consumed {
                player,
                item,
                potion,
                food,
            } => {
                // `PotionContentsComponentMixin`.
                if let Some(potion) = &potion {
                    for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                        if let Trigger::DrinkPotion { potions } = trigger
                            && potions.contains(potion)
                        {
                            self.complete_player(sq, player, online, o);
                        }
                    }
                }
                // `FoodComponentMixin`.
                let Some(team) = self.team(player).filter(|_| food) else {
                    return out;
                };
                let new_food = self.with_trackers(|t| insert(t.food_types_eaten.entry(team).or_default(), &item));
                let name = self.game().map(|m| m.name(player)).unwrap_or_default();
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::EatAll { .. } if new_food => self.evaluate_tracked(sq, team, online, o),
                        Trigger::Consume { items } if items.contains(&item) => {
                            self.complete_player(sq, player, online, o);
                        }
                        Trigger::EatUnique { amount } => {
                            let size = self.with_trackers(|t| t.food_types_eaten.get(&team).map_or(0, Vec::len));
                            self.tooltip_update(sq, team, true, online, o);
                            if size >= amount {
                                self.complete_team(sq, team, online, o);
                            }
                        }
                        Trigger::Opponent(OpponentEvent::EatsFood) => {
                            self.complete_1v1_player(sq, player, false, format!("{name} ate food."), online, o);
                        }
                        _ => {}
                    }
                }
            }
            Event::MilkAway { player } => {
                if let Some((sq, _)) = self
                    .game()
                    .map(Match::open)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|(_, t)| *t == Trigger::MilkAwayEffect)
                {
                    self.complete_player(sq, player, online, o);
                }
            }
            Event::Stat {
                player,
                stat_type,
                stat,
                value,
            } => {
                let name = self.game().map(|m| m.name(player)).unwrap_or_default();
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match (stat_type.as_str(), trigger) {
                        ("minecraft:custom", Trigger::Stat { stats }) if stats.contains(&stat) => {
                            self.complete_player(sq, player, online, o);
                        }
                        ("minecraft:custom", Trigger::Opponent(OpponentEvent::EatsFood))
                            if stat == "minecraft:eat_cake_slice" =>
                        {
                            self.complete_1v1_player(sq, player, false, format!("{name} ate food."), online, o);
                        }
                        ("minecraft:custom", Trigger::SprintCm { cm }) if stat == "minecraft:sprint_one_cm" => {
                            let total = self.with_trackers(|t| {
                                let n = t.distance_sprinted.entry(player).or_default();
                                *n += value;
                                *n
                            });
                            if total >= cm {
                                self.complete_player(sq, player, online, o);
                            }
                        }
                        ("minecraft:broken", Trigger::BreakItem { matcher }) if matcher.matches(&stat) => {
                            self.complete_player(sq, player, online, o);
                        }
                        ("minecraft:used", Trigger::PlaceItem { matcher }) if matcher.matches(&stat) => {
                            self.complete_player(sq, player, online, o);
                        }
                        ("minecraft:used", Trigger::Deed(Deed::PlaceEndCrystal)) if stat == "minecraft:end_crystal" => {
                            self.complete_player(sq, player, online, o);
                        }
                        _ => {}
                    }
                }
            }
            Event::Mined { player, block } => {
                let ice = ["minecraft:ice", "minecraft:packed_ice", "minecraft:blue_ice"];
                if ice.contains(&block.as_str())
                    && let Some(team) = self.team(player)
                    && self.with_trackers(|t| insert(t.ice_types_mined.entry(team).or_default(), &block))
                {
                    for (sq, trigger) in self.game().map(Match::all).unwrap_or_default() {
                        if matches!(trigger, Trigger::MineUnique { .. }) {
                            self.evaluate_tracked(sq, team, online, o);
                        }
                    }
                }
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    if let Trigger::Mine { blocks } = trigger
                        && blocks.contains(&block)
                    {
                        self.complete_player(sq, player, online, o);
                    }
                }
            }
            Event::Dimension { player, dimension } => self.complete_where(
                player,
                online,
                o,
                |t| matches!(t, Trigger::Dimension { dimension: d } if *d == dimension),
            ),
            Event::XpLevel { player, level } => self.complete_where(
                player,
                online,
                o,
                |t| matches!(t, Trigger::XpLevel { level: l } if level >= *l),
            ),
            Event::Crafted { player, item } => self.crafted(player, &item, online, o),
            Event::Named { player, kind, name } => self.complete_where(
                player,
                online,
                o,
                |t| matches!(t, Trigger::Name { entity, names } if *entity == kind && names.contains(&name)),
            ),
            Event::Enraged { player, kind } => self.complete_where(
                player,
                online,
                o,
                |t| matches!(t, Trigger::Enrage { entity } if *entity == kind),
            ),
            Event::Brewed { player, item, potion } => self.complete_where(player, online, o, |t| match t {
                Trigger::Brewed { potions } => potion.as_ref().is_some_and(|p| potions.contains(p)),
                Trigger::BrewLingering => item == "minecraft:lingering_potion",
                _ => false,
            }),
            Event::Composted { player, item, food } => {
                if let Some(team) = self.team(player)
                    && food
                    && self.with_trackers(|t| insert(t.composted_foods.entry(team).or_default(), &item))
                {
                    for (sq, trigger) in self.game().map(Match::all).unwrap_or_default() {
                        if matches!(trigger, Trigger::CompostUnique { .. }) {
                            self.evaluate_tracked(sq, team, online, o);
                        }
                    }
                }
            }
            Event::Spied { player, kind } => {
                let Some(team) = self.team(player) else { return out };
                let new_mob = self.with_trackers(|t| insert(t.spied_mobs.entry(team).or_default(), &kind));
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    match trigger {
                        Trigger::SpyUnique { .. } if new_mob => self.evaluate_tracked(sq, team, online, o),
                        Trigger::Spy { entity } if entity == kind => self.complete_player(sq, player, online, o),
                        _ => {}
                    }
                }
            }
            Event::Leashed { player, kinds } => self.complete_where(player, online, o, |t| match t {
                Trigger::LeashUnique { amount } => kinds.len() >= *amount,
                Trigger::LeashIronGolem => kinds.iter().any(|k| k == "minecraft:iron_golem"),
                _ => false,
            }),
            Event::Damage {
                victim,
                attacker,
                arrow,
                damage_type,
                amount,
            } => self.damage(victim, attacker, arrow, &damage_type, amount, online, o),
            Event::HitBy {
                shooter,
                shooter_name,
                victim,
                victim_name,
                projectile,
            } => {
                if shooter == victim {
                    return out;
                }
                let thing = if projectile == "minecraft:egg" {
                    "an Egg"
                } else {
                    "a Snowball"
                };
                for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                    if let Trigger::Opponent(OpponentEvent::HitBy(kind)) = trigger
                        && kind == projectile
                    {
                        let message = format!("{shooter_name} hit {victim_name} with {thing}.");
                        self.complete_1v1_player(sq, shooter, true, message, online, o);
                    }
                }
            }
            Event::PickedUp { player, owner } => {
                let (Some(mine), Some(theirs)) = (self.team(player), self.team(owner)) else {
                    return out;
                };
                if mine != theirs {
                    self.complete_where(player, online, o, |t| *t == Trigger::PickUpOpponentItem);
                }
            }
            Event::Did { player, deed } => self.complete_where(player, online, o, |t| match t {
                Trigger::Deed(Deed::GlowInkNether) => matches!(deed, Deed::GlowInkCrimson | Deed::GlowInkWarped),
                Trigger::Kill { entities, .. } => {
                    deed == Deed::KillCreaking && entities.iter().any(|e| e == "minecraft:creaking")
                }
                Trigger::Deed(d) => *d == deed,
                _ => false,
            }),
        }
        out
    }

    fn with_trackers<R>(&mut self, f: impl FnOnce(&mut Trackers) -> R) -> R {
        let mut empty = Trackers::default();
        f(self.game.as_mut().map_or(&mut empty, |m| &mut m.trackers))
    }

    /// Completes, for `player`, every open goal `which` picks.
    fn complete_where(
        &mut self,
        player: u128,
        online: &[u128],
        out: &mut Vec<Action>,
        which: impl Fn(&Trigger) -> bool,
    ) {
        for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
            if which(&trigger) {
                self.complete_player(sq, player, online, out);
            }
        }
    }

    /// `EndServerTickEventHandler` for one player: inventory goals, riding, hunger, heights,
    /// water; `EntityMixin` for fire; the carved pumpkin's timer.
    fn polled(&mut self, p: &Polled, online: &[u128], out: &mut Vec<Action>) {
        let player = p.player;
        for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
            if !self.game().is_some_and(|m| m.is_open(sq)) {
                continue;
            }
            match &trigger {
                Trigger::Opponent(OpponentEvent::Obtains(item, name)) => {
                    if holds(&p.stacks, item) > 0 {
                        let message = format!("{} obtained {name}.", p.name);
                        self.complete_1v1_player(sq, player, false, message, online, out);
                    }
                }
                Trigger::WearPumpkin { ticks } => {
                    let Some(team) = self.team(player) else { continue };
                    if armor(&p.stacks)
                        .iter()
                        .flatten()
                        .any(|h| h.item == "minecraft:carved_pumpkin")
                    {
                        let worn = self.with_trackers(|t| {
                            let n = t.pumpkin_wear_time.entry(player).or_default();
                            *n += 1;
                            *n
                        });
                        if worn % 20 == 0 {
                            self.tooltip_update(sq, team, true, online, out);
                        }
                        if worn >= *ticks {
                            self.complete_player(sq, player, online, out);
                        }
                    }
                }
                Trigger::Ride { entities } => {
                    if let Some(v) = &p.vehicle
                        && entities.contains(v)
                    {
                        let holding = p.stacks.get(p.selected).and_then(Option::as_ref);
                        if v != "minecraft:pig" || holding.is_some_and(|h| h.item == "minecraft:carrot_on_a_stick") {
                            self.complete_player(sq, player, online, out);
                        }
                    }
                }
                Trigger::EmptyHunger if p.food == 0 => self.complete_player(sq, player, online, out),
                Trigger::Height { dimension, min_y } if p.y >= *min_y && p.dimension == *dimension => {
                    self.complete_player(sq, player, online, out);
                }
                Trigger::ReachBedrock if p.y < 10.0 && p.below == "minecraft:bedrock" => {
                    self.complete_player(sq, player, online, out);
                }
                Trigger::Opponent(OpponentEvent::TouchesWater) if p.inside == "minecraft:water" => {
                    let message = format!("{} touched water.", p.name);
                    self.complete_1v1_player(sq, player, false, message, online, out);
                }
                Trigger::Opponent(OpponentEvent::CatchesFire) if p.on_fire => {
                    let message = format!("{} caught on fire.", p.name);
                    self.complete_1v1_player(sq, player, false, message, online, out);
                }
                Trigger::Deed(Deed::FillBundleWithBundles) if bundle_of_bundles(&p.stacks) => {
                    self.complete_player(sq, player, online, out);
                }
                t if satisfied(t, &p.stacks) => self.complete_player(sq, player, online, out),
                _ => {}
            }
        }
    }

    /// `CraftingResultSlotMixin`.
    fn crafted(&mut self, player: u128, item: &str, online: &[u128], out: &mut Vec<Action>) {
        if let Some(team) = self.team(player) {
            let new_team_craft =
                self.with_trackers(|t| insert(t.unique_items_crafted_by_team.entry(team).or_default(), item));
            let trim = item.ends_with("_armor_trim_smithing_template");
            for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                match trigger {
                    Trigger::CraftUnique { .. } if new_team_craft => self.evaluate_tracked(sq, team, online, out),
                    Trigger::CraftItemSuffix { .. } if trim => self.complete_player(sq, player, online, out),
                    _ => {}
                }
            }
        }
        let (added, size) = self.with_trackers(|t| {
            let crafts = t.unique_crafts.entry(player).or_default();
            let added = insert(crafts, item);
            (added, crafts.len() as i32)
        });
        if !added {
            return;
        }
        for (sq, trigger) in self.game().map(Match::all).unwrap_or_default() {
            if trigger != Trigger::MostUniqueCrafts {
                continue;
            }
            out.push(Action::Sound {
                player,
                sound: "minecraft:block.note_block.iron_xylophone",
                category: "blocks",
                volume: 2.0,
                pitch: 2.0,
            });
            if size % 5 == 0 {
                out.push(Action::Tell {
                    player,
                    spans: vec![Span::new(format!("You have crafted {size} unique items."), 0xAAAAAA).italic()],
                });
            }
            out.push(Action::ActionBar {
                player,
                spans: vec![Span::new(format!("Unique crafts: {size}"), 0xFFFFFF)],
            });
            let (most, holder) = self.with_trackers(|t| (t.most_unique_crafts, t.most_unique_crafts_player));
            if size > most {
                if holder != Some(player) {
                    self.update_goal_completion(sq, player, online, out);
                }
                self.with_trackers(|t| {
                    t.most_unique_crafts_player = Some(player);
                    t.most_unique_crafts = size;
                });
            }
        }
    }

    /// `LivingEntityMixin.onDamage` (a player's hit on anything) and `PlayerMixin.onDamage`
    /// (a hit on a player).
    #[allow(clippy::too_many_arguments)]
    fn damage(
        &mut self,
        victim: Option<u128>,
        attacker: Option<u128>,
        arrow: bool,
        damage_type: &str,
        amount: f64,
        online: &[u128],
        out: &mut Vec<Action>,
    ) {
        if let Some(attacker) = attacker
            && let Some(team) = self.team(attacker)
        {
            let dealt = self.with_trackers(|t| {
                let d = t.damage_dealt.entry(team).or_default();
                *d += amount;
                *d
            });
            for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
                match trigger {
                    Trigger::ShootOpponentWithArrow if arrow => {
                        if let Some(v) = victim
                            && self.team(v).is_some_and(|vt| vt != team)
                        {
                            self.complete_player(sq, attacker, online, out);
                        }
                    }
                    Trigger::DealDamage { amount: needed } => {
                        self.tooltip_update(sq, team, true, online, out);
                        if dealt >= needed {
                            self.complete_player(sq, attacker, online, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let Some(victim) = victim else { return };
        let Some(team) = self.team(victim) else { return };
        let taken = self.with_trackers(|t| {
            let d = t.damage_taken.entry(team).or_default();
            *d += amount;
            *d
        });
        let name = self.game().map(|m| m.name(victim)).unwrap_or_default();
        let display = self.game().map(|m| m.teams[team].display_name()).unwrap_or_default();
        for (sq, trigger) in self.game().map(Match::open).unwrap_or_default() {
            match trigger {
                Trigger::TakeDamage { amount: needed } => {
                    self.tooltip_update(sq, team, true, online, out);
                    if taken >= needed {
                        self.complete_team(sq, team, online, out);
                    }
                }
                Trigger::Opponent(OpponentEvent::FallDamage) if damage_type == "minecraft:fall" => {
                    self.complete_1v1_player(sq, victim, false, format!("{name} took fall damage."), online, out);
                }
                Trigger::Opponent(OpponentEvent::TakesDamage(limit)) if taken >= limit => {
                    self.complete_1v1(sq, team, false, format!("{display} took 100 damage."), online, out);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_wrap_as_the_java_mod_wraps_them() {
        let names: Vec<String> = [
            "Zombie",
            "Skeleton",
            "Creeper",
            "Spider",
            "Enderman",
            "Witch",
            "Zombified Piglin",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            comma_separated(&names),
            vec![
                "§7 §oZombie, Skeleton, Creeper, Spider, Enderman,",
                "§7 §oWitch, Zombified Piglin"
            ]
        );
        assert!(comma_separated(&[]).is_empty());
    }

    #[test]
    fn timers_read_as_java_prints_them() {
        assert_eq!(ticks_to_timer(0), "00:00");
        assert_eq!(ticks_to_timer(20 * 61), "01:01");
        assert_eq!(ticks_to_timer(20 * 3601), "01:00:01");
    }

    #[test]
    fn unique_armor_needs_four_materials_in_their_slots() {
        let mut stacks: Vec<Option<Held>> = vec![None; 41];
        stacks[39] = Some(Held::new("minecraft:turtle_helmet", 1));
        stacks[38] = Some(Held::new("minecraft:iron_chestplate", 1));
        stacks[37] = Some(Held::new("minecraft:golden_leggings", 1));
        stacks[36] = Some(Held::new("minecraft:chainmail_boots", 1));
        assert!(satisfied(&Trigger::WearUniqueMaterials, &stacks));
        stacks[36] = Some(Held::new("minecraft:iron_boots", 1));
        assert!(!satisfied(&Trigger::WearUniqueMaterials, &stacks));
    }
}
