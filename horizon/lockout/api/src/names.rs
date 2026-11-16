//! English names, as the Java mod's tooltips print them (`Item.getName`,
//! `EntityType.getDescription`, `Block.getName`, an advancement's display title). They come
//! from the 26.3 client's `en_us.json` (`tools/extract_names.py`); an id the table doesn't
//! know (a newer game's, a datapack's) is shown as its id, as Java shows a missing key.
use horizon_client::mod_api::serde_json::{self, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

const NAMES_JSON: &str = include_str!("../data/names.json");

struct Names {
    items: HashMap<String, String>,
    blocks: HashMap<String, String>,
    entities: HashMap<String, String>,
    advancements: HashMap<String, String>,
}

fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| {
        let v: Value = serde_json::from_str(NAMES_JSON).unwrap_or(Value::Null);
        let map = |key: &str| -> HashMap<String, String> {
            v.get(key)
                .and_then(Value::as_object)
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                        .collect()
                })
                .unwrap_or_default()
        };
        Names {
            items: map("items"),
            blocks: map("blocks"),
            entities: map("entities"),
            advancements: map("advancements"),
        }
    })
}

fn translation_key(kind: &str, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("{kind}.{ns}.{}", path.replace('/', "."))
}

pub fn item(id: &str) -> String {
    names()
        .items
        .get(id)
        .cloned()
        .unwrap_or_else(|| translation_key("item", id))
}

pub fn block(id: &str) -> String {
    names()
        .blocks
        .get(id)
        .cloned()
        .unwrap_or_else(|| translation_key("block", id))
}

pub fn entity(id: &str) -> String {
    names()
        .entities
        .get(id)
        .cloned()
        .unwrap_or_else(|| translation_key("entity", id))
}

/// A vanilla advancement's title; None for one without a display (recipes, roots of
/// hidden trees) or one the table doesn't know.
pub fn advancement(id: &str) -> Option<String> {
    names().advancements.get(id).cloned()
}

/// `WordUtils.capitalize(id.getPath().replace('_', ' '))`: how biomes and damage types are
/// shown.
pub fn capitalized_path(id: &str) -> String {
    let path = id.split_once(':').map_or(id, |(_, p)| p).replace('_', " ");
    let mut out = String::with_capacity(path.len());
    let mut start = true;
    for c in path.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        start = c.is_whitespace();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_the_games() {
        assert_eq!(item("minecraft:beef"), "Raw Beef");
        assert_eq!(item("minecraft:oak_planks"), "Oak Planks");
        assert_eq!(block("minecraft:packed_ice"), "Packed Ice");
        assert_eq!(entity("minecraft:zombified_piglin"), "Zombified Piglin");
        assert_eq!(advancement("minecraft:story/mine_stone").as_deref(), Some("Stone Age"));
        assert_eq!(advancement("minecraft:recipes/misc/bread"), None);
        assert_eq!(capitalized_path("minecraft:dripstone_caves"), "Dripstone Caves");
        assert_eq!(item("mymod:gadget"), "item.mymod.gadget");
    }
}
