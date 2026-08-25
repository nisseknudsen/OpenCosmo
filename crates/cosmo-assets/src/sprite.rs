//! Actor/player sprite frame decoding. Confirmed against `DrawSprite()`
//! (game1.c:1210-1264) and `LoadActorTileData()` (game1.c:650-659):
//!
//! - An "info" table (`ACTRINFO.MNI`/`PLYRINFO.MNI`, loaded as a raw `u16`
//!   array) holds, per sprite type, a base *word index* into the same array;
//!   `base + frame*4` locates a 4-word record: `{height, width, data_offset,
//!   bank}`, where `height`/`width` are in whole 8x8 tiles and `data_offset`
//!   is a *byte* offset into the corresponding pixel-data buffer.
//! - Pixel data for one frame is `width*height` consecutive 40-byte masked
//!   tiles (same format as `MASKTILE.MNI`), in row-major order (confirmed by
//!   `src += 40` walking column-by-column then row-by-row in `DrawSprite`).
//! - `ACTORS.MNI` is split into 3 buffers ("banks") of at most `WORD_MAX`
//!   (0xffff) bytes purely as a 16-bit-DOS-segment workaround; `bank` in the
//!   4-word record picks which one. `PLAYERS.MNI` fits in a single buffer
//!   (no banking) and its info records leave `bank` unused.

use crate::tile::{decode_masked_tile, TILE_PIXELS};

pub const WORD_MAX: usize = 0xffff;

/// Splits a raw ACTORS.MNI-style blob into the 3 fixed-size banks the
/// original engine loaded it into.
pub fn split_actor_banks(data: &[u8]) -> [&[u8]; 3] {
    let b0_end = WORD_MAX.min(data.len());
    let b1_end = (2 * WORD_MAX).min(data.len());
    [&data[..b0_end], &data[b0_end..b1_end], &data[b1_end..]]
}

#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub height_tiles: u16,
    pub width_tiles: u16,
    pub data_offset: u16,
    pub bank: u16,
}

/// `info` is the raw ACTRINFO.MNI/PLYRINFO.MNI word array. `type_index` is
/// the sprite type (word index whose value is the frame table's base word
/// offset); pass 0 for the player, which has no per-type indirection.
pub fn frame_info(info: &[u16], type_index: usize, frame: usize) -> FrameInfo {
    let base = info[type_index] as usize;
    let off = base + frame * 4;
    FrameInfo {
        height_tiles: info[off],
        width_tiles: info[off + 1],
        data_offset: info[off + 2],
        bank: info[off + 3],
    }
}

/// The info table's layout is: a header of one base-word-offset per sprite
/// type, followed by each type's run of 4-word frame records back to back.
/// There's no explicit per-type frame count anywhere, but every type's own
/// base offset is itself a value that appears in the table — so the
/// smallest other base value greater than this type's base is a safe upper
/// bound on how many 4-word records belong to it (a false-positive
/// "boundary" only makes the bound tighter, never lets it run over into the
/// next type's pixel data).
pub fn max_frames_for_type(info: &[u16], type_index: usize) -> usize {
    let base = info[type_index] as usize;
    let mut next = info.len();
    for (i, &v) in info.iter().enumerate() {
        let v = v as usize;
        if i != type_index && v > base && v < next {
            next = v;
        }
    }
    (next.saturating_sub(base)) / 4
}

/// Decodes one frame's tiles, row-major, `width_tiles * height_tiles` of them.
pub fn decode_frame_tiles(tile_bank: &[u8], info: &FrameInfo) -> Vec<[[u8; 4]; TILE_PIXELS]> {
    let count = info.width_tiles as usize * info.height_tiles as usize;
    let mut out = Vec::with_capacity(count);
    let mut off = info.data_offset as usize;
    for _ in 0..count {
        out.push(decode_masked_tile(&tile_bank[off..off + 40]));
        off += 40;
    }
    out
}

/// Composites a frame's tiles into one `width*8 x height*8` RGBA image.
pub fn composite_frame(info: &FrameInfo, tiles: &[[[u8; 4]; TILE_PIXELS]]) -> (u32, u32, Vec<[u8; 4]>) {
    let w = info.width_tiles as usize * 8;
    let h = info.height_tiles as usize * 8;
    let mut out = vec![[0u8; 4]; w * h];
    for ty in 0..info.height_tiles as usize {
        for tx in 0..info.width_tiles as usize {
            let tile = &tiles[ty * info.width_tiles as usize + tx];
            for i in 0..64 {
                let px = tx * 8 + i % 8;
                let py = ty * 8 + i / 8;
                out[py * w + px] = tile[i];
            }
        }
    }
    (w as u32, h as u32, out)
}
