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
  lockout_tweaks/           injections: raids, zombie villagers, the debug screen
  dev/lockout-run/          runs the mod on a Horizon checkout you build yourself
  tools/                    regenerate goals.json, names.json and the atlas
```

Native bundles may only carry `.rs` and `.toml` files, `data/` folders and READMEs, which is why the tools live
outside `lockout/`.

## Install

You need the Horizon launcher and a Horizon built from `main` at or after
`4626921895df51521be64d485ee763412de2917c` (native hook API 645). The 2026-11-13 Nightly (API 415) is too old: it
lacks hooks the mod uses. Neither bundle is pinned to a commit, so they build on the instance's own Horizon.

```bash
horizon-launcher native-build horizon/lockout --instance <instance> --trust-native-code
```

```bash
horizon-launcher native-build horizon/lockout_tweaks --instance <instance> --trust-native-code
```

Native mods run unsandboxed with your rights; read the code before you build it. The same build serves single
player, LAN hosting and a dedicated server (`--server-output` for a standalone server).

### On your own Horizon checkout

The launcher builds mods from GitHub's copy of a Horizon commit, never from a local checkout. To play the mod on a
checkout you build yourself, do by hand what the launcher's generated game does: one package that links the mod and
starts the game with its hooks (its `main.rs` mirrors the one the launcher generates). Check the checkout's hook API first (645 or later):

```bash
grep -n "const NATIVE_HOOKS_API" crates/horizon-client/src/game/native_hooks.rs
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
`/blackout team|players ...`, `/blackout draft ...`, `/chat team|local`, `/GiveGoal`, `/SetStartTime`,
`/SetBoardSize`, `/SetDraftSettings`, `/SetScoutingTime`, `/RemoveCustomBoard`, and on the client `/BoardPosition`,
`/BoardBuilder` and `/SetCustomBoard`. Hold **B** for the board (`key.lockout.open_board`).

## What's the same

Everything the Fabric mod does was compared against its sources, goal by goal:

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

## What isn't, and why

Horizon has no hook for some of the moments the Java mixins catch, so the port watches for their result instead.
The goals complete as on Fabric, but the timing or an edge case can differ:

- A station's output (stonecutter, loom, grindstone, smithing table, cartography table, a brewing stand's bottles)
  counts when the click on its result slot puts the item in the player's inventory or cursor, seen on the next
  tick; a result thrown with Q counts without that check. Whether a map was locked is read from the glass pane the
  player put in the table.
- Composting, filling a campfire, a chiseled bookshelf or a decorated pot, glow ink on a sign, opening an abandoned
  camp's loot, an Allay duplicating, wolf armour and a full armour stand are read from the block or mob the tick
  after the click.
- A snowball or egg counts when it hits the player (Java: when it touches them). A fall off vines is read from the
  death message. A Copper Golem counts for everyone within 5 blocks of a Copper Golem that didn't spawn
  naturally (spawn eggs too). A bundle of 16 bundles counts while it is held, not on the click that fills it.
- Damage goals count the hit as Horizon reports it, after the difficulty's scaling (Java counts the amount before
  it): the same on Normal.
- The spyglass's line of sight is traced through blocks by name (air, plants, torches... let it through) rather
  than by collision shape.
- A teleport through a portal with an ender pearl needs Horizon to report the pearl's cross-dimension teleport.
- `/chat team` with no team and no match sends a plain line rather than a normal chat message.
- The client commands answer in the action bar rather than in chat.

The mod has been type-checked and unit-tested against Horizon, not yet played in a running Horizon game.

## Keeping it in step with the Fabric mod

- After a goal changes on the Java side, run `python3 horizon/tools/extract_goals.py` and commit `goals.json`; new
  goal kinds need a `Trigger` in `goals.rs` and an arm in `rules.rs`.
- `names.json` comes from the game's client jar: `python3 -I horizon/tools/extract_names.py <client.jar>`.
- `wire::LOCKOUT_VERSION` must equal `mod_version` in `gradle.properties`: both sides refuse another version.

## Tests

The crate's tests cover the payload encoding, the goal table, board generation, the rules (tooltips, messages,
team progress), saving, drafts and the hooks (through Horizon's own `dispatch_*` functions). They need a Horizon
checkout to build against: add the crate to that checkout's workspace with `horizon-client` as a path dependency,
then

```bash
cargo test -p mod-lockout
```

They were run against horizon-rust/horizon `4626921895df51521be64d485ee763412de2917c`.
