# Fabric and Horizon parity

Lockout lives here twice: the Fabric mod (`src/`, Java) and the Horizon port (`horizon/`, Rust,
[horizon/README.md](horizon/README.md)). They are one mod. They talk to each other over the network, read each
other's saves, config and custom boards, and must play the same. This file records where the two stand and how to
keep them there.

## The rule

A change to what players see or do lands on both sides in the same commit or pull request, with one
`changelog.md` entry. Where one side can't have it (no hook point, a platform limit), it gets a row under
[Differences](#differences) saying why, and a row in the [sync log](#sync-log) either way.

## What CI checks

`.github/workflows/check.yml`, on every push and pull request:

- `python3 -I horizon/tools/check_sync.py`: `goals.json` is what `extract_goals.py` makes of the Java goals; one
  version everywhere (`gradle.properties` `mod_version`, `wire::LOCKOUT_VERSION`, both bundles' `horizon-mixin.toml`,
  the three crates); both bundles pin the same Horizon commit; both pack as the launcher packs them.
- `./gradlew build`: the Fabric mod.
- `cargo test -p mod-lockout` on the Horizon commit the mod pins (`horizon/dev/lockout-test`), and the local runner
  (`horizon/dev/lockout-run`) builds.

`.github/workflows/release.yml` runs `check_sync.py` too and attaches `lockout-<version>.hnmod`
(`horizon/tools/pack_hnmod.py`) beside the jar, which is what `horizon-launcher mods add-github` installs.

CI can't check behaviour. Goal conditions, chat lines and tooltips are ported by reading the Java; the Rust tests
pin the ported behaviour (`rules.rs`, `server.rs`, `lib.rs` tests), so a Java change shows up as a Rust test to
change.

## The shared contract

What the two sides exchange, so it must be identical:

| What | Fabric | Horizon | Checked by |
| --- | --- | --- | --- |
| Network payloads (`lockout:*`) | `network/*Payload.java`, `Networking` | `wire.rs` | `wire.rs` tests, by hand |
| Version (both refuse another) | `gradle.properties` `mod_version` | `wire::LOCKOUT_VERSION`, `horizon-mixin.toml`, `Cargo.toml` | `check_sync.py` |
| Goal ids, names, groups, requirements | `lockout/goals/**`, `GoalGroup`, `GoalRequirements` | `api/data/goals.json` | `check_sync.py` |
| Saved match (`lockout-match.json`) | `MatchPersistence` | `persist.rs` (same field names) | `persist.rs` tests, by hand |
| Config (`config/lockout.json`) | `LockoutConfig` | `client.rs`, `draft.rs`, `lib.rs` (same keys) | by hand |
| Custom boards (`.minecraft/lockout-boards`) | `BoardBuilderIO` | `screens.rs` | by hand |
| Chat lines, colours, tooltips | goal classes, `Lockout`, `LockoutServer` | `rules.rs`, `server.rs` | Rust tests, by hand |

## Where each part lives

| Fabric (`src/main/java/me/marin/lockout/`) | Horizon (`horizon/lockout/api/src/`) |
| --- | --- |
| `Lockout`, `server/LockoutServer`, `server/handlers/*` | `server.rs` (the match), `lib.rs` (events to match) |
| `lockout/goals/**`, `lockout/interfaces/*` | `goals.rs` (triggers), `rules.rs` (completion), `data/goals.json` |
| `mixin/server/*` | `lib.rs` hooks; raids and zombie villagers in `horizon/.lockout_tweaks/` |
| `generator/*` | `board.rs` |
| `server/DraftSession`, `server/DraftScouting` | `draft.rs` |
| `server/MatchPause` | `pause.rs` |
| `server/MatchPersistence` | `persist.rs` |
| `mixin/server/CompassItemMixin`, `server/handlers` (compass) | `compass.rs` |
| `network/*` | `wire.rs` |
| `client/LockoutClient`, `Utility.drawBingoBoard`, `mixin/client/*` | `client.rs`; the debug screen in `horizon/.lockout_tweaks/` |
| `client/gui/*` | `screens.rs` |
| `LockoutInitializer` (commands, bartering table) | `lib.rs` (commands, `bartering_table`) |

## Features

| Feature | Fabric | Horizon |
| --- | --- | --- |
| All 338 goals, with the Java conditions | yes | yes |
| Lockout and Blackout (`/lockout teams\|players`, `/blackout team\|players`) | yes | yes |
| Random boards: groups, requirements, locating biomes and structures | yes | yes |
| Drafts, solo drafts, scouting (`/lockout draft ...`, `/blackout draft ...`, `/SetDraftSettings`, `/SetScoutingTime`) | yes | yes |
| `/lockout cancel`, `/blackout cancel`: end a draft, or a match with no winner | yes | yes |
| Countdown, players unhurtable until the start, timer, winner, clinch and tie rules | yes | yes |
| Team chat (`/chat team\|local`) | yes | yes |
| Tracking compass | yes | yes |
| Saving every 30 s, restoring, the pause after a restart | yes | yes |
| `/GiveGoal`, `/SetStartTime`, `/SetBoardSize`, `/RemoveCustomBoard`, also in lowercase (`/givegoal`...) | yes | lowercase only (Horizon takes no capitals in a command name; the console takes either) |
| Version check, kicking a client without the mod | yes | yes |
| The board on screen, `/BoardPosition left\|right\|hidden`, hold B for the board | yes | yes |
| Board, draft and Board Builder screens, `/BoardBuilder`, `/SetCustomBoard` | yes | yes |
| Completion sounds, hidden toasts | yes | yes (see differences) |
| Lockout's debug screen (F3) | yes | with `lockout_tweaks` |
| 1.16 piglin bartering | yes | yes |
| Normal raids on every difficulty, zombies always converting villagers | yes | with `lockout_tweaks` |
| `show NoiseRouter line` (config) | yes | no: Horizon's debug screen has no such line |

## Differences

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
- The board hides while the debug screen (F3) is open on either side; the Java client hides it on the left only,
  because its debug screen drops the right column, which Horizon's keeps without `lockout_tweaks`.
- Toasts already on screen when the board arrives stay (Horizon can refuse a toast, not hide one), so on a
  Lockout server the tutorial's hints are refused from the start.
- `lockout_tweaks` (raids, zombie villagers, the debug screen) is a separate bundle, built on its own
  (horizon/README.md, "Install"); its debug screen shows the targeted block, fluid and entity only while Horizon's
  own debug screen has them on, where the Java one always shows them.
- The Horizon port has been type-checked and unit-tested against Horizon, not yet played in a running Horizon game.

## Making a change

- A goal added or changed in Java: `python3 -I horizon/tools/extract_goals.py`, commit `goals.json`; a new kind of
  goal needs a `Trigger` in `goals.rs` and an arm in `rules.rs` (`every_goal_is_supported` fails until it has one).
- A payload or a field of one: both sides, and a new version on both (`check_sync.py` lists every place).
- A command: both sides, with the same permission and messages, and both READMEs' command lists.
- A version bump: rename `changelog.md`'s `## Unreleased` to `## Lockout v<version>` (the release refuses
  otherwise), then `gradle.properties`, `wire.rs`, `horizon/lockout/horizon-mixin.toml`,
  `horizon/.lockout_tweaks/horizon-mixin.toml`, `horizon/lockout/api/Cargo.toml`, `horizon/dev/*/Cargo.toml`.
- A new Horizon commit to pin: both `horizon-mixin.toml`s, then the tests on it (horizon/README.md, "Tests").
- Add a row to the sync log below.

## Sync log

| Date | Change | Fabric | Horizon |
| --- | --- | --- | --- |
| 2026-11-15 | The port audited goal by goal against the Java at `97fa71d` (v0.10.4, the clinch rule included) | `src/` at `97fa71d` | `horizon/` |
| 2026-11-16 | `/lockout cancel`, `/blackout cancel` | `LockoutServer.cancelCommand` | `State::cancel_match` |
| 2026-11-16 | `/BoardPosition hidden` | `LockoutConfig.BoardPosition.HIDDEN` | `client::BoardPosition::Hidden` |
| 2026-11-16 | The debug screen's targeted block, fluid and entity | already | `.lockout_tweaks/debug_screen.rs` |
| 2026-11-17 | Lowercase names for the six capitalised commands | `LockoutInitializer` (redirects) | `COMMANDS` |
| 2026-11-17 | A goal completed during a player's action reaches every board (Horizon server) | n/a | `online()` |
