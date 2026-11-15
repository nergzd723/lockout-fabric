# Lockout for Horizon

A port of Lockout to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition
26.3. It is a Horizon **native trait mod**: Rust compiled into the game, using only Horizon's catalog hook points
and `Host` helpers ([Horizon's native mod docs](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md)).

It speaks the Fabric mod's network payloads byte for byte (`lockout:*` channels, version `0.10.4`), so the two mix:

| Server | Client | Works |
| --- | --- | --- |
| Horizon + this mod | Horizon + this mod | yes |
| Horizon + this mod | Fabric + Lockout 0.10.4 | yes: the Fabric client draws the board, tooltips and all |
| Fabric + Lockout 0.10.4 | Horizon + this mod | yes: the mod answers the version check, so you aren't kicked |
| Horizon + this mod | vanilla or Horizon without the mod | plays, without a board on screen |

## Layout

```
horizon/
  lockout/                  the bundle the launcher builds
    horizon-mixin.toml      [mod] id/version, [api] path
    api/Cargo.toml          the trait mod's crate (depends on horizon-client only)
    api/data/goals.json     the goal table, generated from the Java sources
    api/src/lib.rs          the Hooks: turns Horizon's events into match events
    api/src/server.rs       the match: teams, goals, completion, winning
    api/src/client.rs       the board on screen
    api/src/goals.rs        goal names, icons and what completes each
    api/src/board.rs        random boards (GoalGroup and GoalRequirements rules)
    api/src/wire.rs         the Fabric payloads
  tools/extract_goals.py    regenerates goals.json
```

Native bundles may only carry `.rs` and `.toml` files, `data/` folders and READMEs, which is why the extractor lives
outside `lockout/`.

## Install

You need the Horizon launcher and an instance of Horizon whose native hook API is 645 or later (Nightlies from
2026-11-15 on). The bundle isn't pinned to a commit, so it builds on the instance's own Horizon.

```bash
horizon-launcher native-build horizon/lockout --instance <instance> --trust-native-code
```

Native mods run unsandboxed with your rights; read the code before you build it. The same build serves single
player, LAN hosting and a dedicated server (`--server-output` for a standalone server).

## Commands

The same as the Fabric mod's, for operators (level 2):

- `/lockout teams <team> <team> ...` - a match between vanilla teams (`/team add`, `/team join`, `/team modify ... color`)
- `/lockout players <player> <player> ...` - every player on their own team
- `/blackout team <team>`, `/blackout players <player> ...` - one team clears the whole board
- `/SetBoardSize <3-7>`, `/SetStartTime <5-300>`
- `/RemoveCustomBoard`; a Fabric client's `/SetCustomBoard` works against a Horizon server too
- `/lockout located <true|false>` - Horizon-only, see below
- `/lockout stop` - Horizon-only: ends the running match

When a match starts, every player is cleared (inventory, effects, XP, advancements), the lockout players wait in
adventure mode through the countdown and everyone else spectates, as with the Fabric mod.

In game, hold **B** to list the board's goals, who holds each, and the progress the server sent (unique mobs killed,
biomes visited...). The key can be changed as the mod key `key.lockout.open_board`.

## What's the same, and what isn't

The goal table comes from the Java sources, so ids, names, data (dye colours, leather pieces) and the board rules
(goal groups and their limits, team-count rules, goals left out of the random pool) are the Java mod's. Of its 338
goals, 299 are tracked on a Horizon server. The other 39 still draw on the board when a Fabric server sends them, but
a Horizon server leaves them off random boards, because Horizon has no hook for what they need yet: armour stands,
end crystals, grindstone/smithing/cartography use, glow ink, campfires, chiseled bookshelves, brushing, candles,
bundles, composting, copper golems, allays, decorated pots, leashing, spyglass spying, ender pearls through portals,
abandoned camp loot, banners on shields, TNT minecart deaths and fireworks shot from crossbows.

Also different from the Fabric mod:

- Goals that need a biome or structure nearby (villages, monuments, swamp huts...) are on random boards by default,
  because Horizon gives mods no way to locate them. `/lockout located false` leaves them off.
- The board draws items, not the Fabric mod's custom textures: each goal shows the item its texture stands for.
- Not ported: drafts, the Board Builder screen, saving a match across restarts, the player-tracking compass, the
  pause when a player leaves, and the vanilla changes (1.16 bartering, Medium raids, zombies always converting
  villagers).
- A Horizon client on a Fabric server plays no completion sounds (the Fabric client plays them itself); on a Horizon
  server the server sends them.

## Keeping it in step with the Fabric mod

- After a goal changes on the Java side, run `python3 horizon/tools/extract_goals.py` and commit `goals.json`.
- `wire::LOCKOUT_VERSION` must equal `mod_version` in `gradle.properties`: both sides refuse another version.
- New goal kinds need a `Trigger` in `goals.rs` and its check in `server.rs`; until then they are unsupported.

## Tests

The crate's tests cover the payload encoding, the goal table, board generation, the match logic and the hooks
(through Horizon's own `dispatch_*` functions). They need a Horizon checkout to build against: add the crate to that
checkout's workspace with `horizon-client` as a path dependency, then

```bash
cargo test -p mod-lockout
```

They were run against horizon-rust/horizon `4626921895df51521be64d485ee763412de2917c`. The mod hasn't been played in a
running Horizon game yet.
