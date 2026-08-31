# OpenCosmo

An open-source remake of Apogee's 1992 DOS platformer *Cosmo's Cosmic
Adventure*, written in Rust on the [Bevy](https://bevyengine.org) engine.

All three episodes are playable. Behaviour is ported from a decompilation of
the original game rather than guessed at, so the physics, camera and level
progression match — with an optional modern presentation layer on top that you
can switch off entirely.

> **This repository contains no game data.** It ships only code, which reads
> the art, levels, music and sound from a copy of the game *you* own, at build
> time, on your machine. You need your own copy to play.

## Requirements

- A **Rust toolchain** — install via [rustup](https://rustup.rs).
- **Your own copy of the game.** The build reads the GOG installer for
  *Cosmo's Cosmic Adventure* (a `.sh` file). It contains all three episodes,
  so nothing else is needed.
- A GPU/driver stack Bevy supports (Vulkan, Metal or DX12).
- On Linux, Bevy's system dependencies — `alsa-lib`, `libudev`, and X11 or
  Wayland development packages. See [Bevy's Linux setup notes](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

## Setup

```sh
git clone https://github.com/nisseknudsen/OpenCosmo.git
cd OpenCosmo
mkdir -p original
cp /path/to/your/cosmos_cosmic_adventure_installer.sh original/
```

## Run

```sh
cargo run --release
```

That's it. The first build converts the game's assets, which takes a minute;
after that the result is cached and startup is immediate.

To play a different episode:

```sh
COSMO_EPISODE=2 cargo run --release
```

<details>
<summary>If the installer lives somewhere else</summary>

```sh
COSMO_INSTALLER=/path/to/installer.sh cargo run --release
```
</details>

## Controls

| Action | Keys |
| --- | --- |
| Move | Arrow keys, or A/D |
| Jump | Ctrl or Space |
| Drop a bomb | Alt |
| Look around | Up/Down while standing still |
| Menu | F1 (restart, level warp, quit) |

Jump onto a creature to pounce it. Walk into a clingable wall while in the air
to grab it, then jump to climb.

Looking up and down is worth using: enemies and prizes only wake up once
they're on screen, so panning the view up at a tall structure is what makes its
bonuses drop down to you.

**Gamepads** work with no setup. Everything is rebindable from **G)ame
Redefine** on the main menu.

## Display modes

Press **F5** to switch.

| | Authentic | Remaster (default) |
| --- | --- | --- |
| Scaling | whole-number, letterboxed | fills the window |
| Artwork | untouched pixels | Scale3x smoothing |
| Screen | none | scanlines, bloom, vignette |
| Motion | original 18.2 Hz steps | interpolated, smooth |

The game itself plays identically either way — only the presentation changes.

Press **F6** for an experimental re-voiced soundtrack. It's off by default and
not very good yet.

## Status

**Under active development.** Playable start to finish, with plenty still
missing. Expect rough edges and shifting internals.

**Works:** movement, jumping, slopes, wall-clinging and collision; the
original's scroll camera; level progression including the bonus stages gated on
your star count; pouncing and bombs; scoring and pickups; the status bar, title
screen, menus and credits; hint globes; death replaying the level from scratch.

Per-actor behaviour is ported for 48 of the original's `ActXxx()` functions,
covering about 96% of the actors placed across the three episodes. The rest
fall back to a generic hazard/walker pass.

**Missing:** force fields, the pusher robot, the head switches that unlock
doors (doors themselves work, but stay locked), the scooter and transporter,
the boss, and the dizzy/ice-slide player states. A few ported behaviours are
missing a piece that needs player state the port does not have yet - the
rocket flies but cannot carry you, and the bear trap does not hold you.

There's no lives or game-over system because the original has none — dying
costs you the level's progress, not a life.

## Contributing

Contributions are very welcome. The most valuable and most approachable work is
porting individual actor behaviours: they're self-contained, precisely specified
by the decompilation, and the single biggest gap.

Behaviour here is ported from [Cosmore](https://github.com/smitelli/cosmore), a
decompilation of the original game, and the code cites it by file and line. To
read along, pull in the reference submodule:

```sh
git submodule update --init
```

It is not needed to build or play — only to follow the citations.

See **[docs/development.md](docs/development.md)** for the codebase layout,
conventions, and the headless testing tools. Opening an issue first saves
duplicated effort on anything large.

## Licence

OpenCosmo is [MIT licensed](LICENSE).

Game logic is ported from [Cosmore](https://github.com/smitelli/cosmore) by
Scott Smitelli and contributors, also MIT licensed. This project would not
exist without it.

*Cosmo's Cosmic Adventure* is copyright © 1992 Apogee Software, Ltd. This
project is not affiliated with, endorsed by, or supported by Apogee or
3D Realms, and distributes none of their data — only code that reads a copy you
already own.
