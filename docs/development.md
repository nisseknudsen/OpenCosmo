# Development

Notes for working on OpenCosmo. For setup and play instructions, see the
[README](../README.md).

## Layout

- `crates/opencosmo-assets/` — the asset pipeline. Unwraps the makeself/mojosetup
  installer, parses Apogee's VOL/STN directory format, decodes EGA planar tile
  and sprite graphics, level maps and AdLib tracks, and writes PNG/JSON/WAV to
  a cache-stamped output directory. Each module's doc comment carries exact
  byte-format citations.
- `crates/opencosmo-game/` — the Bevy game. `build.rs` triggers conversion;
  `src/` has a module per subsystem (tileset, level, player, camera, actors,
  flow, hud, …).
- `reference/cosmore/` — a **git submodule** pointing at
  [Cosmore](https://github.com/smitelli/cosmore) (MIT), a decompilation of the
  original executables. This is the primary source for every format and physics
  decision here. Nothing builds from it, so it is optional — but you want it
  checked out if you are porting behaviour (see below).
- `docs/file-formats.md` — every file format and physics constant, with source
  citations and an explicit list of what is confirmed versus still uncertain.

## The Cosmore reference

Around 280 comments in this codebase cite Cosmore by file and line — for
example `game1.c:9968` for `NextLevel()`. Those line numbers are only
meaningful against one exact revision, so Cosmore is pinned as a submodule
rather than linked loosely. Check it out with:

```sh
git submodule update --init
```

The pin is upstream commit `80418d1`. The game builds and runs perfectly well
without it; you only need it to read the source a citation points at, or to
add new citations. If you bump the pin, re-verify the citations — a line-number
shift upstream invalidates them silently.

## How the port works

Game logic runs on an **18.2 Hz fixed tick**, because that is the rate the DOS
timer interrupted at, and every constant in the original is expressed per tick:
the jump curve is a table of ten per-tick offsets, walking is one tile per tick,
gravity is a per-tick counter. Raising the tick rate would not make the game
smoother — it would make Cosmo run three times faster. Rendering is decoupled
from the tick instead (see *Smooth motion* below).

The camera is the original's stateful `scrollX`/`scrollY` pair with its dead
zone, not a camera centred on the player. This matters for more than feel:
actors only run while on screen, so panning the view up to a stack of prizes is
what wakes them — which is the mechanic behind looking up.

Everything draws into a single 320×200 buffer at the original's exact
resolution, which is scaled to the window exactly once. That is also what let
the layout match the original screen precisely, including the 8px black border
around the play area.

## Presentation internals

**Scale3x** was chosen over smoother upscalers (xBRZ, HQx, neural) for one
property: it only ever *copies* a neighbouring pixel, never blends two, so the
16-colour EGA palette survives exactly. It also leaves interior dithering
alone, which matters because this game's backdrops run to 27% dither by pixel
count, and a blending upscaler turns that into blobs.

**Smooth motion** spreads a tick's worth of movement across the frames that
fall inside it. Measured: 176 distinct drawn positions across a walk instead of
53, in 1–3 pixel steps rather than 8-pixel jumps — still landing on whole
pixels, so the pixel grid (and Scale3x) is undisturbed.

Individual effects can be dialled independently of the F5 preset:

| Variable | Effect |
| --- | --- |
| `COSMO_PRESENT=authentic\|remaster` | pick the mode at launch |
| `COSMO_SCANLINE=0.3` | scanline strength |
| `COSMO_BLOOM`, `COSMO_VIGNETTE`, `COSMO_CURVE` | CRT pass components |
| `COSMO_SMOOTH` | Scale3x smoothing |
| `COSMO_SMOOTH_MOTION=on\|off` | motion interpolation |
| `COSMO_VSYNC=off` | uncap the frame rate |
| `COSMO_WINDOW=1280x800` | window size |

## Audio internals

The original AdLib soundtrack is IMF decoded to OPL2 synthesis, rendered to
looping WAV, and matched to each level by the real per-level track assignment
packed into the level header's flags word. PC-speaker effects run with the
original's monophonic priority behaviour, where a quieter effect is dropped
outright rather than mixed.

The experimental re-voiced soundtrack (**F6**, off by default,
`COSMO_AUDIO=remaster`) decodes the IMF register stream back into notes —
pitch, timing, length and velocity are all recoverable from documented hardware
registers — and re-renders them with warm additive voices, tape wobble and a
low-pass. The composition is untouched; only the instruments change. It is not
good enough yet and the approach is being reconsidered.

## Actor behaviours

Each actor type's behaviour is a `tick_*` function in `enemy_ai.rs`, ported
from the matching `ActXxx()` in `game1.c` and cited by line. They are
deliberately **pure functions over `Enemy` plus the level** - no `Commands`,
no queries, no Bevy types - which is what lets all of them be unit tested
without standing up an app, a window or an audio device. `cargo test` is
silent and takes under a tenth of a second.

Four things a behaviour cannot do directly, and queues instead:

- **Spawning another actor** (`NewActor` in the original) - push to
  `Enemy::spawns` as `(ACT_* id, x, y)`. A turret's projectile, a hatching
  egg's ghost.
- **Writing to the map** (`SetMapTile`) - push to `Enemy::tile_writes` as
  `(x, y, raw tile)`. A door making itself solid, a platform dropping out
  from under the player.

- **Shoving the player** — set `Enemy::push_player`. The pusher robot.
- **Holding the player still** — set `Enemy::hold_player`. The bear trap.

`spawn_queued_actors` drains them after the ticks, so anything created this
tick first acts on the next one, as in the original.

Three behaviours are systems rather than ticks, because they are not about one
actor: `run_transporters` (a transporter's destination is a different entity),
`draw_force_field_beams` (a force field and a beam robot are partly a line
rather than a body), and `finish_on_boss_defeat`.

Level-wide state the switches govern lives in `SwitchState`; every flag starts
on, and the presence of the switch that governs one turns it off, which is what
the original does at construction time.

To add one: find the `ActXxx()` in `reference/cosmore/src/game1.c`, find the
`ConstructActor` call that installs it (which gives you the `data1..data5`
seeds), add an `EnemyKind` variant, a `tick_*` function, a row in
`ENEMY_TABLE`, a dispatch arm, and a test. Note anything you deliberately
left out with a `NOT PORTED:` line in the doc comment.

## Headless verification

The game can be scripted and traced, which is how most behaviour here gets
checked. `COSMO_INPUT` takes comma-separated `<keys><ticks>` steps, where keys
are `w`/`e` (west/east), `u`/`d` (look up/down), `j` (jump), `b` (bomb), `k`
(dismiss a text frame) and `.` (nothing):

```sh
COSMO_STATE=playing COSMO_LEVEL=a1 COSMO_TRACE=3 COSMO_QUIT_AFTER=120 \
  COSMO_INPUT="e8,k3,e30,.3,u30,.3,d40" cargo run -p opencosmo-game
```

That one checks that looking up and down pans the view: the `rel_row` column is
the player's row within the window, and it should move while `pos` stays put.

| Variable | Effect |
| --- | --- |
| `COSMO_STATE=menu\|credits\|playing` | jump straight to a screen |
| `COSMO_LEVEL=<stem>` | starting level, e.g. `a1`, `bonus1` |
| `COSMO_EPISODE=1\|2\|3` | episode to load |
| `COSMO_SPAWN=x,y` | override the player's start tile |
| `COSMO_GIVE_BOMBS=n` | stock the bomb counter |
| `COSMO_TRACE=<n>` | print position, facing, frame, scroll, cling and enemy count every nth tick |
| `COSMO_MOTION=1` | log the player's *drawn* position every frame — the only way to see whether interpolation is doing anything |
| `COSMO_SHOT=<path>` | grab the window once, at `COSMO_SHOT_AT` (default tick 30) |
| `COSMO_SHOT_RAW=1` | grab the 320×200 virtual screen instead — use this for pixel-alignment questions, since it has not been through the present shader or the display scale factor |
| `COSMO_FPS=1` | report frame pacing, with window size (without which two runs are not comparable) |
| `COSMO_QUIT_AFTER=<ticks>` | end the run |
| `COSMO_HELP=1`, `COSMO_WARP=1` | open the help menu or level warp on frame one |
| `COSMO_AUTOPLAY=1` | drive the player automatically |
| `COSMO_INSTALLER=<path>` | installer to convert assets from |

`RUST_LOG=opencosmo=debug` logs pounce, bomb and blast events.

The **level warp** (`F1` → `L`) jumps to any slot in the episode's progression.
It is a development aid with no counterpart in the shipped game.

## Conventions

- **Cite the source.** When porting behaviour, reference the file and line in
  `reference/cosmore/` it came from, in a comment. That is what makes
  disagreements about "is this faithful?" answerable.
- **Prefer a faithful port to a plausible one.** If the original does something
  that looks like a bug, it probably shipped that way, and the goal is to match
  it. Note it in a comment rather than fixing it silently.
- **Verify empirically.** `cargo test --workspace` should stay green, and
  behavioural changes are best backed by a scripted headless run or a unit test
  over the pure logic.
- **Keep new presentation features behind the authentic/remaster split.**
  Authentic mode is meant to stay pixel-exact.
