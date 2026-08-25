//! Level/map file parser. Format confirmed against cosmore's `LoadMapData()`
//! (game1.c:10264-10333): a 6-byte header, an actor spawn table, then a
//! raw row-major u16 tile grid filling the rest of the file.

use anyhow::{ensure, Result};

pub const MASKED_TILE_THRESHOLD: u16 = 16000; // TILE_MASKED_0

#[derive(Debug, Clone, Copy)]
pub struct MapActor {
    pub map_type: u16,
    pub x: u16,
    pub y: u16,
}

pub struct Level {
    pub width: usize,
    pub height: usize,
    pub actors: Vec<MapActor>,
    /// Row-major, `width * height` raw cell values. Divide by 8 to get a
    /// plain tile index; a raw value >= MASKED_TILE_THRESHOLD is a masked
    /// tile, indexed as `(raw - MASKED_TILE_THRESHOLD) / 8`.
    pub tiles: Vec<u16>,
}

fn read_u16le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

pub fn parse(bytes: &[u8]) -> Result<Level> {
    ensure!(bytes.len() >= 6, "level file too short for header");
    let width = read_u16le(bytes, 2) as usize;
    ensure!(width.is_power_of_two(), "map width {width} is not a power of two");
    let actor_word_count = read_u16le(bytes, 4) as usize;
    let actor_bytes = actor_word_count * 2;
    ensure!(
        bytes.len() >= 6 + actor_bytes,
        "level file too short for actor table"
    );

    let mut actors = Vec::with_capacity(actor_word_count / 3);
    let mut off = 6;
    for _ in 0..(actor_word_count / 3) {
        actors.push(MapActor {
            map_type: read_u16le(bytes, off),
            x: read_u16le(bytes, off + 2),
            y: read_u16le(bytes, off + 4),
        });
        off += 6;
    }

    // The original reads tile data with a single fixed-size fread() capped at
    // WORD_MAX bytes and never stores an explicit height; trailing all-empty
    // words at the end of the last row are routinely trimmed from the file
    // on disk (the load buffer is otherwise implicitly zeroed). So: round the
    // row count up and zero-pad ("air") any missing trailing words.
    let tile_bytes = &bytes[6 + actor_bytes..];
    let tile_word_count = tile_bytes.len() / 2;
    let height = tile_word_count.div_ceil(width);
    let mut tiles: Vec<u16> = tile_bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    tiles.resize(width * height, 0);

    Ok(Level {
        width,
        height,
        actors,
        tiles,
    })
}
