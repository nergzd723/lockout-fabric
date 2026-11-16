# Lockout for Horizon

A port of Lockout to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition
26.3. It is a Horizon **native trait mod**: Rust compiled into the game, using only Horizon's catalog hook points
and `Host` helpers ([Horizon's native mod docs](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md)),
plus a small second bundle, `lockout_tweaks`, for the vanilla changes Horizon has no hook point for.

It speaks the Fabric mod's network payloads byte for byte (`lockout:*` channels, version `0.10.4`), so the two mix:

| Server | Client | Works |
| --- | --- | --- |
| Horizon + this mod | Horizon + this mod | yes |
| Horizon + this mod | Fabric + Lockout 0.10.4 | yes: the Fabric client draws the board, tooltips and all |
| Fabric + Lockout 0.10.4 | Horizon + this mod | yes: the mod answers the version check, so you aren't kicked |
| Horizon + this mod | vanilla or Horizon without the mod | kicked after 5 seconds, as the Fabric server kicks a client without the mod |

A saved match (`lockout-match.json` in the world folder) has the Fabric mod's layout, progress included, so a world
keeps its match when it moves between a Fabric and a Horizon server.

## Layout

```
horizon/
  lockout/                  the bundle the launcher builds
    horizon-mixin.toml      [mod] id/version, [api] path
    api/Cargo.toml          the trait mod's crate (horizon-client and png)
    api/data/goals.json     the goal table, generated from the Java sources
    api/data/names.json     English item, block, mob and advancement names (26.3's en_us)
    api/data/atlas.*        the Fabric mod's textures, packed
    api/src/lib.rs          the Hooks: turns Horizon's events into match events
    api/src/rules.rs        what completes each goal, per Java handler and mixin
    api/src/server.rs       the match: teams, completing, winning, the tick
    api/src/goals.rs        goal names, icons and triggers
    api/src/board.rs        random boards (GoalGroup and GoalRequirements rules)
    api/src/draft.rs        drafts and scouting
    api/src/pause.rs        the pause after a restart
    api/src/persist.rs      saving and restoring the match
    api/src/compass.rs      the tracking compass
    api/src/client.rs       the board on screen, the client commands
    api/src/screens.rs      the board, draft and Board Builder screens
    api/src/wire.rs         the Fabric payloads
  .lockout_tweaks/          injections: raids, zombie villagers, the debug screen
  dev/lockout-run/          runs the mod on a Horizon checkout you build yourself
  dev/lockout-test/         builds and tests the mod on a Horizon checkout (CI too)
  tools/                    regenerate goals.json, names.json and the atlas; check_sync.py, pack_hnmod.py
```

Native bundles may only carry `.rs` and `.toml` files, `data/` folders and READMEs, which is why the tools live
outside `lockout/`.

## Install

With the Horizon launcher (0.2.69, the current release) and any Horizon installed in it:

```bash
horizon-launcher mods add-github https://github.com/nergzd723/lockout-fabric --trust-native-code
```

then build the instance's native game from the Mods page (or `horizon-launcher native-build --trust-native-code`). The
mod pins the Horizon commit it was made and tested on (`4626921895df51521be64d485ee763412de2917c`, hook API 645,
launcher 0.2.69's), so the launcher builds the game from that commit's source whatever version the instance plays.
The first build downloads Rust and compiles the game: 20 to 30 minutes.

Native mods run unsandboxed with your rights; read the code before you build it. The same build serves single
player, LAN hosting and a dedicated server (`--server-output` for a standalone server).

`lockout_tweaks` (raids, zombie villagers, the debug screen) is a bundle of its own in `horizon/.lockout_tweaks/`, so
that the link above finds Lockout alone. From a clone of this repository:

```bash
horizon-launcher native-build horizon/.lockout_tweaks --instance <instance> --trust-native-code
```

Horizon's `main` moved to a new mod SDK (`horizon-mod-api`) on 2026-11-16, after launcher 0.2.69: launchers built
from it refuse mods that depend on `horizon-client`, as this one does. Until the mod is ported to that SDK, stay on
launcher 0.2.69 (`HORIZON_LAUNCHER_NO_SELF_UPDATE=1` keeps it from updating itself).

### On your own Horizon checkout

The launcher builds mods from GitHub's copy of a Horizon commit, never from a local checkout. To play the mod on a
checkout you build yourself, do by hand what the launcher's generated game does: one package that links the mod and
starts the game with its hooks (its `main.rs` mirrors the one the launcher generates). The checkout must be at the
commit the mod pins (`main` from 2026-11-16 on has the new SDK, which this doesn't build on):

```bash
cd ~/horizon && git fetch origin && git checkout 4626921895df51521be64d485ee763412de2917c
```

`horizon/dev/lockout-run/` is that package. With the two clones side by side (`~/horizon` and `~/lockout-fabric`;
otherwise change `[lib] path` in its `Cargo.toml`), copy it into the checkout and add it to the workspace:

```bash
cp -r ~/lockout-fabric/horizon/dev/lockout-run ~/horizon/
```

```bash
cd ~/horizon && sed -i 's/^members = \[/members = [\n    "lockout-run",/' Cargo.toml
```

Then run it as you run `horizon-client`, from the checkout's root, where it finds the prepared resources:

```bash
cargo run --release -p lockout-run -- --render-distance 12
```

This is the client with the mod; its single player and LAN worlds run the match on its integrated server. It
doesn't include `lockout_tweaks`, which needs `horizon-mixins apply` over the checkout, as the launcher does.

## Commands

The Fabric mod's, with its permission levels and messages: `/lockout teams|players ...`, `/lockout draft ...`,
`/lockout cancel`, `/blackout team|players ...`, `/blackout draft ...`, `/blackout cancel`, `/chat team|local`,
`/GiveGoal`, `/SetStartTime`, `/SetBoardSize`, `/SetDraftSettings`, `/SetScoutingTime`, `/RemoveCustomBoard`, and on
the client `/BoardPosition left|right|hidden`, `/BoardBuilder` and `/SetCustomBoard`. Hold **B** for the board
(`key.lockout.open_board`).

## Same as the Fabric mod, and where it isn't

Everything the Fabric mod does was compared against its sources, goal by goal, and changes land on both sides
together. [PARITY.md](../PARITY.md) records it: the shared contract (payloads, version, goals, saves, config), the
features on each side, the differences Horizon's hook points force and why, the checks CI runs, and the log of
changes ported.

In short:

- All 338 goals, with the Java classes' conditions: kill credit lasts 100 ticks (a tamed wolf's kill counts for its
  owner), unique goals count what the Java mod counts (only foods for "unique food", honey included; only
  advancements with a display; biomes from Adventuring Time's criteria), riding a pig needs a carrot on a stick in
  hand, the carved pumpkin's five minutes are counted per player, "have the most" goals stay with their holder on a
  tie, and the 1v1 goals go to the first other team with the Java chat lines ("Alex touched water.").
- Who a completion is credited to (the player, or the team for team-progress goals), the chat lines and their
  colours, and every tooltip, word for word, with the Java mod's line wrapping.
- The match: boards (groups, requirements, locating biomes and structures with `/locate`), drafts and scouting, the
  countdown with players unhurtable until the start, the timer, the winner and tie rules, team chat, the tracking
  compass, saving every 30 seconds, the pause after a restart, the version check.
- The client: the board and its textures, the board, draft and Board Builder screens, the completion sounds, and
  the hidden toasts and debug screen.
- The vanilla changes: 1.16 bartering, Normal raids on every difficulty, zombies always converting villagers.

The mod has been type-checked and unit-tested against Horizon, not yet played in a running Horizon game.

## Keeping it in step with the Fabric mod

[PARITY.md](../PARITY.md), "Making a change", has the checklist. The tools:

- `python3 -I horizon/tools/check_sync.py` checks what CI checks: `goals.json` up to date, one version everywhere,
  the same Horizon pin in both bundles, and both pack.
- `python3 -I horizon/tools/extract_goals.py` regenerates `goals.json` from the Java goals.
- `python3 -I horizon/tools/extract_names.py <client.jar>` regenerates `names.json` from the game's client jar.
- `python3 -I horizon/tools/pack_hnmod.py horizon/lockout lockout-<version>.hnmod` packs the mod as the launcher
  does; the release workflow attaches it to every release, since `mods add-github` on the repository installs its
  latest release's `.hnmod` (a release without one falls back to its tag's source). Check one with Horizon's
  `cargo horizon-mod check <file>`.

## Tests

The crate's tests cover the payload encoding, the goal table, board generation, the rules (tooltips, messages,
team progress), saving, drafts and the hooks (through Horizon's own `dispatch_*` functions). They build against a
Horizon checkout at the pinned commit, through `horizon/dev/lockout-test` (the crate with `horizon-client` as a path
dependency). With the two clones side by side, as for `lockout-run`:

```bash
cp -r ~/lockout-fabric/horizon/dev/lockout-test ~/horizon/
```

```bash
cd ~/horizon && sed -i 's/^members = \[/members = [\n    "lockout-test",/' Cargo.toml
```

```bash
cargo test -p mod-lockout
```

CI runs them the same way on every push (`.github/workflows/check.yml`), against horizon-rust/horizon
`4626921895df51521be64d485ee763412de2917c`.
