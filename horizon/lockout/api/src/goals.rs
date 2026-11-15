//! The goal table, shared with the Fabric mod: `goals.json` is extracted from its Java
//! sources (`horizon/tools/extract_goals.py`), so ids, names and board rules are the same
//! on both. This file turns each entry into what the board draws and what the server
//! watches for.
use horizon_client::mod_api::serde_json::{self, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

const GOALS_JSON: &str = include_str!("../data/goals.json");

/// No data: the Java mod's `GoalDataConstants.DATA_NONE`.
pub const DATA_NONE: &str = "null";

/// The 16 dye colours, the values a coloured goal's data takes.
pub const DYES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// Mobs that count for "Kill N unique hostile mobs" (vanilla's `Enemy` mobs).
pub const HOSTILE: &[&str] = &[
    "minecraft:blaze",
    "minecraft:bogged",
    "minecraft:breeze",
    "minecraft:cave_spider",
    "minecraft:creaking",
    "minecraft:creeper",
    "minecraft:drowned",
    "minecraft:elder_guardian",
    "minecraft:ender_dragon",
    "minecraft:enderman",
    "minecraft:endermite",
    "minecraft:evoker",
    "minecraft:ghast",
    "minecraft:guardian",
    "minecraft:hoglin",
    "minecraft:husk",
    "minecraft:magma_cube",
    "minecraft:parched",
    "minecraft:phantom",
    "minecraft:piglin",
    "minecraft:piglin_brute",
    "minecraft:pillager",
    "minecraft:ravager",
    "minecraft:shulker",
    "minecraft:silverfish",
    "minecraft:skeleton",
    "minecraft:slime",
    "minecraft:spider",
    "minecraft:stray",
    "minecraft:vex",
    "minecraft:vindicator",
    "minecraft:warden",
    "minecraft:witch",
    "minecraft:wither",
    "minecraft:wither_skeleton",
    "minecraft:zoglin",
    "minecraft:zombie",
    "minecraft:zombie_nautilus",
    "minecraft:zombie_villager",
    "minecraft:zombified_piglin",
];

/// What completes a goal on a Horizon server. `Unsupported` goals still draw on the board
/// (a Fabric server may send them) but aren't put on boards this port generates.
#[derive(Debug, Clone, PartialEq)]
pub enum Trigger {
    /// Hold every item, each at least `amount` times.
    ObtainAll {
        items: Vec<String>,
        amount: u32,
    },
    /// Hold `amount` of the listed items, any of them.
    ObtainSome {
        items: Vec<String>,
        amount: u32,
    },
    /// Hold `amount` of any one listed item.
    ObtainAny {
        items: Vec<String>,
        amount: u32,
    },
    /// A stack of 64 of anything.
    StackOf64,
    /// Every inventory slot full, no item twice.
    FillInventoryUnique,
    /// A potion item of one of these potions.
    ObtainPotion {
        potions: Vec<String>,
    },
    WearAll {
        items: Vec<String>,
    },
    WearAny {
        items: Vec<String>,
    },
    WearUniqueMaterials,
    /// A worn piece of `item` dyed exactly `rgb`.
    WearDyed {
        item: String,
        rgb: i32,
    },
    /// Full leather armour in four colours, none undyed.
    WearUniqueDyedLeather,
    Advancement {
        ids: Vec<String>,
    },
    UniqueAdvancements {
        amount: usize,
    },
    Kill {
        entities: Vec<String>,
        color: Option<String>,
        dimension: Option<String>,
    },
    KillCount {
        entities: Option<Vec<String>>,
        amount: u32,
    },
    KillAll {
        entities: Vec<String>,
    },
    KillUniqueHostile {
        amount: usize,
    },
    DieTo {
        damage_types: Vec<String>,
    },
    DieToEntity {
        entities: Vec<String>,
    },
    DieByMessage {
        key: String,
    },
    DieUniqueWays {
        amount: usize,
    },
    Breed {
        entities: Vec<String>,
    },
    BreedUnique {
        amount: usize,
    },
    Tame {
        entities: Vec<String>,
    },
    Effect {
        effects: Vec<String>,
    },
    EffectsAtOnce {
        amount: usize,
    },
    Consume {
        items: Vec<String>,
    },
    DrinkPotion {
        potions: Vec<String>,
    },
    EatUnique {
        amount: usize,
    },
    EatAll {
        items: Vec<String>,
    },
    Stat {
        stats: Vec<String>,
    },
    Mine {
        blocks: Vec<String>,
    },
    MineUnique {
        blocks: Vec<String>,
        amount: usize,
    },
    Dimension {
        dimension: String,
    },
    Biome {
        biomes: Vec<String>,
    },
    BiomesAll {
        biomes: Vec<String>,
    },
    UniqueBiomes {
        amount: usize,
    },
    XpLevel {
        level: i32,
    },
    CraftUnique {
        amount: usize,
    },
    CraftItemSuffix {
        suffix: String,
    },
    Ride {
        entities: Vec<String>,
    },
    Name {
        entity: String,
        name: String,
    },
    BreakItem {
        matcher: ItemMatch,
    },
    PlaceBlockSuffix {
        suffix: String,
    },
    PlaceHanging {
        kind: String,
    },
    UseItem {
        item: String,
    },
    MilkAwayEffect,
    EmptyHunger,
    Height {
        dimension: String,
        min_y: f64,
    },
    StandOn {
        dimension: String,
        block: String,
    },
    Enrage {
        entity: String,
    },
    ShieldDisabled,
    ItemFrameInItemFrame,
    WolfArmor,
    Stonecutter,
    Loom,
    Anvil,
    KillOtherPlayer,
    ShootOpponentWithArrow,
    PickUpOpponentItem,
    TakeDamage {
        amount: f32,
    },
    DealDamage {
        amount: f32,
    },
    SprintCm {
        cm: i32,
    },
    MostXpLevels,
    MostUniqueCrafts,
    /// Completed for the other team when the event happens to a player (1v1 goals).
    Opponent(OpponentEvent),
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemMatch {
    Any(Vec<String>),
    Armor,
    Tool,
}

impl ItemMatch {
    pub fn matches(&self, item: &str) -> bool {
        let path = item.rsplit(':').next().unwrap_or(item);
        match self {
            ItemMatch::Any(items) => items.iter().any(|i| i == item),
            ItemMatch::Armor => ["_helmet", "_chestplate", "_leggings", "_boots"]
                .iter()
                .any(|s| path.ends_with(s)),
            ItemMatch::Tool => {
                ["_pickaxe", "_axe", "_shovel", "_hoe", "_sword", "_spear"]
                    .iter()
                    .any(|s| path.ends_with(s))
                    || [
                        "fishing_rod",
                        "flint_and_steel",
                        "shears",
                        "brush",
                        "carrot_on_a_stick",
                        "warped_fungus_on_a_stick",
                        "bow",
                        "crossbow",
                        "trident",
                        "mace",
                        "shield",
                    ]
                    .contains(&path)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpponentEvent {
    Obtains(String),
    Dies { times: u32 },
    CatchesFire,
    HitBy(String),
    TakesDamage(f32),
    FallDamage,
    TouchesWater,
    EatsFood,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Requirements {
    pub biomes: Vec<String>,
    pub structures: Vec<String>,
    pub random_pool: bool,
    pub teams_min: Option<usize>,
    pub teams_exactly: Option<usize>,
}

impl Requirements {
    pub fn teams_ok(&self, teams: usize) -> bool {
        self.teams_min.is_none_or(|m| teams >= m) && self.teams_exactly.is_none_or(|n| teams == n)
    }
}

#[derive(Debug, Clone)]
pub struct GoalDef {
    pub id: String,
    pub kind: String,
    name: String,
    pub icon: String,
    pub items: Vec<String>,
    pub entities: Vec<String>,
    pub amount: Option<u32>,
    /// `dye` or `leather_dye` when the board picks a colour for it.
    pub data: Option<String>,
    pub requirements: Requirements,
    raw: Value,
}

impl GoalDef {
    /// The goal's name for `data`, as the Java goal names it.
    pub fn name(&self, data: &str) -> String {
        let color = dye_of(data).map(color_title).unwrap_or_default();
        let amount = self.amount.map(|a| a.to_string()).unwrap_or_default();
        if self.id == "WEAR_COLORED_LEATHER_ARMOR_PIECE" {
            return format!(
                "Wear {color} {}",
                piece_of(data)
                    .map(color_title)
                    .unwrap_or_else(|| "Leather Armor".into())
            );
        }
        self.name.replace("{color}", &color).replace("{amount}", &amount)
    }

    /// The item the board draws for this goal (the Java client draws custom textures
    /// for some; this is the item each would fall back to).
    pub fn icon_for(&self, data: &str) -> String {
        if let Some(dye) = dye_of(data) {
            match self.id.as_str() {
                "KILL_COLORED_SHEEP" => return format!("minecraft:{dye}_wool"),
                "OBTAIN_COLORED_GLAZED_TERRACOTTA" => return format!("minecraft:{dye}_glazed_terracotta"),
                "OBTAIN_64_COLORED_WOOL" => return format!("minecraft:{dye}_wool"),
                "OBTAIN_64_COLORED_CONCRETE" => return format!("minecraft:{dye}_concrete"),
                "OBTAIN_COLORED_HARNESS" => return format!("minecraft:{dye}_harness"),
                "WEAR_COLORED_LEATHER_ARMOR_PIECE" => {
                    return format!("minecraft:{}", piece_of(data).unwrap_or("leather_chestplate"));
                }
                _ => {}
            }
        }
        self.icon.clone()
    }

    /// The count drawn over the icon ("Kill 7 Unique Hostile Mobs" shows a 7).
    pub fn icon_count(&self) -> Option<u32> {
        let counted = [
            "UniqueTrackedGoal",
            "EatUniqueFoodsGoal",
            "BreedUniqueAnimalsGoal",
            "KillUniqueHostileMobsGoal",
            "GetUniqueAdvancementsGoal",
            "VisitUniqueBiomesGoal",
            "CraftUniqueItemsGoal",
            "CompostUniqueFoodsGoal",
            "LeashUniqueMobsGoal",
            "SpyUniqueMobsGoal",
            "ReachXPLevelGoal",
            "KillSpecificMobsGoal",
            "ObtainSomeOfTheItemsGoal",
        ];
        if counted.contains(&self.kind.as_str()) || self.id.starts_with("GET_") && self.id.ends_with("_AT_ONCE") {
            self.amount.filter(|a| *a > 1)
        } else {
            None
        }
    }

    fn list(&self, key: &str) -> Vec<String> {
        self.raw.get(key).and_then(Value::as_array).map_or_else(Vec::new, |a| {
            a.iter().filter_map(Value::as_str).map(str::to_owned).collect()
        })
    }

    /// Data of the shape this goal takes, for asking what it needs before a board picks one.
    pub fn sample_data(&self) -> &'static str {
        match self.data.as_deref() {
            Some("leather_dye") => "leather_boots&red",
            Some(_) => "red",
            None => DATA_NONE,
        }
    }

    /// What completes this goal with board data `data`.
    pub fn trigger(&self, data: &str) -> Trigger {
        let amount = self.amount.unwrap_or(1);
        let dye = dye_of(data);
        let colored = |base: &str| dye.map(|d| vec![format!("minecraft:{d}_{base}")]).unwrap_or_default();
        // Goals whose Java class has its own logic.
        let special = match self.id.as_str() {
            "OBTAIN_COLORED_GLAZED_TERRACOTTA" => Some(Trigger::ObtainAll {
                items: colored("glazed_terracotta"),
                amount: 1,
            }),
            "OBTAIN_64_COLORED_WOOL" => Some(Trigger::ObtainAll {
                items: colored("wool"),
                amount: 64,
            }),
            "OBTAIN_64_COLORED_CONCRETE" => Some(Trigger::ObtainAll {
                items: colored("concrete"),
                amount: 64,
            }),
            "OBTAIN_COLORED_HARNESS" => Some(Trigger::ObtainAll {
                items: colored("harness"),
                amount: 1,
            }),
            "KILL_COLORED_SHEEP" => Some(Trigger::Kill {
                entities: vec!["minecraft:sheep".into()],
                color: dye.map(str::to_owned),
                dimension: None,
            }),
            "OBTAIN_32_ARROWS" => Some(Trigger::ObtainAny {
                items: self.items.clone(),
                amount,
            }),
            "KILL_SNOW_GOLEM_IN_NETHER" => Some(Trigger::Kill {
                entities: vec!["minecraft:snow_golem".into()],
                color: None,
                dimension: Some("minecraft:the_nether".into()),
            }),
            "WEAR_COLORED_LEATHER_ARMOR_PIECE" => match (piece_of(data), dye.and_then(dye_rgb)) {
                (Some(piece), Some(rgb)) => Some(Trigger::WearDyed {
                    item: format!("minecraft:{piece}"),
                    rgb,
                }),
                _ => Some(Trigger::Unsupported),
            },
            "WEAR_UNIQUE_COLORED_LEATHER_ARMOR" => Some(Trigger::WearUniqueDyedLeather),
            "OBTAIN_STACK_OF_64" => Some(Trigger::StackOf64),
            "FILL_INVENTORY_UNIQUE_ITEMS" => Some(Trigger::FillInventoryUnique),
            "WEAR_UNIQUE_ARMOR" => Some(Trigger::WearUniqueMaterials),
            "BREW_LINGERING_POTION" => Some(Trigger::ObtainAll {
                items: vec!["minecraft:lingering_potion".into()],
                amount: 1,
            }),
            "TOOT_GOAT_HORN" => Some(Trigger::UseItem {
                item: "minecraft:goat_horn".into(),
            }),
            "REMOVE_STATUS_EFFECT_USING_MILK" => Some(Trigger::MilkAwayEffect),
            "GET_3_STATUS_EFFECTS_AT_ONCE" | "GET_4_STATUS_EFFECTS_AT_ONCE" | "GET_6_STATUS_EFFECTS_AT_ONCE" => {
                Some(Trigger::EffectsAtOnce {
                    amount: amount as usize,
                })
            }
            "USE_STONECUTTER" => Some(Trigger::Stonecutter),
            "USE_LOOM" => Some(Trigger::Loom),
            "USE_ANVIL" => Some(Trigger::Anvil),
            "EMPTY_HUNGER_BAR" => Some(Trigger::EmptyHunger),
            "REACH_HEIGHT_LIMIT" => Some(Trigger::Height {
                dimension: "minecraft:overworld".into(),
                min_y: 320.0,
            }),
            "REACH_NETHER_ROOF" => Some(Trigger::Height {
                dimension: "minecraft:the_nether".into(),
                min_y: 128.0,
            }),
            "REACH_BEDROCK" => Some(Trigger::StandOn {
                dimension: "minecraft:overworld".into(),
                block: "minecraft:bedrock".into(),
            }),
            "ENRAGE_ZOMBIFIED_PIGLIN" => Some(Trigger::Enrage {
                entity: "minecraft:zombified_piglin".into(),
            }),
            "ENRAGE_ENDERMAN" => Some(Trigger::Enrage {
                entity: "minecraft:enderman".into(),
            }),
            "KILL_OTHER_PLAYER" => Some(Trigger::KillOtherPlayer),
            "KILL_100_MOBS" => Some(Trigger::KillCount { entities: None, amount }),
            "TAKE_200_DAMAGE" => Some(Trigger::TakeDamage { amount: 200.0 }),
            "DEAL_400_DAMAGE" => Some(Trigger::DealDamage { amount: 400.0 }),
            "SPRINT_1_KM" => Some(Trigger::SprintCm { cm: 100_000 }),
            "HAVE_MORE_XP_LEVELS" => Some(Trigger::MostXpLevels),
            "HAVE_MORE_UNIQUE_CRAFTS" => Some(Trigger::MostUniqueCrafts),
            "HAVE_YOUR_SHIELD_DISABLED" => Some(Trigger::ShieldDisabled),
            "ITEM_FRAME_IN_ITEM_FRAME" => Some(Trigger::ItemFrameInItemFrame),
            "PUT_WOLF_ARMOR_ON_WOLF" => Some(Trigger::WolfArmor),
            "DUPLICATE_ARMOR_TRIM" => Some(Trigger::CraftItemSuffix {
                suffix: "_armor_trim_smithing_template".into(),
            }),
            "MINE_3_TYPES_OF_ICE" => Some(Trigger::MineUnique {
                blocks: self.items.clone(),
                amount: amount as usize,
            }),
            "EAT_ALL_SOUPS" => Some(Trigger::EatAll {
                items: self.items.clone(),
            }),
            "VISIT_ALL_CAVE_BIOMES" => Some(Trigger::BiomesAll {
                biomes: self.list("named"),
            }),
            "SHOOT_OPPONENT_WITH_ARROW" => Some(Trigger::ShootOpponentWithArrow),
            "PICK_UP_OPPONENT_ITEM" => Some(Trigger::PickUpOpponentItem),
            "DIE_5_UNIQUE_WAYS" => Some(Trigger::DieUniqueWays {
                amount: amount as usize,
            }),
            "DIE_BY_FALLING_OFF_VINE" => Some(Trigger::DieByMessage {
                key: "death.fell.accident.vines".into(),
            }),
            "BREAK_ANY_ARMOR" => Some(Trigger::BreakItem {
                matcher: ItemMatch::Armor,
            }),
            "BREAK_ANY_TOOL" => Some(Trigger::BreakItem {
                matcher: ItemMatch::Tool,
            }),
            "PLACE_HANGING_SIGN" => Some(Trigger::PlaceBlockSuffix {
                suffix: "_hanging_sign".into(),
            }),
            "PLACE_PAINTING" => Some(Trigger::PlaceHanging {
                kind: "minecraft:painting".into(),
            }),
            "OPPONENT_DIES" => Some(Trigger::Opponent(OpponentEvent::Dies { times: 1 })),
            "OPPONENT_DIES_3_TIMES" => Some(Trigger::Opponent(OpponentEvent::Dies { times: 3 })),
            "OPPONENT_CATCHES_ON_FIRE" => Some(Trigger::Opponent(OpponentEvent::CatchesFire)),
            "OPPONENT_HIT_BY_EGG" => Some(Trigger::Opponent(OpponentEvent::HitBy("minecraft:egg".into()))),
            "OPPONENT_HIT_BY_SNOWBALL" => Some(Trigger::Opponent(OpponentEvent::HitBy("minecraft:snowball".into()))),
            "OPPONENT_TAKES_100_DAMAGE" => Some(Trigger::Opponent(OpponentEvent::TakesDamage(100.0))),
            "OPPONENT_TAKES_FALL_DAMAGE" => Some(Trigger::Opponent(OpponentEvent::FallDamage)),
            "OPPONENT_TOUCHES_WATER" => Some(Trigger::Opponent(OpponentEvent::TouchesWater)),
            "OPPONENT_EATS_FOOD" => Some(Trigger::Opponent(OpponentEvent::EatsFood)),
            _ => None,
        };
        if let Some(t) = special {
            return t;
        }
        let entities = self.entities.clone();
        match self.kind.as_str() {
            "ObtainAllItemsGoal" if !self.items.is_empty() => Trigger::ObtainAll {
                items: self.items.clone(),
                amount,
            },
            "ObtainSomeOfTheItemsGoal" => Trigger::ObtainSome {
                items: self.items.clone(),
                amount,
            },
            "ObtainPotionItemGoal" => Trigger::ObtainPotion {
                potions: self.list("potions"),
            },
            "WearArmorGoal" => Trigger::WearAll {
                items: self.items.clone(),
            },
            "WearArmorPieceGoal" => Trigger::WearAny {
                items: self.items.clone(),
            },
            "AdvancementGoal" => Trigger::Advancement {
                ids: self.list("advancements"),
            },
            "GetUniqueAdvancementsGoal" => Trigger::UniqueAdvancements {
                amount: amount as usize,
            },
            "KillMobGoal" | "KillSnowGolemGoal" if !entities.is_empty() => Trigger::Kill {
                entities,
                color: None,
                dimension: None,
            },
            "KillSpecificMobsGoal" => Trigger::KillCount {
                entities: Some(entities),
                amount,
            },
            "KillAllSpecificMobsGoal" => Trigger::KillAll { entities },
            "KillUniqueHostileMobsGoal" => Trigger::KillUniqueHostile {
                amount: amount as usize,
            },
            "DieToDamageTypeGoal" => Trigger::DieTo {
                damage_types: self.list("damage_types"),
            },
            "DieToEntityGoal" => Trigger::DieToEntity { entities },
            "BreedAnimalGoal" => Trigger::Breed { entities },
            "BreedUniqueAnimalsGoal" => Trigger::BreedUnique {
                amount: amount as usize,
            },
            "TameAnimalGoal" => Trigger::Tame { entities },
            "StatusEffectGoal" => Trigger::Effect {
                effects: self.list("effects"),
            },
            "ConsumeItemGoal" => Trigger::Consume {
                items: self.items.clone(),
            },
            "DrinkPotionGoal" => Trigger::DrinkPotion {
                potions: self.list("potions"),
            },
            "EatUniqueFoodsGoal" => Trigger::EatUnique {
                amount: amount as usize,
            },
            "IncrementStatGoal" => Trigger::Stat {
                stats: self.list("stats"),
            },
            "MineBlockGoal" => Trigger::Mine {
                blocks: self.items.clone(),
            },
            "EnterDimensionGoal" => self
                .raw
                .get("dimension")
                .and_then(Value::as_str)
                .map_or(Trigger::Unsupported, |d| Trigger::Dimension { dimension: d.into() }),
            "VisitBiomeGoal" => Trigger::Biome {
                biomes: self.list("biomes"),
            },
            "VisitUniqueBiomesGoal" => Trigger::UniqueBiomes {
                amount: amount as usize,
            },
            "ReachXPLevelGoal" => Trigger::XpLevel { level: amount as i32 },
            "CraftUniqueItemsGoal" => Trigger::CraftUnique {
                amount: amount as usize,
            },
            "RideEntityGoal" => Trigger::Ride { entities },
            "NameEntityGoal" => match (entities.first(), quoted_name(&self.name)) {
                (Some(e), Some(n)) => Trigger::Name {
                    entity: e.clone(),
                    name: n,
                },
                _ => Trigger::Unsupported,
            },
            "BreakItemGoal" => Trigger::BreakItem {
                matcher: ItemMatch::Any(self.items.clone()),
            },
            "OpponentObtainsItemGoal" => self.items.first().map_or(Trigger::Unsupported, |i| {
                Trigger::Opponent(OpponentEvent::Obtains(i.clone()))
            }),
            _ => Trigger::Unsupported,
        }
    }
}

/// `Dinnerbone` from `Name a Ghast 'Dinnerbone'`.
fn quoted_name(name: &str) -> Option<String> {
    let start = name.find('\'')? + 1;
    let end = start + name[start..].find('\'')?;
    Some(name[start..end].to_owned())
}

/// A dye's colour on leather (`DyeColor.getTextureDiffuseColor`, the Java mod's
/// `GoalDataConstants.getDyeColorValue`).
pub fn dye_rgb(dye: &str) -> Option<i32> {
    const RGB: [i32; 16] = [
        0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8,
        0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
    ];
    DYES.iter().position(|d| *d == dye).map(|i| RGB[i])
}

/// Undyed leather (`DyedItemColor.LEATHER_COLOR`).
pub const LEATHER_RGB: i32 = 0xA06540;

/// The dye colour in a goal's data (`red`, or `leather_helmet&red`).
pub fn dye_of(data: &str) -> Option<&str> {
    data.split('&').find(|p| DYES.contains(p))
}

fn piece_of(data: &str) -> Option<&str> {
    data.split('&').find(|p| p.starts_with("leather_"))
}

/// `light_blue` → `Light Blue`.
fn color_title(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct Table {
    pub goals: Vec<GoalDef>,
    by_id: HashMap<String, usize>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub goals: Vec<String>,
    pub limit: usize,
    pub require_predecessor: Vec<String>,
}

impl Table {
    pub fn get(&self, id: &str) -> Option<&GoalDef> {
        self.by_id.get(id).map(|&i| &self.goals[i])
    }
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map_or_else(Vec::new, |a| {
        a.iter().filter_map(Value::as_str).map(str::to_owned).collect()
    })
}

fn parse() -> Table {
    let root: Value = serde_json::from_str(GOALS_JSON).expect("goals.json is valid");
    let mut goals = Vec::new();
    for g in root["goals"].as_array().expect("goals") {
        let items = strings(g, "items");
        let entities = strings(g, "entities");
        let req = g.get("requirements");
        let requirements = Requirements {
            biomes: req.map(|r| strings(r, "biomes")).unwrap_or_default(),
            structures: req.map(|r| strings(r, "structures")).unwrap_or_default(),
            random_pool: req
                .and_then(|r| r.get("random_pool"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            teams_min: req
                .and_then(|r| r.get("teams_min"))
                .and_then(Value::as_u64)
                .map(|v| v as usize),
            teams_exactly: req
                .and_then(|r| r.get("teams_exactly"))
                .and_then(Value::as_u64)
                .map(|v| v as usize),
        };
        let kind = g["kind"].as_str().unwrap_or("Goal").to_owned();
        let icon = g
            .get("icon")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| items.first().cloned())
            .or_else(|| entities.first().map(|e| format!("{e}_spawn_egg")))
            .unwrap_or_else(|| default_icon(&kind).to_owned());
        let id = g["id"].as_str().expect("id").to_owned();
        goals.push(GoalDef {
            name: g
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| id.replace('_', " ")),
            id,
            kind,
            icon,
            items,
            entities,
            amount: g.get("amount").and_then(Value::as_u64).map(|v| v as u32),
            data: g.get("data").and_then(Value::as_str).map(str::to_owned),
            requirements,
            raw: g.clone(),
        });
    }
    let by_id = goals.iter().enumerate().map(|(i, g)| (g.id.clone(), i)).collect();
    let groups = root["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .map(|g| Group {
            goals: strings(g, "goals"),
            limit: g["limit"].as_u64().unwrap_or(1) as usize,
            require_predecessor: strings(g, "require_predecessor"),
        })
        .collect();
    Table { goals, by_id, groups }
}

fn default_icon(kind: &str) -> &'static str {
    match kind {
        "ObtainPotionItemGoal" | "DrinkPotionGoal" => "minecraft:potion",
        "DieToDamageTypeGoal" | "DieToEntityGoal" => "minecraft:skeleton_skull",
        "StatusEffectGoal" => "minecraft:glass_bottle",
        "EnterDimensionGoal" => "minecraft:obsidian",
        "ReachXPLevelGoal" => "minecraft:experience_bottle",
        "OpponentObtainsItemGoal" => "minecraft:barrier",
        _ => "minecraft:paper",
    }
}

pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_goal_has_a_name_and_an_icon() {
        let t = table();
        assert_eq!(t.goals.len(), 338);
        for g in &t.goals {
            let data = if g.data.is_some() { "lime" } else { DATA_NONE };
            assert!(!g.name(data).contains('{'), "{}", g.id);
            assert!(g.icon_for(data).starts_with("minecraft:"), "{}", g.id);
        }
    }

    #[test]
    fn names_and_triggers_follow_the_java_goals() {
        let t = table();
        assert_eq!(
            t.get("KILL_COLORED_SHEEP").unwrap().name("light_blue"),
            "Kill Light Blue Sheep"
        );
        assert_eq!(
            t.get("BREED_6_UNIQUE_ANIMALS").unwrap().name(DATA_NONE),
            "Breed 6 Unique Animals"
        );
        assert_eq!(
            t.get("OBTAIN_64_COLORED_WOOL").unwrap().trigger("red"),
            Trigger::ObtainAll {
                items: vec!["minecraft:red_wool".into()],
                amount: 64
            }
        );
        assert_eq!(
            t.get("ENTER_NETHER").unwrap().trigger(DATA_NONE),
            Trigger::Dimension {
                dimension: "minecraft:the_nether".into()
            }
        );
        assert_eq!(
            t.get("NAME_SHEEP_JEB").unwrap().trigger(DATA_NONE),
            Trigger::Name {
                entity: "minecraft:sheep".into(),
                name: "jeb_".into()
            }
        );
        assert!(matches!(
            t.get("CURE_ZOMBIE_VILLAGER").unwrap().trigger(DATA_NONE),
            Trigger::Advancement { .. }
        ));
    }

    #[test]
    fn most_goals_are_supported() {
        let t = table();
        let supported = t
            .goals
            .iter()
            .filter(|g| g.trigger(g.sample_data()) != Trigger::Unsupported)
            .count();
        assert_eq!(supported, 299, "of {}", t.goals.len());
    }
}
