# Cosmo's Cosmic Adventure — File Format & Engine Reference

Compiled 2026-08-24 while researching how to reimplement the game. This is a
technical spec for parsing the original game's assets (which the project
owner owns a legitimate copy of, extracted from the GOG installer into
`extracted/game/data/noarch/data/COSMO{1,2,3}.{VOL,STN,EXE,CFG}`) and for
replicating its physics/behavior.

## Primary source: use `smitelli/cosmore`, don't reverse-engineer blind

**https://github.com/smitelli/cosmore** — a complete, buildable C
decompilation of `COSMO{1,2,3}.EXE` by Scott Smitelli, **MIT licensed**
(confirmed: `LICENSE` file, "Copyright (c) 2020-2024 Scott Smitelli and
contributors"). It does not include any game *data* — the user must supply
their own `COSMOx.VOL`/`COSMOx.STN`, exactly our situation.

This is by far the most authoritative source available: it's the actual game
logic, not a guess, with exact constants and formats preserved from the
original assembly/C. **Recommendation: treat this repo as the reference
implementation.** Port logic from it directly where useful (physics, actor
behavior, level/asset parsing) rather than re-deriving from scratch. Relevant
files fetched during this research (raw.githubusercontent.com/smitelli/cosmore/master/...):

- `src/def.h`, `src/player.h`, `src/actor.h`, `src/graphics.h`, `src/music.h`,
  `src/sound.h`, `src/glue.h` — constants, structs, actor/tile/sound tables
- `src/game1.c` (10,631 lines), `src/game2.c` — the actual game loop, level
  loader, physics, rendering, `GroupEntryFp()` (VOL/STN reader)
- `src/episode1.h` — Episode 1's level list and feature flags
- `src/lowlevel.asm`, `C-DRAWING.md` — low-level EGA drawing routines (the
  `C-DRAWING.md` doc gives a from-scratch **C reimplementation** of every
  assembly draw routine, which is what pinned down the exact tile pixel
  formats below)

No compression of any kind was found anywhere in the asset pipeline (grepped
for RLE/LZ/Huffman/Carmack — nothing). All tile, sprite, level, and music data
is stored **raw** inside the VOL/STN containers. This significantly
simplifies the asset pipeline versus other Apogee/id titles of the era.

## VOL / STN container format

Both `COSMOx.VOL` (graphics/levels) and `COSMOx.STN` (music) use the
identical directory format. Confirmed against both a hex dump of the real
file and `GroupEntryFp()` in `game2.c:1472`:

- Directory is a flat array of 20-byte entries starting at file offset 0:
  - bytes 0..11 (12 bytes): entry name, uppercase ASCII, null-padded (e.g.
    `"TITLE1.MNI\0\0"`). The original lookup only compares the first 11
    bytes (a documented quirk/bug — "Fun ensues" per the source comment) —
    safe to just match the full null-padded 12-byte name when writing a
    clean implementation.
  - bytes 12..15 (4 bytes): LE `u32` data offset (absolute, from start of file)
  - bytes 16..19 (4 bytes): LE `u32` data length in bytes
- The directory ends at the first entry whose name byte 0 is `\0`. (The
  original code only scans the first 980 bytes / 49 entries of the header —
  irrelevant for a reimplementation; just scan until a null name or EOF.)
- Entry data is stored verbatim (no compression) at the given offset/length.

This matches the hex dump taken from the real `COSMO1.VOL`:
`54 49 54 4c 45 31 2e 4d 4e 49 00 00 | a0 0f 00 00 | 00 7d 00 00` =
`"TITLE1.MNI"`, offset `0x0fa0` (4000), length `0x00007d00` (32000).

Implementation note: reading is straightforward — scan the directory once,
build a `HashMap<String, (offset, len)>`, then slice/seek per entry. No need
to replicate the original's on-demand linear-scan-per-lookup or its STN→VOL→CWD
fallback chain (that fallback exists only because the original supported
loose loose files during development).

## Tile graphics format

Tile size is fixed at **8×8 pixels**, native resolution **320×200** (EGA/VGA,
4-bit color / 16-color palette). Two tile "kinds" share one numbering scheme:

- Map tile values `0x0000..0x3E78` (in steps of 8) = **solid tiles** (opaque,
  no transparency). `TILE_EMPTY = 0x0000` is "air". First ~2000 solid tile
  slots.
- Map tile values `>= 0x3E80` (`TILE_MASKED_0`, aka index 16000) = **masked
  tiles** (support transparency, used for the busier decorative/overlay
  tiles). Same numbering step of 8 per tile.
- A map cell's raw `u16` value is `tile_index * 8`; divide by 8 to get a
  plain tile index, and check against the masked threshold (2000, i.e. raw
  value 16000) to pick the decode path.

**Solid tile pixel data** (source: `TILES.MNI`, loaded via
`LoadGroupEntryData("TILES.MNI", ..., 64000)` in `game1.c:8277`):
- 64000 bytes total ÷ 2000 tiles = **32 bytes per tile**.
- Standard EGA planar format: 4 bitplanes (one bit per pixel = the palette
  index's 4 bits), 8 rows × 1 byte/row per plane = 8 bytes/plane × 4 planes =
  32 bytes/tile. Bit 7 = leftmost pixel of the row (MSB-first), each of the 4
  planes contributes one bit to that pixel's 4-bit palette index.
  (`DrawSolidTile` in `C-DRAWING.md` uses an EGA latch-copy trick to blit
  these — that trick is a DOS/VGA-hardware optimization only; a modern
  renderer should just decode the 4 planes into an 8×8 RGBA texture directly
  using the game's 16-color EGA palette and ignore the latch mechanism
  entirely.)

**Masked tile / sprite pixel data** (confirmed exactly via the C
reimplementation of `DrawMaskedTile`/`DrawSpriteTile` in `C-DRAWING.md`):
- Row stride is **5 bytes**: byte 0 = 8-bit **AND mask** (1 = keep
  background/transparent, 0 = draw pixel — inverted-looking but confirmed by
  `*localdst = (*localdst & *localsrc) | ...`), bytes 1..4 = the 4 **OR**
  color planes (1 bit/pixel each, same MSB-first bit order as solid tiles).
- 8 rows × 5 bytes/row = **40 bytes per 8×8 masked tile**.
- To decode to RGBA: for each pixel, if the AND-mask bit is 0 the pixel is
  opaque with the palette index built from the 4 OR-plane bits; if the mask
  bit is 1, the pixel is transparent (alpha 0).

**Actor/sprite graphics** (source: chunks like `A1.MNI`..`A16.MNI`, loaded
into `actorTileData[]`/described via `actorInfoData`/`ACTRINFO.MNI`,
`PLYRINFO.MNI`, `CARTINFO.MNI` — see `LoadInfoData()` in `game1.c:688` and
usage around `game1.c:919-950`): each sprite frame's metadata is a 4-word
(8-byte) record: `{height, width, data_offset, plane_index}` (the info table
is indexed as `actorInfoData[sprite_type] -> offset; offset + frame*4` gives
this 4-word record — see `game1.c:919-922`). Frame pixel data uses the same
5-bytes/row AND+OR×4 masked format as tiles, just with `width`×`height`
measured in tiles (most actors are larger than 8×8, spanning multiple tile
cells). **Not fully traced byte-for-byte in this pass** — confirming the
precise multi-tile frame layout inside `A*.MNI` chunks (and the exact
`ACTRINFO.MNI`/`PLYRINFO.MNI`/`CARTINFO.MNI` header layout) needs an
empirical pass: read `game1.c` around lines 600-950 in full, or just write a
small probe that decodes `A1.MNI` with the above assumptions and visually
diffs against a DOSBox screenshot.

**Palette**: standard 16-color EGA palette, set via `SetPaletteRegister`
(`C-DRAWING.md`). The specific 16 RGB values used by this game were not
pulled in this pass — either find them in `game1.c`'s palette-setup code
(search for `SetPaletteRegister` call sites) or use the well-known standard
EGA 64-color-cube 16-color default palette as a starting point and verify
against a DOSBox screenshot. **Unconfirmed — verify before finalizing art.**

## Level / map format

Source: `LoadMapData()`, `game1.c:10264-10333`. Level files (`A1.MNI`,
`A2.MNI`, ..., `bonus1.mni`, `bonus2.mni`, etc., looked up by name through the
VOL directory) have this exact layout:

```
u16le  map_flags       // skipped/unused by loader (getw(fp) discarded)
u16le  map_width       // tile columns; MUST be a power of two: 32,64,128,256,512,1024,2048
u16le  actor_word_count// length of the actor table that follows, in u16 words (= 3 * actor_count)
[actor_word_count/3 records]  each record = 3x u16le: { type, x, y }
[remaining bytes to EOF]      tile map data: u16le per cell, row-major,
                               `map_width` cells per row; height = remaining_words / map_width
```

- `map_width` is stored as a literal power of two because the original engine
  computes row addressing via a bit-shift (`mapYPower`, derived by a switch
  on `map_width` in `LoadMapData`) instead of a multiply — irrelevant for a
  reimplementation using normal indexing, but confirms map width is always
  one of {32,64,128,256,512,1024,2048}.
  columns.
  Map height is **not stored explicitly** — it's implied by how much tile
  data follows the actor table (i.e. `(file_length - actor_table_bytes -
  6_byte_header) / 2 / map_width`).
- Each tile map cell is a raw `u16` in the same encoding as described above
  (`raw_value = tile_index * 8`, threshold 16000 = masked vs. solid).
- Actor records: `type` is a map-actor-type ID (see Actors section below),
  `x`/`y` are tile coordinates. Type `0` = `SPA_PLAYER_START` (player spawn
  point), types `1..8` are "special" actors (moving platforms, particle
  fountains, colored light sources — not regular game objects), types `>= 31`
  map to the `ACT_*` table (real game actors/enemies/items/hazards) via
  `type - 31`? — **the exact offset subtraction wasn't traced in this pass**;
  `actor.h` states "map actor type 0" for `SPA_PLAYER_START` and "map actor
  type 31" for `ACT_BASKET_NULL`, so the mapping is very likely
  `map_type == 0..8 => SPA_*`, `map_type >= 31 => ACT_(map_type - 31)`, with
  `9..30` reserved/unused. Verify against `NewMapActorAtIndex()` in
  `game1.c` before relying on it.

### Tile attribute bits (collision)

Source: `game1.c:247-254`. A separate table (`tileAttributeData`, loaded from
a group entry — likely `MASKED.MNI`/an attributes chunk, name not confirmed
in this pass) holds one **byte per 8 tiles** (bit-packed, `tileAttributeData[tile/8]`,
so 1 bit per tile — meaning tile index must additionally be reduced mod 8 to
pick the bit... **the exact bit-selection math wasn't fully traced**; the
macros only show the byte lookup, e.g. `TILE_BLOCK_SOUTH(val) = *(tileAttributeData + val/8) & 0x01`
— this looks like it's actually indexing per raw-tile-index/8 giving a
**shared** attribute byte per group of 8 tiles with fixed bit meanings below,
not "1 bit per tile"; re-derive precisely from the source before implementing.)
Known bit meanings (each is a flag on whatever byte the tile's `value/8`
resolves to):

| bit  | meaning                                    |
|------|---------------------------------------------|
| 0x01 | blocks southward movement (floor)           |
| 0x02 | blocks northward movement (ceiling)          |
| 0x04 | blocks westward movement (wall)              |
| 0x08 | blocks eastward movement (wall)              |
| 0x10 | slippery                                     |
| 0x20 | drawn in front of the player/actors          |
| 0x40 | sloped (ramp)                                |
| 0x80 | can be clung to (wall-climb)                 |

### Episode 1 level list

Source: `episode1.h`. 30 map-file entries covering 16 "world" stages (A1
through A16) interleaved with repeating bonus stages:

```
A1  A2  bonus1 bonus2
A3  A4  bonus1 bonus2
A5  A6  bonus1 bonus2
A7  A8  bonus1 bonus2
A9  A10 bonus1 bonus2
A11 A12 bonus1 bonus2
A13 A14 bonus1 bonus2
A15 A16
```

Other Episode 1 chunks: `TITLE1.MNI` (title screen), `END1.MNI` (ending
screen), `COSMO1.MNI` (exit/order-info text). `#define SHAREWARE` and
`#define HAS_MAP_11` are episode-1-specific build flags (Episode 1 is the
shareware episode — this matches the extracted GOG installer, which only
contains `COSMO1.*` as the free episode plus `COSMO2/3.*` for the full game
we also have access to since the user purchased the full GOG release).
Episode 2/3 level lists are presumably in `episode2.h`/`episode3.h` (not
fetched in this pass — same pattern expected).

## Actor types

Source: `actor.h` (301 lines, MIT licensed, copied in full below for
reference — this is the complete enum of every map/game actor type). Two ID
spaces:

- **`SPA_*`** (map types 0..8): `PLAYER_START`, `PLATFORM`, 4 fountain sizes,
  3 light-source variants (west/middle/east).
- **`ACT_*`** (map types 31+): ~250 actual game actors — every enemy,
  collectible fruit/gem + its spawn "barrel"/"basket" container, hazard,
  switch, door, decoration, and effect in the game. Numeric IDs are sparse
  (gaps exist — not every value 0..296 is used).

Array size limits worth preserving in a port: `MAX_ACTORS 410`,
`MAX_DECORATIONS 10`, `MAX_EXPLOSIONS 7`, `MAX_FOUNTAINS 10`, `MAX_LIGHTS
200`, `MAX_PLATFORMS 10`, `MAX_SHARDS 16`, `MAX_SPAWNERS 6` — these were
fixed-size arrays in the original (DOS memory constraints); a Rust/Bevy port
has no reason to cap these, but they're useful as a sanity check on
level/actor density.

Full `actor.h` is small enough to just fetch directly when implementing:
`https://raw.githubusercontent.com/smitelli/cosmore/master/src/actor.h`
(MIT licensed, safe to vendor a copy or transcribe the `ACT_*`/`SPA_*` table
directly into a Rust enum).

## Player physics

Source: `MovePlayer()`, `game1.c:8438-8822` ("This is the hairiest function
in the entire game" — direct quote from the source comment). This is **not**
continuous-acceleration physics — it's a classic tile-grid, frame-table-driven
system:

- Movement is in **whole tile-grid steps per game tick**, not sub-pixel.
  `playerX`/`playerY` are tile coordinates; horizontal walking moves exactly
  ±1 tile per tick (`playerX++`/`playerX--`, confirmed at multiple call
  sites, e.g. `game1.c:8571,8589` etc.) — i.e. **no run/walk speed
  distinction was found**; movement rate is presumably gated by the game's
  fixed tick rate rather than a variable per-frame speed value.
- **Jump curve**: a fixed lookup table of per-tick vertical deltas, applied
  while `playerJumpTime` (an index) increments each tick the jump button is
  held (up to a cap):
  ```c
  static int jumptable[] = {-2, -1, -1, -1, -1, -1, -1, 0, 0, 0};
  // playerY += jumptable[playerJumpTime];  (playerJumpTime = 0..9)
  ```
  Jump is force-terminated (`isPlayerFalling = true`) once `playerJumpTime`
  exceeds 6 (`game1.c:8768`) even if the button is still held, or immediately
  if the jump button is released (`cmdJumpLatch`), or if the head hits a
  ceiling (`TestPlayerMove(DIR4_NORTH, ...)` blocked).
- **Falling**: `playerY++` once per tick while falling
  (`game1.c:8785`); after `playerFallTime > 3` ticks of continuous falling, an
  **extra** `playerY++` is applied per tick (`game1.c:8800-8802`) — i.e. fall
  speed is 1 tile/tick for the first ~3 ticks then steps up to 2 tiles/tick
  (a crude "terminal velocity" ramp), capped by `playerFallTime` saturating at
  25 (`game1.c:8817`, doesn't affect speed further, just used elsewhere e.g.
  to pick the "long fall" sprite/sound).
- **Recoil/bounce** (`playerRecoilLeft`, used for the pogo-stick-style bounce
  and hit-recoil): a separate counter-driven vertical push, `playerY--` while
  `playerRecoilLeft > 1`, with an extra decrement+move at `playerRecoilLeft >
  13` (`game1.c:8698-8721`). Exact recoil trigger conditions (e.g. pogo bounce
  off enemies vs. taking damage) weren't traced in this pass — read
  `game1.c` around the `isPlayerRecoiling`/`playerRecoilLeft` assignments
  (several call sites earlier in the file, not yet read) to nail this down.
- **Cling** (`playerClingDir`): wall-cling state gated by tile attribute bit
  `0x80` (`TILE_CAN_CLING`) and, for slippery-wall variants, bit `0x10`
  (`TILE_SLIPPERY`) causing a slow slide (`clingslip`, `game1.c:8465-8495`).
- Other player state not detailed here but present in the source and worth
  porting directly rather than re-guessing: bomb placement/cooldown
  (`game1.c:8497-8544`), push/knockback (`MovePlayerPush`), dizzy state,
  scooter mount, transporter use — all gate `MovePlayer()` at the top via
  early-return guards (`game1.c:8451-8454`).

**Recommendation**: because this whole function is one contiguous ~380-line
block with a lot of interacting edge cases (the source's own comment warns
about this), the highest-fidelity path is a close line-by-line port of
`MovePlayer()` into Rust/Bevy's fixed-timestep update, rather than
re-deriving "equivalent" continuous physics. A tile-stepped movement model
translates naturally to an ECS fixed-timestep system operating in tile units
(then interpolating for smooth rendering between ticks if desired).

## Screen / viewport geometry

- Native resolution: **320×200**, EGA/VGA, 16-color palette.
- Tile size: **8×8 pixels** → 40×25 tiles fit the raw screen.
- Visible scrolling game window: **`SCROLLW=38` × `SCROLLH=18`** tiles
  (`def.h:138-139`) — i.e. the full 40-wide screen minus a 1-tile border on
  left/right, and 25 rows minus 7 rows for the status bar + border (status
  bar is drawn separately, occupying the remaining rows at a fixed screen
  location — exact status bar row count not traced, but 18 visible rows out
  of 25 leaves 7 for status bar + top border).
- Backdrop (background) layer scrolls at **half rate** (4px steps vs. the
  foreground's 8px/1-tile steps) for a parallax effect, implemented in the
  original via 4 pre-shifted copies of each backdrop image
  (`game1.c:696-800`, detailed walkthrough in the source comments). This is a
  DOS-hardware-latch optimization; a modern renderer gets the same visual
  parallax effect trivially by rendering the backdrop layer at half the
  camera's scroll delta — no need to replicate the pre-shifted-copies trick.
- Backdrops are **40×18 tiles**, stored uncompressed, 32 bytes/tile (same
  planar format as solid tiles): `BACKDROP_SIZE = 23040` = `40*18*32`
  (`graphics.h:24-27`).

## Audio

Two **separate** systems, confirmed via `sound.h`/`music.h`:

- **Sound effects** (`SND_*`, 65 defined in `sound.h`) are **PC speaker**
  effects — simple tone/duration sequences, not AdLib. Format/data location
  not traced in this pass (look for the PC speaker driver in `lowlevel.asm`
  and wherever `SND_*` data chunks are loaded from the VOL/STN).
- **Music** (`MUSIC_*`, 19 tracks defined in `music.h`, e.g. `MUSIC_CAVES`,
  `MUSIC_BOSS`, `MUSIC_ROCKIT`) is **AdLib (OPL2)**, loaded via
  `LoadMusicData()` (`game2.c:1561-1577`) into a:
  ```c
  typedef struct { word length; word datahead; } Music;
  ```
  i.e. a 2-byte length prefix followed immediately by the raw AdLib command
  stream (`fread(&dest->datahead, 1, lastGroupEntryLength + 2, fp)`), read
  straight from a VOL/STN group entry named via `musicNames[music_num]`
  (array not fetched in this pass — likely follows the same
  `"xxxxxxxx.MNI"` naming convention as everything else, stored inside the
  `.STN` file specifically based on `GroupEntryFp`'s STN-first search order).
  This 2-byte-length + raw-stream shape is consistent with the well-known
  **IMF (id Music Format)** used across many Apogee/id titles of this era: a
  stream of 4-byte records `{register, value, delay_lo, delay_hi}` (16-bit LE
  delay, in some fixed tick unit) directly driving the OPL2's registers.
  **Not fully confirmed** — the exact record layout and playback tick
  rate/interrupt frequency need verification, either by reading the AdLib
  service routine in `lowlevel.asm`/`game1.c` (search for the timer
  interrupt handler / `ServiceAdLib`-equivalent) or empirically by feeding a
  candidate parse into an OPL2 emulator (e.g. the `opl` Rust crate) and
  listening for correctness against a DOSBox reference. If it does turn out
  to be standard IMF, existing IMF-to-WAV tools/crates can shortcut a lot of
  this.

## Summary of what's solid vs. unconfirmed

**Solid / directly sourced from `cosmore`:**
VOL/STN directory format; no-compression confirmation; solid tile format (32
B/tile, 4-plane EGA planar); masked/sprite tile format (40 B/tile, AND mask +
4 OR planes, 5 B/row); level file header + actor table + tile map layout;
tile collision attribute bit meanings; jump curve table and fall-speed
ramp-up; tile-grid (not sub-pixel) movement model; screen/tile geometry;
Episode 1's 30-map level list; the full `ACT_*`/`SPA_*` actor type enum;
sound effects are PC-speaker, music is AdLib, with the `Music` struct's
2-byte length + raw-stream shape.

**Resolved since this document was first written.** Everything on the
original open-questions list has now been settled by reading further into
`cosmore` and checking the result against the real data files:

1. **Sprite frame layout** — a 4-word record per frame `{height, width,
   data_offset, bank}`, with pixel data as `width*height` consecutive
   40-byte masked tiles in row-major order. Frame counts are bounded by the
   next sprite's base offset in the info table. See
   `crates/opencosmo-assets/src/sprite.rs`.
2. **EGA palette** — the game only ever reprograms register 5 (as an
   animation key colour); every other register stays at the BIOS default,
   so the stock 16-colour table is correct. See `palette.rs`.
3. **Tile attribute math** — one uniform lookup, `tileAttributeData[raw / 8]`,
   for both solid and masked values (game1.c:247-254). Note this is a
   *different* indexing scheme from the masked-tile graphic lookup, which
   is `(raw - 16000) / 40`; conflating the two produced a long-lived
   rendering bug.
4. **Map actor type offset** — confirmed `ACT_* = map_type - 31` from
   `NewMapActorAtIndex()` (game1.c:10252). `ACT_*` and `SPR_*` are separate
   numbering spaces; the mapping between them is in
   `crates/opencosmo-assets/src/actor_sprite_map.rs`, extracted from every
   `ConstructActor` call.
5. **Recoil / pounce triggers** — `TryPounce` (game1.c:6844-6895) plus the
   per-sprite `TryPounce(...)` values in the switch at game1.c:7094+.
6. **AdLib music** — headerless IMF: 4-byte `{register, value, delay_le16}`
   records at a 560 Hz tick (the Apogee rate, not id's 700 Hz). See
   `music.rs`.
7. **PC-speaker sound effects** — each bank is a 16-byte header followed by
   16-byte records `{offset_u16, priority_u8, unknown_u8, name[12]}`;
   sample streams are flat `u16` PIT divisors terminated by `0xFFFF`, with
   `0` meaning silence and pitch `1193182 / divisor`. Serviced at 140 Hz on
   both timer paths. Only 23 of each bank's 24 records are loaded — the
   original's loop stops one short of its own table. See `sound.rs`.
8. **Episode 2/3 level lists** — in `episode2.h`/`episode3.h`, same shape as
   Episode 1.

Two things remain genuinely unexplained rather than merely unimplemented:
the `0x08` byte following each sound-effect priority is never read by the
original, and the fixed-size actor arrays (`MAX_ACTORS` and friends) are DOS
memory constraints with no behavioural meaning worth reproducing.
