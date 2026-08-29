# OpenCosmo

An open-source, from-scratch reimplementation of Apogee's 1992 DOS platformer
*Cosmo's Cosmic Adventure*, written in Rust on the [Bevy](https://bevyengine.org)
engine.

**This repository contains no game data.** It ships only code, which reads the
art, levels, music and sound out of a copy of the game *you* own — at build
time, on your machine. You need your own copy to play.

## What it's for

Two things at once, and the tension between them is the point:

**Faithfulness.** Behaviour is ported from
[Cosmore](https://github.com/smitelli/cosmore), Scott Smitelli's MIT-licensed
decompilation of the original executables, rather than reverse-engineered by
eye. Physics run on an 18.2 Hz fixed tick because that is the rate the DOS
timer interrupted at, and every constant in the original is expressed per
tick — the jump curve is a table of ten per-tick offsets, walking is one tile
per tick. The camera is the original's stateful `scrollX`/`scrollY` pair with
its dead zone, not a camera centred on the player. Non-trivial decisions in
this codebase cite the source file and line they came from.

**A modern presentation layer that doesn't lie about the pixels.** Everything
draws into a single 320×200 buffer at the original's exact resolution, which
is scaled to the window exactly once. On top of that sits an optional
*remaster* mode — Scale3x smoothing, sharp scaling, a restrained CRT pass, and
motion interpolation — every part of which is off in *authentic* mode. The
simulation is identical either way; only the presentation changes.

## Requirements

- A **Rust toolchain** (stable, 2021 edition or later) — install via
  [rustup](https://rustup.rs).
- A GPU/driver stack that Bevy 0.16 supports (Vulkan, Metal or DX12).
- **Your own copy of the game.** The build reads the GOG installer for
  *Cosmo's Cosmic Adventure* (a `.sh` file, which is a shell script with a
  zip archive appended). The GOG release contains all three episodes' data, so
  nothing else needs acquiring.
- On Linux, Bevy's usual system dependencies (`alsa-lib`, `libudev`,
  `libX11`/Wayland development packages). See
  [Bevy's Linux setup notes](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

## Building and running

Put your GOG installer in `./original/` (the directory is gitignored), then:

```sh
cargo run --release -p opencosmo-game
```

The first build converts assets into `crates/opencosmo-game/assets/generated/`
(also gitignored) and caches the result by content hash, so later builds skip
reconversion entirely unless the installer or the converter code changes.

To point at an installer elsewhere:

```sh
COSMO_INSTALLER=/path/to/installer.sh cargo run --release -p opencosmo-game
```

`--release` is worth it: the game crate builds at `opt-level = 1` in a dev
profile, and the present shader's cost scales with your window's pixel count.

Pick an episode with `COSMO_EPISODE=1|2|3` (there is no in-game episode
selector yet). Episode 1 is the most complete; 2 and 3 convert and play, but
lean on actor types that aren't ported yet.

## Controls

| Action | Default binding |
| --- | --- |
| Move | Left/Right, or A/D |
| Jump | Ctrl or Space |
| Bomb | Alt |
| Look up / down | Up/Down, or W/S (while standing still) |

Jump onto a creature to pounce it. Ctrl and Alt are the original's own
defaults; Space is added as a modern alternative for jump.

Walls that can be clung to are grabbed automatically by walking into them
mid-air, and climbed by jumping. Walls that are *also* slippery let Cosmo
slide down a row per tick until he runs out of wall.

Looking up and down is a mechanic, not a flourish: actors only run while on
screen, so panning up to a stack of prizes overhead is what wakes them — and
drops them on you.

**Gamepads** work with no configuration (d-pad or left stick to move, bottom
face button to jump, left face button to bomb). Everything is rebindable from
**G)ame Redefine** on the main menu: pick an action's number, then press the
key, button or stick direction you want. Bindings are saved to
`$XDG_CONFIG_HOME/opencosmo/controls.json`. Each of the six actions holds
several bindings at once, so keyboard and gamepad work without a mode switch.
A stick is treated as a d-pad with a firm deadzone — movement is tile-stepped
at 18.2 Hz, so there is no sub-tile precision for an analogue reading to
express.

**Menus**: any key at the title screen, then `B` to begin, `C` for credits,
`T` back to the title, `Q` to quit. In game, `F1` opens the help menu (`R`
restart level, `L` level warp, `Q` back to the main menu, `Esc` resume); the
game pauses while it's open. Any key dismisses a hint globe's message.

## Presentation modes

**F5** toggles between the two presets.

| | Authentic | Remaster (default) |
| --- | --- | --- |
| Scaling | whole-number, letterboxed | sharp-bilinear, fills the window |
| Artwork | untouched pixels | Scale3x smoothing |
| CRT pass | none | scanlines, thresholded bloom, slight vignette |
| Motion | 18.2 Hz, one step per tick | interpolated across frames |

**Scale3x** was chosen over smoother upscalers (xBRZ, HQx, neural) for one
property: it only ever *copies* a neighbouring pixel, never blends two, so the
16-colour EGA palette survives exactly. It also leaves interior dithering
alone, which matters because this game's backdrops run to 27% dither by pixel
count, and a blending upscaler turns that into blobs.

**Smooth motion** keeps game logic on its 18.2 Hz tick — raising the tick rate
wouldn't smooth anything, it would make Cosmo run three times faster — and
decouples drawing from it instead, spreading a tick's worth of movement across
the frames that fall inside it. Measured: 176 distinct drawn positions across a
walk instead of 53, in 1–3 pixel steps rather than 8-pixel jumps, still landing
on whole pixels so the pixel grid (and Scale3x) is undisturbed.

Individual effects can be dialled independently of the preset:
`COSMO_SCANLINE=0.3`, `COSMO_BLOOM`, `COSMO_VIGNETTE`, `COSMO_CURVE`,
`COSMO_SMOOTH`, `COSMO_SMOOTH_MOTION=on|off`. `COSMO_VSYNC=off` uncaps the
frame rate; `COSMO_WINDOW=1280x800` sets the window size;
`COSMO_PRESENT=authentic|remaster` picks the mode at launch.

## Audio

The original AdLib soundtrack plays per level — IMF decoded to OPL2 synthesis,
rendered to looping WAV — matched to each level by the real per-level track
assignment packed into the level header's flags word. PC-speaker sound effects
run with the original's monophonic priority behaviour, where a quieter effect
is dropped outright rather than mixed.

**F6** toggles an experimental re-voiced soundtrack, **off by default**. It
decodes the IMF register stream back into notes — pitch, timing, length and
velocity are all recoverable from documented hardware registers — and
re-renders them with warm additive voices, tape wobble and a low-pass. The
composition is untouched; only the instruments change. It isn't good enough
yet and the approach is being reconsidered; the code stays reachable via
`COSMO_AUDIO=remaster` for anyone who wants to pick it up.

## Status

**Under active development.** Playable start to finish on episode 1, with
plenty still missing. Expect rough edges, and expect internals to move.

Working: player movement, jumping, slopes, wall-clinging and collision, ported
from `MovePlayer()`/`TestPlayerMove()`; the original's scroll camera; the level
progression rule from `NextLevel()`, including the bonus stages it gates on
your star count; pouncing and bombs with per-type hit points and recoil; the
pickup table with its real per-item scores; the status bar rebuilt from the
game's own panel and font; the title, menu and credits screens; hint globes
with all messages transcribed from source; the foreground tile layer; actor
spawn offsets and the four `ConstructActor` flags (which between them define
the "look up and the prizes fall" behaviour); death replaying the level from
scratch, enemies included, and rewinding score, stars, bombs and health.

Per-actor AI is ported for 14 of the original's `ActXxx()` behaviours, covering
roughly 47% of episode 1's level actors. The rest fall back to a generic
hazard/walker pass, which is a pragmatic stopgap rather than a port.

Not yet implemented: switches and doors, turret projectiles, moving platforms
(needs live map-tile mutation), the scooter and transporter, the dizzy and
ice-slide player states, and the actor types only episodes 2–3 enable. There is
no lives or game-over system because the original has none — dying costs you
the level's progress, not a life.

## Contributing

Contributions are very welcome — particularly ports of individual `ActXxx()`
behaviours, which are self-contained, well-specified by the decompilation, and
the single biggest gap.

A few conventions worth knowing before you start:

- **Cite the source.** When porting behaviour, reference the file and line in
  `reference/cosmore/` that it came from, in a comment. That is what makes
  disagreements about "is this faithful?" answerable.
- **Prefer a faithful port to a plausible one.** If the original does something
  that looks like a bug, it probably shipped that way, and the goal is to match
  it. Note it in a comment rather than fixing it silently.
- **Verify empirically.** `cargo test --workspace` should stay green, and
  behavioural changes are best backed by a scripted headless run (below) or a
  unit test over the pure logic.
- Keep new presentation features behind the authentic/remaster split. Authentic
  mode is meant to stay pixel-exact.

Issues and pull requests both welcome. If you're picking up something large,
opening an issue first saves duplicated effort.

## Development

### Layout

- `crates/opencosmo-assets/` — the asset pipeline. Unwraps the makeself/mojosetup
  installer, parses Apogee's VOL/STN directory format, decodes EGA planar tile
  and sprite graphics, level maps and AdLib tracks, and writes PNG/JSON/WAV to
  a cache-stamped output directory. Each module's doc comment carries exact
  byte-format citations.
- `crates/opencosmo-game/` — the Bevy game. `build.rs` triggers conversion;
  `src/` has a module per subsystem (tileset, level, player, camera, actors,
  flow, hud, …).
- `reference/cosmore/` — vendored copy of the decompilation (MIT). The primary
  source for every format and physics decision here.
- `docs/file-formats.md` — technical reference for every file format and
  physics constant, with citations and an explicit list of what is confirmed
  versus still uncertain.

### Headless verification

The game can be scripted and traced, which is how most behaviour here gets
checked. `COSMO_INPUT` takes comma-separated `<keys><ticks>` steps, where keys
are `w`/`e` (west/east), `u`/`d` (look up/down), `j` (jump), `b` (bomb), `k`
(dismiss a text frame) and `.` (nothing):

```sh
COSMO_STATE=playing COSMO_LEVEL=a1 COSMO_TRACE=3 COSMO_QUIT_AFTER=120 \
  COSMO_INPUT="e8,k3,e30,.3,u30,.3,d40" cargo run -p opencosmo-game
```

That one checks that looking up and down pans the view: the `rel_row` column
is the player's row within the window, and it should move while `pos` stays
put.

| Variable | Effect |
| --- | --- |
| `COSMO_TRACE=<n>` | print position, facing, frame, scroll, cling and enemy count every nth tick |
| `COSMO_SHOT=<path>` | grab the window once, at `COSMO_SHOT_AT` (default tick 30) |
| `COSMO_SHOT_RAW=1` | grab the 320×200 virtual screen instead — use this for pixel-alignment questions, since it hasn't been through the present shader or the display scale factor |
| `COSMO_QUIT_AFTER=<ticks>` | end the run |
| `COSMO_FPS=1` | report frame pacing, with window size (without which two runs aren't comparable) |
| `COSMO_MOTION=1` | log the player's *drawn* position every frame — the only way to see whether interpolation is doing anything |
| `COSMO_LEVEL=<stem>` | starting level, e.g. `bonus1` |
| `COSMO_STATE=menu\|credits\|playing` | jump straight to a screen |
| `COSMO_SPAWN=x,y` | override the player's start tile |
| `COSMO_GIVE_BOMBS=n` | stock the bomb counter |
| `COSMO_HELP=1`, `COSMO_WARP=1` | open the help menu or level warp on frame one |
| `COSMO_AUTOPLAY=1` | drive the player automatically |

`RUST_LOG=opencosmo=debug` logs pounce, bomb and blast events.

The **level warp** (`F1` → `L`) jumps to any slot in the episode's progression.
It's a development aid with no counterpart in the shipped game.

## Licence and credits

OpenCosmo is [MIT licensed](LICENSE).

Game logic is ported from [Cosmore](https://github.com/smitelli/cosmore) by
Scott Smitelli and contributors, also MIT licensed; its notice is preserved at
`reference/cosmore/LICENSE`. This project would not exist without it.

*Cosmo's Cosmic Adventure* is copyright © 1992 Apogee Software, Ltd. This
project is not affiliated with, endorsed by, or supported by Apogee or
3D Realms, and distributes none of their data — only code that reads a copy
you already own.
