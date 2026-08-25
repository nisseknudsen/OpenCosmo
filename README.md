# Cosmo's Cosmic Adventure — Reboot

A from-scratch Rust/Bevy remake of Apogee's 1992 DOS platformer *Cosmo's
Cosmic Adventure*, built by decoding real game assets out of your GOG
installer at build time and porting the original engine's physics/behavior
from source.

## Status: playable vertical slice, Episode 1

- All 11 shipped main stages (`A1`–`A11`) plus both bonus stages convert and
  render correctly (tiles, backdrops, level geometry).
- Player movement is a faithful port of the original's `MovePlayer()` /
  `TestPlayerMove()`: tile-stepped walking, the real jump curve, gravity
  ramp-up, wall-cling, and collision — running on an 18.2Hz fixed tick
  matching the DOS timer rate the original used.
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
  track assignment (packed into the level header's flags word).
- The status bar is rebuilt from the game's own `STATUS.MNI` panel and
  `FONTS.MNI` glyphs — score, stars and bombs as flush-right digit runs,
  health as the original's stacked filled/empty cell meter.
- Title screen, main menu and credits screen, drawn with the real artwork
  and font. The original's menu also offers Restore/Story/Instructions/
  High Scores/Game Redefine/Ordering Info/BBS/Demo, which depend on
  subsystems this remake hasn't ported, so only working entries are shown.
- Touching a level's exit actor advances to the next stage in the real
  `A1 A2 bonus1 bonus2 A3 A4 …` progression.
- Not yet implemented: lives/game-over, sound effects (PC-speaker `SND_*`,
  separate from the AdLib music), bomb placement, switches/doors/moving
  platforms, and Episodes 2–3 (same pipeline, just needs `COSMO2`/`COSMO3`
  wired up the same way as `COSMO1`).

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

**Controls**: Left/Right or A/D to move, Space to jump, Up/Down (or W/S)
to look up/down while stationary.

**Debug env vars**: `COSMO_LEVEL=<stem>` picks the starting level (e.g.
`bonus1`); `COSMO_AUTOPLAY=1` drives the player automatically (used for
headless verification during development, not meant for normal play).

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
