/// The lines the Java mod's debug screen keeps, in its order, from Horizon's (which writes
/// them as vanilla does); a blank line after the render stats.
fn debug_screen(columns: (Vec<String>, Vec<String>), open: bool) -> (Vec<String>, Vec<String>) {
    if !open {
        return (Vec::new(), Vec::new());
    }
    let lines: Vec<&String> = columns.0.iter().chain(columns.1.iter()).collect();
    let first = |test: &dyn Fn(&str) -> bool| lines.iter().find(|l| test(l.as_str())).map(|l| (*l).clone());
    let mut left = Vec::new();
    let head = [
        first(&|l| l.starts_with("Minecraft ")),
        first(&|l| l.contains(" fps")),
        first(&|l| l.starts_with("C: ")),
        first(&|l| l.starts_with("E: ")),
    ];
    left.extend(head.into_iter().flatten());
    left.push(String::new());
    for prefix in ["XYZ: ", "Block: ", "Chunk: ", "Facing: "] {
        left.extend(first(&|l| l.starts_with(prefix)));
    }
    match first(&|l| l.starts_with("SH ")) {
        Some(sh) => left.push(sh),
        None => left.extend(first(&|l| l == "Waiting for chunk...")),
    }
    left.extend(first(&|l| l.starts_with("Biome: ")));
    (left, Vec::new())
}
