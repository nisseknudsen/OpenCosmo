# Cosmo's Cosmic Adventure — Reboot

A from-scratch Rust/Bevy remake of Apogee's 1992 DOS platformer *Cosmo's
Cosmic Adventure*, built by decoding real game assets out of your GOG
installer at build time and porting the original engine's physics/behavior
from source.

## Status: playable, all three episodes

- All three episodes convert and play: 14/13/13 levels respectively, each
  with its own level naming (`A*`/`B*`/`C*`), bonus stages, backdrops and
  music. Select with `COSMO_EPISODE=1|2|3`.
- Player movement is a faithful port of the original's `MovePlayer()` /
  `TestPlayerMove()`: tile-stepped walking, the real jump curve, gravity
  ramp-up, slopes, and collision — running on an 18.2Hz fixed tick
  matching the DOS timer rate the original used. Walls that can be clung
  to are grabbed automatically by walking into them mid-air and climbed by
  jumping; ones that are *also* slippery let Cosmo slide down a row a tick
  until he runs out of wall.
- The view is the original's stateful `scrollX`/`scrollY` pair rather than
  a camera centred on the player: it only gives chase once the player
  leaves a dead zone, and holding up or down while standing still walks it
  through the world a row at a time. That last part is a mechanic, not a
  flourish — actors only run while on screen, so looking up at a stack of
  prizes overhead is what wakes them and drops them on you.
- Hint globes work: standing at one and pressing up opens its message, and
  the first globe of each level speaks up unprompted. All 26 episode-1
  messages plus episodes 2 and 3's are transcribed from the source.
- Dying replays the level from scratch — enemies included — rather than
  just moving the player back, matching the original's
  `LoadGameState('T'); InitializeLevel(levelNum)`.
- Actors (enemies, items, decorations) render at their authored positions
  using the correct sprite for each type — `ACT_*` (map actor type) and
  `SPR_*` (the sprite actually drawn) are different numbering spaces that
  only sometimes coincide, resolved via `actor_sprite_map.rs`.
- Collectibles (food/gems, a curated list of `ACT_*` ids) add to score on
  contact and despawn. A curated hazard subset damages the player on
  contact (health starts at 4, matching the original) and a "walker"
  subset of those patrols left/right, reversing at walls/ledges — this is
  a pragmatic generic pass, *not* a per-type port of each actor's real
  behavior function (`ActXxx` in game1.c; there are ~250 of them).
- Original AdLib music plays per-level (decoded IMF → OPL2 synthesis →
  looped WAV playback), matched to each level via the real per-level
  track assignment (packed into the level header's flags word), alongside
  the PC-speaker sound effects with the original's priority behaviour.
- Collectibles use the original's own pickup table: score varies by item
  (200/400/800/1600/3200), stars and bombs feed their own counters, a
  hamburger widens the health meter and a power-up heals or pays out.
- Dying rewinds score, stars, bombs and health to their values on entering
  the level, the way the original's checkpoint save does.
- The status bar is rebuilt from the game's own `STATUS.MNI` panel and
  `FONTS.MNI` glyphs — score, stars and bombs as flush-right digit runs,
  health as the original's stacked filled/empty cell meter.
- Title screen, main menu and credits screen, drawn with the real artwork
  and font. The original's menu also offers Restore/Story/Instructions/
  High Scores/Game Redefine/Ordering Info/BBS/Demo, which depend on
  subsystems this remake hasn't ported, so only working entries are shown.
- Touching a level's exit actor advances to the next stage in the real
  `A1 A2 bonus1 bonus2 A3 A4 …` progression.
- Per-actor enemy AI for 14 of the original's `ActXxx()` behaviours,
  covering ~47% of Episode 1's level actors.
- Pouncing kills creatures and launches the recoil bounce, with per-type
  recoil and hit points (a ghost soaks four pounces, a parachute ball two,
  a basket bursts with a softer bounce). Bombs are collected, placed with
  Alt, and detonate into a real 6x6-tile blast that damages enemies and
  the player alike — and reaches things a pounce can't, like the roamer
  slug.
- Everything is drawn into one 320x200 offscreen buffer at the original's
  exact resolution, and that buffer is scaled to the window exactly once.
  Two presentation modes, toggled with **F5**: *remaster* (the default)
  fills the window with sharp-bilinear scaling plus a restrained CRT pass —
  scanlines, brightness-thresholded bloom, a slight vignette — and
  *authentic* uses whole-number scaling with letterboxing and no filtering
  at all. Rendering to a fixed buffer is also what let the layout match the
  original's screen exactly, including the 8px black border around the play
  area that the window-relative version had no way to express.
- F1 opens the help menu (restart, level warp, quit). The original also
  offers Save/Restore/Help/Game-redefine/High-scores, which depend on
  unported subsystems, so only working entries are listed. "L)evel Warp"
  is a development aid with no counterpart in the shipped game: it jumps
  to any slot in the episode's progression.
- Not yet implemented: switches and doors, turret projectiles, moving
  platforms (needs live map-tile mutation), the scooter and transporter,
  the dizzy/ice-slide player states, and the actor types only episodes 2–3
  enable. There is no lives or game-over system because the original has
  none — dying costs you the level's progress, not a life.

## Building & running

You need your own copy of the GOG installer for the game (the one you
already own) — this repo contains no game assets, only code that reads
yours. Requires the original `original/*.sh` (excluded from git) to build,
since asset conversion happens in `cosmo-game`'s `build.rs`.

```sh
# from the repo root, with your GOG installer .sh placed in ./original/
cargo run -p cosmo-game
```

The first build converts assets into `crates/cosmo-game/assets/generated/`
(gitignored) and caches the result by content hash — subsequent builds skip
reconversion entirely unless the installer file or converter code changes.
To point at an installer somewhere else: `COSMO_INSTALLER=/path/to/installer.sh cargo run -p cosmo-game`.

**Controls**: Left/Right or A/D to move, Ctrl or Space to jump, Alt to
drop a bomb, Up/Down (or W/S) to look up/down while stationary. Jump onto
a creature to pounce it. Ctrl/Alt match the original's own defaults; Space
is added as a modern alternative for jump.

**Menu**: any key at the title screen, then B to begin, C for credits,
T back to the title, Q to quit. In game, F1 opens the help menu (R to
restart the level, L for the level warp, Q back to the main menu, ESC to
resume); the game is paused while it's open. In the level warp, the arrow
keys move the cursor (left/right jump a column), Enter goes, ESC backs
out. Any key dismisses a hint globe's message.

**Debug env vars**, all used for headless verification during development
rather than normal play: `COSMO_LEVEL=<stem>` picks the starting level
(e.g. `bonus1`); `COSMO_STATE=menu|credits|playing` jumps straight to a
screen; `COSMO_SPAWN=x,y` overrides the player's start tile;
`COSMO_GIVE_BOMBS=n` stocks the bomb counter; `COSMO_HELP=1` /
`COSMO_WARP=1` open the help menu or level warp on the first frame;
`COSMO_PRESENT=authentic|remaster` picks the presentation mode;
`COSMO_EPISODE=1|2|3` picks the episode; `COSMO_AUTOPLAY=1` drives
the player automatically. Run with `RUST_LOG=cosmo_game=debug` to log
pounce/bomb/blast events.

**Verifying a mechanic headlessly.** `COSMO_INPUT` scripts the controls as
comma-separated `<keys><ticks>` steps — keys being `w`/`e` (west/east),
`u`/`d` (look up/down), `j` (jump), `b` (bomb), `k` (dismiss a text
frame), `.` (nothing). `COSMO_TRACE=<n>` prints player position, facing,
frame, scroll, cling and live enemy count every nth tick;
`COSMO_SHOT=<path>` grabs the window once (at `COSMO_SHOT_AT`, default
tick 30), `COSMO_SHOT_RAW=1` grabs the 320x200 virtual screen instead —
the one to use for pixel-alignment questions, since it hasn't been through
the present shader or the display's scale factor — and
`COSMO_QUIT_AFTER=<ticks>` ends the run. So, for example,
checking that looking up and down actually pans the view:

```sh
COSMO_STATE=playing COSMO_LEVEL=a1 COSMO_TRACE=3 COSMO_QUIT_AFTER=120 \
  COSMO_INPUT="e8,k3,e30,.3,u30,.3,d40" cargo run -p cosmo-game
```

The `rel_row` column in that trace is the player's row within the window,
and it is what should move while `pos` stays put.

## Architecture

- `crates/cosmo-assets/` — the asset pipeline. Unwraps the makeself/mojosetup
  installer shell script, parses Apogee's VOL/STN directory format, decodes
  EGA planar tile/sprite graphics, level maps, and (music format permitting)
  AdLib tracks, and writes everything to a cache-stamped output directory as
  PNGs/JSON/WAV. See each module's doc comment for exact byte-format
  citations back to the source.
- `crates/cosmo-game/` — the Bevy game. `build.rs` triggers asset conversion;
  `src/` has one module per subsystem (tileset, level, player, camera,
  actors, flow/level-progression, hud).
- `reference/cosmore/` — a vendored copy of `smitelli/cosmore` (MIT
  licensed), a full decompilation of the original `COSMO{1,2,3}.EXE`. This
  is the primary source for every format/physics decision in this project;
  see `docs/file-formats.md` for the research summary and exact citations.
- `docs/file-formats.md` — technical reference for every file format and
  physics constant, with source line citations and an explicit list of
  what's confirmed vs. still uncertain.

## Extending to Episodes 2 & 3

The pipeline is already episode-agnostic at the format level (VOL/STN/tile/
level/sprite decoding doesn't care which episode's data it's fed). Extending
requires: adding `COSMO2`/`COSMO3` variants of `convert_episode1` (rename to
something like `convert_episode`, parameterized by the `COSMO{N}` file
prefix) and a level-select/episode-select flow in `cosmo-game`. The GOG
installer used here already contains all three episodes' data
(`COSMO2.VOL`/`.STN`/`.EXE`, `COSMO3.VOL`/`.STN`/`.EXE`), so no additional
asset acquisition is needed.
