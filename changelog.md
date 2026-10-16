## Lockout v0.10.4
- Added 18 goals:
    - Visit 10 Unique Biomes, Visit 15 Unique Biomes (Overworld biomes, counted per team)
    - Kill a Creaking (break its Creaking Heart while it's protecting it), Kill a Zombie Nautilus
    - Loot an Abandoned Camp, Obtain a Sulfur Cube Bucket, Obtain all Eggs (white, blue, brown), Sleep in a Straw Bed
    - Shoot an Opponent with an Arrow, Pick Up an Item an Opponent Dropped (thrown or from their death) - teams only
    - Die by Suffocation, Die by Stalagmite, Die by Magma Block, Die 5 Unique Ways
    - Deflect a Projectile with a Shield, Cure a Zombie Villager, Return to Sender (kill a Ghast with its own
      fireball), Subspace Bubble
- New goals only appear when the biomes/structures they need are near spawn (e.g. Pale Garden for the Creaking,
  a warm and a cold biome for all eggs).
- Kill Breeze using Wind Charge now requires Trial Chambers near spawn.

## Lockout v0.10.3
- Drafts now start with scouting: drafting players are frozen for 15 seconds before the first pick, can jump to toggle
  a bird's-eye view 30 blocks up, and get a Spyglass to zoom with (taken back afterwards).
    - `/SetScoutingTime <seconds>` (0-60, 0 turns it off), default in the config file ('draft scouting seconds' 15)

## Lockout v0.10.2
- Matches now survive a server restart. The match is saved in the world folder (every 30 seconds and when the server
  stops) and restored when the server starts again: board, teams, points, timer and all goal progress.
- A restored match stays paused until every player is back. Players who are already online are frozen in adventure
  mode and can't be hurt; once everyone is back, the match resumes after a 5 second countdown. The timer doesn't run
  while paused.
- A draft in progress is not restored after a restart.

## Lockout v0.10.1
- Added solo drafts: `/blackout draft players <player names>` or `/blackout draft team <team name>`.
  One team picks every goal (with the same offers, timer and rerolls), then plays the board as Blackout.
  This also works in singleplayer.
- Fixed a crash when a draft finished while one of its players was offline.

## Lockout v0.10.0
- The mod is now on Minecraft 26.3.
- Added Draft mode: teams build the board by picking goals in turns (snake order) before the match starts.
    - `/lockout draft teams <team names>` or `/lockout draft players <player names>`, `/lockout draft cancel`
    - `/SetDraftSettings <offer size> <pick seconds> <rerolls>`, with defaults in the config file
      ('draft offer size' 2, 'draft pick seconds' 10, 'draft rerolls' 1)
    - Picks respect the same goal requirements and conflicts as random boards.
    - Players who disconnect during the draft are still on their team when the match starts.
- 'Obtain 4 Unique Saplings' now counts Poplar and Pale Oak Saplings.
- 'Obtain every type of Horse Armor' now requires Copper Horse Armor.
- 'Use Glow Ink on Crimson Sign', 'Use Glow Ink on Warped Sign' and 'Use Glow Ink on Crimson or Warped Sign' can't appear on the same board.
- Only one 'Reach XP level' goal can appear on a board.
- Added 52 goals:
    - Breed Armadillos, Breed Camels
    - Craft 20/50/100 Unique Items (unique crafts are counted per team)
    - Die by Drowning, Die by falling in Void, Die to Pufferfish, Die to Trident, Die to Warden
    - Eat Beetroot Soup
    - Kill Bogged, Kill Husk, Kill Parched, Kill Warden, Kill Pillager using Crossbow
    - Mine Obsidian
    - Obtain "Mob Kabob", "Stay Hydrated!" and "The Power of Books" Advancements
    - Obtain 2 Unique Armor Trims, Obtain 3 types of Horse Armor, Obtain 3 Unique Banner Patterns
    - Obtain 16 Glow Lichen, Obtain 32 Arrows (or Spectral Arrows), Obtain 64 Coarse Dirt
    - Obtain all Copper Tools, Obtain every type of Furnace, Obtain every type of Torch
    - Obtain Amethyst Block, Bone Block, Block of Resin, Copper Chest, Dead Bush, Dried Kelp Block, Emerald Block,
      Gilded Blackstone, Lodestone, Netherite Scrap, Pottery Sherd, Resin Brick Wall, Smooth Basalt, Smooth Quartz Stairs, Tinted Glass
    - Obtain Colored Harness
    - Reach XP level 10, Reach XP level 20
    - Tame a Nautilus
    - Tune a Note Block
    - Use Glow Ink on Warped Sign, Use Glow Ink on Crimson or Warped Sign
    - Wear Full Copper Armor
- 'Obtain Gilded Blackstone' is not in the random goal pool, since bastions can't be located before the match.
- Added 38 more goals:
    - Break any Armor Piece, Break any Tool, Break Fishing Rod, Break Shield
    - Place a Hanging Sign, Place Painting
    - Compost 3/5/7 Unique Foods (counted per team)
    - Construct a Copper Golem, Duplicate Allay, Duplicate any Armor Trim
    - Eat every type of Soup
    - Enrage Enderman, Explode End Crystal, Feed Golden Dandelion to any Baby Mob, Fill a Decorated Pot
    - Leash 4/6/8 unique Mobs at once, Leash an Iron Golem
    - Lock a Map using Cartography Table, Use Cartography Table
    - Mine 3 types of Ice
    - Name a Ghast 'Dinnerbone', Name a Sheep 'jeb_', Name an Iron Golem 'Dinnerbone'
    - Obtain a stack of any Item
    - Spy on 10/15/20/25 Unique Mobs (counted per team), Spy on Enderman, Spy on Iron Golem, Spy on Piglin Brute
    - Teleport through Nether Portal using Ender Pearl
    - Visit all Cave Biomes
    - Wear Full Armor with different materials
- 'Spy on Piglin Brute' is not in the random goal pool, since bastions can't be located before the match.
- 'Visit all Cave Biomes' only appears when Lush Caves, Dripstone Caves and the Deep Dark are all near spawn.

## Lockout v0.9.3
- Fixed spectators getting advancements causing a crash

## Lockout v0.9.2
- Added `/SetBoardSize <board size>` command. This replaces `lockoutBoardSize` gamerule.
- Added `/BoardSide <left/right>` command.
- Added a config file in `.minecraft/config/lockout.json`.
    - 'default board size' - changes the board size when the server (or singleplayer world) starts.
        - default: 5
    - 'board position' - changes the position of the board (left/right)
        - default: "right"
    - 'show NoiseRouter line' - shows the long NoiseRouter line on debug hud (F3)
        - default: false
- Biome/Structure search radius has been reduced to 750 blocks (in each direction from world spawn; 1500x1500 square).
- Board is rendered above visual effects such as vignette, nausea etc.
- Fixed 'Decorate Shield with Banner'.
- Added height map debug line to debug hud (F3) - O value is often used for knowing terrain height when underground.
- Added goals:
    - 'Obtain 64 Arrows'
- Removed goals from random goal pool:
    - 'Fill Bundle with 16 empty Bundles' - overcooked

## Lockout v0.9.1
- Fixed #22 (zombie-type mobs killing anything in the nether caused a crash)

## Lockout v0.9.0
- The mod is now on version 1.21.3.
- There is a new gamerule (`lockoutBoardSize`) that allows you to play boards of different sizes. You can play boards as small as 3x3, and as large as 7x7. This change also applies to Board Builder. (#10 by @bbb651)
- Fixed most crashes and disconnect bugs.
- Biome/Structure search radius has been reduced to 1000 blocks.
- Status effects are now shown next to the board.
- Board Builder can be opened with existing boards. (`/BoardBuilder <board name>`)
- Updated some icons to better reflect their goals.
- Added 7 goals:
    - Shoot Firework from Crossbow
    - Mine Crafter
    - Light Candle
    - Wear Full Enchanted Armor
    - Put Wolf Armor on Wolf
    - Kill Breeze using Wind Charge
    - Fill Bundle with 16 empty Bundles
- Changed some goals:
    - Obtain End Crystal -> Place End Crystal
    - 'Opponent dies 3 times' is now shared in the team
    - 'Wear a carved pumpkin for 5 minutes' - progress doesn't reset after you stop wearing a pumpkin, it's saved and shown on the board
- Removed some goals:
    - Kill Ender Dragon (there is already 'Obtain Dragon Egg' goal which is more straightforward)
- Some goals are no longer part of the random goal pool, but they can still be played on custom boards:
    - End city related goals - too much rng, cities can spawn thousands of blocks away
    - Opponent obtains seeds/obsidian - no way to force these if it's left as a tie breaker
---
## Lockout v0.8.1
- Literally nothing, I released v0.8.0 again instead of v0.8.1
    - Should have implemented a fix for #6 (rejoining the server wouldn't work)

## Lockout v0.8
- Added 3 new goals:
    - 'Drink water bottle'
    - 'Use brush on suspicious sand/gravel'
    - 'Die to Polar Bear'
- Some 1v1 team goals can appear on random boards now
- Spectator advancements (e.g. entering nether/fort/bastion) will no longer display in chat
---
## Lockout v0.7:
- Added 3 new goals:
    - 'Kill bat',
    - 'Fill chiseled bookshelf'
    - 'Opponent eats food'
- Timer is now server-sided; added pre-game countdown
- Added `/SetStartTime <seconds>` command (in seconds, between 5-300)
- Changed a few goal icons
- Fixed a bug with Piglin bartering
---
## Lockout v0.6:
- Added ability to create and play custom boards (`/BoardBuilder`, `/SetCustomBoard <board name>`)
- Added spectators (every player who isn't a part of a game is a spectator)
- Villagers now always convert to Zombie Villagers if they are killed by Zombies
---
## Lockout v0.5:
- Reverted to version 1.20.1
- Team chat now works outside the game as well.
---
## Lockout v0.4:
- Added support for single-player and LAN worlds.
- Added Blackout mode (`/blackout players <player name> <player name>...` or `/blackout team <team name>`)
---
## Lockout v0.3:
- Added `/GiveGoal <player> <goal number>` command
- Added 4 new goals
- Fixed some bugs
---
## Lockout v0.2:
- Fixed many bugs, added compasses
---
## Lockout v0.1
- Initial release