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
    /// tile, indexed as `(raw - MASKED_TILE_THRESHOLD) / 40` - masked tiles
    /// are addressed as a direct byte offset into MASKTILE.MNI (confirmed
    /// via `DrawMaskedTile(maskedTileData + *mapcell, ...)`,
    /// game1.c:896-897, `localsrc = src - 16000` in the low-level draw
    /// routine), NOT the tile_index*8 EGA-VRAM-address scheme solid tiles
    /// use - each masked tile is 40 bytes, so the byte-offset divisor is
    /// 40, not 8. (Tile *attribute* lookups are a separate, unrelated
    /// indexing scheme: `tileAttributeData[raw_value / 8]` uniformly for
    /// both solid and masked values - see game1.c:247-254.)
    pub tiles: Vec<u16>,
    pub backdrop_num: u16,
    pub music_num: u16,
    pub has_rain: bool,
    pub has_h_scroll_backdrop: bool,
    pub has_v_scroll_backdrop: bool,
    pub palette_animation_num: u8,
}

/// Backdrop names as indexed by `backdrop_num` (game1.c:111-118). Not every
/// index is present in every episode's data files.
pub const BACKDROP_NAMES: [&str; 26] = [
    "bdblank.mni", "bdpipe.mni", "bdredsky.mni", "bdrocktk.mni", "bdjungle.mni",
    "bdstar.mni", "bdwierd.mni", "bdcave.mni", "bdice.mni", "bdshrum.mni",
    "bdtechms.mni", "bdnewsky.mni", "bdstar2.mni", "bdstar3.mni",
    "bdforest.mni", "bdmountn.mni", "bdguts.mni", "bdbrktec.mni",
    "bdclouds.mni", "bdfutcty.mni", "bdice2.mni", "bdcliff.mni", "bdspooky.mni",
    "bdcrystl.mni", "bdcircut.mni", "bdcircpc.mni",
];

/// Music names as indexed by `music_num` (game1.c:120-125).
pub const MUSIC_NAMES: [&str; 19] = [
    "mcaves.mni", "mscarry.mni", "mboss.mni", "mrunaway.mni", "mcircus.mni",
    "mtekwrd.mni", "measylev.mni", "mrockit.mni", "mhappy.mni", "mdevo.mni",
    "mdadoda.mni", "mbells.mni", "mdrums.mni", "mbanjo.mni", "measy2.mni",
    "mteck2.mni", "mteck3.mni", "mteck4.mni", "mzztop.mni",
];

fn read_u16le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

pub fn parse(bytes: &[u8]) -> Result<Level> {
    ensure!(bytes.len() >= 6, "level file too short for header");
    let map_variables = read_u16le(bytes, 0);
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
        backdrop_num: map_variables & 0x001f,
        has_rain: map_variables & 0x0020 != 0,
        has_h_scroll_backdrop: map_variables & 0x0040 != 0,
        has_v_scroll_backdrop: map_variables & 0x0080 != 0,
        palette_animation_num: ((map_variables >> 8) & 0x07) as u8,
        music_num: (map_variables >> 11) & 0x001f,
    })
}
