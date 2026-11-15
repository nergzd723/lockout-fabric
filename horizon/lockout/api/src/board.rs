//! Random boards, by the Java mod's rules (`BoardGenerator`, `GoalGroup`, `GoalRequirements`).
use crate::goals::{DATA_NONE, DYES, GoalDef, Table, Trigger};

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

/// Data for a goal that picks a colour (`GoalDataGenerator`): the Java mod leaves out the
/// colours hard to get.
fn data_for(goal: &GoalDef, rng: &mut Rng) -> String {
    let excluded: &[&str] = match goal.id.as_str() {
        "KILL_COLORED_SHEEP" => &["white", "gray", "black", "light_gray", "pink", "brown"],
        "WEAR_COLORED_LEATHER_ARMOR_PIECE" => &["white", "gray", "black", "pink", "light_gray"],
        _ => &[],
    };
    let dyes: Vec<&str> = DYES.iter().copied().filter(|d| !excluded.contains(d)).collect();
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

/// Whether a random board may hold `goal` for `teams` teams. `with_located` also lets in the
/// goals that need a biome or structure nearby: Horizon gives mods no way to locate them, so
/// the server owner chooses (`/lockout located on|off`).
pub fn eligible(goal: &GoalDef, teams: usize, with_located: bool) -> bool {
    let req = &goal.requirements;
    req.random_pool
        && req.teams_ok(teams)
        && (with_located || req.biomes.is_empty() && req.structures.is_empty())
        && goal.trigger(goal.sample_data()) != Trigger::Unsupported
}

/// A `size`×`size` board of (goal id, data), or None when too few goals qualify.
pub fn generate(
    table: &Table,
    size: usize,
    teams: usize,
    with_located: bool,
    rng: &mut Rng,
) -> Option<Vec<(String, String)>> {
    let pool: Vec<&GoalDef> = table
        .goals
        .iter()
        .filter(|g| eligible(g, teams, with_located))
        .collect();
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
            board.push((goal.id.clone(), data_for(goal, rng)));
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
        for seed in 0..40 {
            let mut rng = Rng::new(seed);
            let board = generate(t, 7, 2, seed % 2 == 0, &mut rng).expect("a board");
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
        }
    }

    #[test]
    fn coloured_goals_get_a_colour() {
        let t = table();
        let mut rng = Rng::new(3);
        let sheep = t.get("KILL_COLORED_SHEEP").unwrap();
        for _ in 0..20 {
            let d = data_for(sheep, &mut rng);
            assert!(DYES.contains(&d.as_str()) && d != "white" && d != "brown");
        }
        assert!(data_for(t.get("WEAR_COLORED_LEATHER_ARMOR_PIECE").unwrap(), &mut rng).starts_with("leather_"));
    }

    #[test]
    fn one_v_one_goals_need_two_teams() {
        let t = table();
        let g = t.get("OPPONENT_OBTAINS_OBSIDIAN").unwrap();
        // Not in the random pool at all, as in the Java mod.
        assert!(!eligible(g, 2, true));
        let pvp = t.goals.iter().find(|g| g.requirements.teams_min == Some(2));
        if let Some(g) = pvp {
            assert!(!eligible(g, 1, true));
        }
    }
}
