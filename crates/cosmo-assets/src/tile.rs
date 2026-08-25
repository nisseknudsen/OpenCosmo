//! Decodes raw EGA planar tile bytes (as stored in TILES.MNI / MASKTILE.MNI)
//! into 8x8 RGBA pixel buffers. Byte layout confirmed against cosmore's
//! `CopyTilesToEGA()`/`DrawSolidTile()`/`DrawMaskedTile()` (game1.c, C-DRAWING.md):
//! each tile is 8 rows, row-major, with the bitplane bytes interleaved within
//! each row (not grouped by plane across the whole tile).

use crate::palette::EGA_PALETTE;

pub const SOLID_TILE_BYTES: usize = 32;
pub const MASKED_TILE_BYTES: usize = 40;
pub const TILE_PIXELS: usize = 64; // 8x8

fn plane_pixel(planes: [u8; 4], bit: u8) -> u8 {
    ((planes[0] >> bit) & 1)
        | (((planes[1] >> bit) & 1) << 1)
        | (((planes[2] >> bit) & 1) << 2)
        | (((planes[3] >> bit) & 1) << 3)
}

/// `data` must be exactly 32 bytes: 8 rows x [plane0, plane1, plane2, plane3].
pub fn decode_solid_tile(data: &[u8]) -> [[u8; 4]; TILE_PIXELS] {
    assert_eq!(data.len(), SOLID_TILE_BYTES);
    let mut out = [[0u8; 4]; TILE_PIXELS];
    for row in 0..8usize {
        let base = row * 4;
        let planes = [data[base], data[base + 1], data[base + 2], data[base + 3]];
        for col in 0..8usize {
            let idx = plane_pixel(planes, 7 - col as u8);
            out[row * 8 + col] = EGA_PALETTE[idx as usize];
        }
    }
    out
}

/// `data` must be exactly 40 bytes: 8 rows x [and_mask, plane0, plane1, plane2, plane3].
/// Mask bit 1 = transparent, 0 = opaque (colored by the OR planes).
pub fn decode_masked_tile(data: &[u8]) -> [[u8; 4]; TILE_PIXELS] {
    assert_eq!(data.len(), MASKED_TILE_BYTES);
    let mut out = [[0u8; 4]; TILE_PIXELS];
    for row in 0..8usize {
        let base = row * 5;
        let mask = data[base];
        let planes = [
            data[base + 1],
            data[base + 2],
            data[base + 3],
            data[base + 4],
        ];
        for col in 0..8usize {
            let bit = 7 - col as u8;
            if (mask >> bit) & 1 == 1 {
                out[row * 8 + col] = [0, 0, 0, 0];
            } else {
                let idx = plane_pixel(planes, bit);
                out[row * 8 + col] = EGA_PALETTE[idx as usize];
            }
        }
    }
    out
}

pub fn decode_all_solid(tiles_mni: &[u8]) -> Vec<[[u8; 4]; TILE_PIXELS]> {
    tiles_mni
        .chunks_exact(SOLID_TILE_BYTES)
        .map(decode_solid_tile)
        .collect()
}

pub fn decode_all_masked(masktile_mni: &[u8]) -> Vec<[[u8; 4]; TILE_PIXELS]> {
    masktile_mni
        .chunks_exact(MASKED_TILE_BYTES)
        .map(decode_masked_tile)
        .collect()
}

/// Full-screen images (TITLE1.MNI, END1.MNI, BONUS.MNI, ...) are 320x200 raw
/// EGA planar bitmaps in **plane-major** order (confirmed against
/// `DrawFullscreenImage()`, game1.c:558-598): four contiguous blocks of
/// `(width/8)*height` bytes, one per bitplane, each a plain row-major
/// bitmap for that plane alone — NOT interleaved per-row like tiles are.
pub fn decode_fullscreen(data: &[u8], width: usize, height: usize) -> Vec<[u8; 4]> {
    let row_bytes = width / 8;
    let plane_size = row_bytes * height;
    assert_eq!(data.len(), plane_size * 4);
    let mut out = vec![[0u8; 4]; width * height];
    for row in 0..height {
        for byte_col in 0..row_bytes {
            let planes = [
                data[0 * plane_size + row * row_bytes + byte_col],
                data[1 * plane_size + row * row_bytes + byte_col],
                data[2 * plane_size + row * row_bytes + byte_col],
                data[3 * plane_size + row * row_bytes + byte_col],
            ];
            for bit_col in 0..8usize {
                let idx = plane_pixel(planes, 7 - bit_col as u8);
                let x = byte_col * 8 + bit_col;
                out[row * width + x] = EGA_PALETTE[idx as usize];
            }
        }
    }
    out
}
