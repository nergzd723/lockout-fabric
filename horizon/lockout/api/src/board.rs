//! Random boards, by the Java mod's rules (`BoardGenerator`, `GoalGroup`, `GoalRequirements`).
use crate::goals::{self, DATA_NONE, DYES, GoalDef, Table, Trigger};
use std::collections::HashSet;

/// A small seeded generator (SplitMix64): boards need nothing better, and it keeps tests
/// reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

/// Whether `goal` may join `board` under every group it is in (`GoalGroup.canAdd`).
pub fn can_add(table: &Table, goal: &str, board: &[String]) -> bool {
    for group in table.groups.iter().filter(|g| g.goals.iter().any(|x| x == goal)) {
        let count = board.iter().filter(|b| group.goals.contains(b)).count();
        if count + 1 > group.limit {
            return false;
        }
        if group.require_predecessor.iter().any(|x| x == goal) {
            let index = group.goals.iter().position(|x| x == goal).unwrap_or(0);
            if !group.goals[..index].iter().any(|p| board.contains(p)) {
                return false;
            }
        }
    }
    true
}

/// What the server found near spawn before the match, which decides the goals a random
/// board may hold and the colours it may ask for.
#[derive(Debug, Clone, Default)]
pub struct World {
    pub biomes: HashSet<String>,
    pub structures: HashSet<String>,
}

impl World {
    /// A world with every biome and structure near spawn (tests, and boards before locating).
    pub fn everything(table: &Table) -> Self {
        Self {
            biomes: goals::biomes_to_locate(table).into_iter().collect(),
            structures: goals::structures_to_locate(table).into_iter().collect(),
        }
    }
    pub fn dyes(&self) -> Vec<&'static str> {
        goals::available_dyes(|b| self.biomes.contains(b))
    }
    pub fn satisfies(&self, goal: &GoalDef) -> bool {
        goal.requirements
            .satisfied(|b| self.biomes.contains(b), |s| self.structures.contains(s))
    }
}

/// Data for a goal that picks a colour (`GoalDataGenerator`): one of the dyes the world
/// allows, less the ones the goal leaves out.
pub fn data_for(goal: &GoalDef, world: &World, rng: &mut Rng) -> String {
    let excluded: &[&str] = match goal.id.as_str() {
        "KILL_COLORED_SHEEP" => &["white", "gray", "black", "light_gray", "pink", "brown"],
        "WEAR_COLORED_LEATHER_ARMOR_PIECE" => &["white", "gray", "black", "pink", "light_gray"],
        _ => &[],
    };
    let mut dyes: Vec<&str> = world.dyes().into_iter().filter(|d| !excluded.contains(d)).collect();
    if dyes.is_empty() {
        dyes = DYES.to_vec();
    }
    let dye = dyes[rng.below(dyes.len())];
    match goal.data.as_deref() {
        Some("leather_dye") => {
            let pieces = [
                "leather_helmet",
                "leather_chestplate",
                "leather_leggings",
                "leather_boots",
            ];
            format!("{}&{dye}", pieces[rng.below(pieces.len())])
        }
        Some(_) => dye.to_owned(),
        None => DATA_NONE.to_owned(),
    }
}

/// Whether a random board or a draft may offer `goal` for `teams` teams
/// (`GoalRequirements`: team count, random pool, located biomes and structures). A goal no
/// Horizon hook can track is left out too.
pub fn eligible(goal: &GoalDef, teams: usize, world: &World) -> bool {
    let req = &goal.requirements;
    req.random_pool
        && req.teams_ok(teams)
        && world.satisfies(goal)
        && goal.trigger(goal.sample_data()) != Trigger::Unsupported
}

/// A `size`×`size` board of (goal id, data), or None when too few goals qualify.
pub fn generate(
    table: &Table,
    size: usize,
    teams: usize,
    world: &World,
    rng: &mut Rng,
) -> Option<Vec<(String, String)>> {
    let pool: Vec<&GoalDef> = table.goals.iter().filter(|g| eligible(g, teams, world)).collect();
    for _ in 0..50 {
        let mut order = pool.clone();
        rng.shuffle(&mut order);
        let mut ids: Vec<String> = Vec::new();
        let mut board = Vec::new();
        for goal in order {
            if board.len() == size * size {
                break;
            }
            if !can_add(table, &goal.id, &ids) {
                continue;
            }
            board.push((goal.id.clone(), data_for(goal, world, rng)));
            ids.push(goal.id.clone());
        }
        if board.len() == size * size {
            // Shuffled again: a goal that needs a predecessor always comes after it.
            rng.shuffle(&mut board);
            return Some(board);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goals::table;

    #[test]
    fn boards_respect_the_groups() {
        let t = table();
        let all = World::everything(t);
        let none = World::default();
        for seed in 0..40 {
            let mut rng = Rng::new(seed);
            let world = if seed % 2 == 0 { &all } else { &none };
            let board = generate(t, 7, 2, world, &mut rng).expect("a board");
            assert_eq!(board.len(), 49);
            let ids: Vec<String> = board.iter().map(|(id, _)| id.clone()).collect();
            for group in &t.groups {
                let n = ids.iter().filter(|i| group.goals.contains(i)).count();
                assert!(n <= group.limit, "group {:?} has {n}", group.goals);
            }
            let mut unique = ids.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), ids.len());
            if world.biomes.is_empty() {
                for id in &ids {
                    let r = &t.get(id).unwrap().requirements;
                    assert!(
                        r.biomes.is_empty() && r.structures.is_empty() && r.biome_groups.is_empty(),
                        "{id}"
                    );
                }
            }
        }
    }

    #[test]
    fn coloured_goals_get_a_colour_the_world_has() {
        let t = table();
        let mut rng = Rng::new(3);
        let sheep = t.get("KILL_COLORED_SHEEP").unwrap();
        let all = World::everything(t);
        for _ in 0..20 {
            let d = data_for(sheep, &all, &mut rng);
            assert!(DYES.contains(&d.as_str()) && d != "white" && d != "brown");
        }
        // No desert, badlands, warm ocean or jungle: no cactus, sea pickle or cocoa dyes.
        let bare = World::default();
        for _ in 0..50 {
            let d = data_for(t.get("OBTAIN_64_COLORED_WOOL").unwrap(), &bare, &mut rng);
            assert!(!["green", "lime", "cyan", "brown"].contains(&d.as_str()), "{d}");
        }
        assert!(data_for(t.get("WEAR_COLORED_LEATHER_ARMOR_PIECE").unwrap(), &all, &mut rng).starts_with("leather_"));
    }

    #[test]
    fn requirements_follow_goal_requirements() {
        let t = table();
        let mut w = World::default();
        let caves = t.get("VISIT_ALL_CAVE_BIOMES").unwrap();
        w.biomes.insert("minecraft:lush_caves".into());
        assert!(!w.satisfies(caves), "all three cave biomes are needed");
        w.biomes.insert("minecraft:dripstone_caves".into());
        w.biomes.insert("minecraft:deep_dark".into());
        assert!(w.satisfies(caves));
        let eggs = t.get("OBTAIN_ALL_EGGS").unwrap();
        w.biomes.insert("minecraft:desert".into());
        assert!(!w.satisfies(eggs), "a cold biome is needed too");
        w.biomes.insert("minecraft:taiga".into());
        assert!(w.satisfies(eggs));
        let witch = t.get("KILL_WITCH").unwrap();
        assert!(!w.satisfies(witch));
        w.structures.insert("minecraft:swamp_hut".into());
        assert!(w.satisfies(witch));
    }

    #[test]
    fn one_v_one_goals_need_two_teams() {
        let t = table();
        let all = World::everything(t);
        let g = t.get("OPPONENT_OBTAINS_OBSIDIAN").unwrap();
        // Not in the random pool at all, as in the Java mod.
        assert!(!eligible(g, 2, &all));
        let pvp = t.goals.iter().find(|g| g.requirements.teams_min == Some(2));
        if let Some(g) = pvp {
            assert!(!eligible(g, 1, &all));
        }
    }
}
