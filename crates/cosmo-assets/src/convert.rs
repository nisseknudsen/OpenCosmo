//! Orchestrates turning the raw GOG installer into Bevy-ready assets
//! (PNG tile atlases, JSON levels, PNG backdrops/sprites) under a target
//! directory, skipping the work entirely when a cache stamp already matches.

use crate::{level, palette, shell, sprite, tile, vol};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const ATLAS_COLS: u32 = 40;
const MAX_ACTOR_TYPES: usize = 400; // generous upper bound on ACT_*/SPR_* ids
const MAX_FRAMES_PER_TYPE: usize = 24;

#[derive(Serialize)]
struct LevelJson {
    name: String,
    width: usize,
    height: usize,
    tiles: Vec<u16>,
    actors: Vec<ActorJson>,
}

#[derive(Serialize)]
struct ActorJson {
    map_type: u16,
    x: u16,
    y: u16,
}

#[derive(Serialize)]
struct FrameMeta {
    file: String,
    width_px: u32,
    height_px: u32,
}

#[derive(Serialize)]
struct SpriteManifest {
    frames: Vec<FrameMeta>,
}

#[derive(Serialize)]
struct TilesetJson {
    tile_size: u32,
    atlas_cols: u32,
    solid_tile_count: usize,
    masked_tile_count: usize,
}

fn entries_by_name<'a>(entries: &'a [vol::VolEntry<'a>]) -> HashMap<String, &'a [u8]> {
    entries
        .iter()
        .map(|e| (e.name.to_ascii_uppercase(), e.data))
        .collect()
}

fn save_atlas(path: &Path, tiles: &[[[u8; 4]; 64]], cols: u32) -> Result<()> {
    if tiles.is_empty() {
        let img = RgbaImage::new(1, 1);
        img.save(path)?;
        return Ok(());
    }
    let rows = (tiles.len() as u32).div_ceil(cols);
    let mut img = RgbaImage::new(cols * 8, rows * 8);
    for (t, tile) in tiles.iter().enumerate() {
        let tx = (t as u32 % cols) * 8;
        let ty = (t as u32 / cols) * 8;
        for (i, p) in tile.iter().enumerate() {
            img.put_pixel(tx + (i as u32 % 8), ty + (i as u32 / 8), Rgba(*p));
        }
    }
    img.save(path)?;
    Ok(())
}

fn save_rgba(path: &Path, w: u32, h: u32, px: &[[u8; 4]]) -> Result<()> {
    let mut img = RgbaImage::new(w, h);
    for (i, p) in px.iter().enumerate() {
        img.put_pixel(i as u32 % w, i as u32 / w, Rgba(*p));
    }
    img.save(path)?;
    Ok(())
}

/// Runs `convert_episode1` only if `out_dir`'s cache stamp doesn't already
/// match the installer's contents + this crate's converter version. Returns
/// `true` if a (re)conversion actually happened.
pub fn convert_episode1_if_stale(sh_path: &Path, out_dir: &Path) -> Result<bool> {
    let source_bytes = std::fs::read(sh_path)
        .with_context(|| format!("reading installer {}", sh_path.display()))?;
    let fp = crate::cache::fingerprint(&source_bytes);
    if crate::cache::is_fresh(out_dir, &fp) {
        return Ok(false);
    }
    convert_episode1(sh_path, out_dir)?;
    crate::cache::write_stamp(out_dir, &fp)?;
    Ok(true)
}

/// Converts everything needed for a faithful, extensible Episode 1: full
/// tileset, every shipped A*/bonus level, every backdrop, and the player +
/// in-use actor sprites. Returns the list of level names actually converted
/// (in file order) so the game can build its level progression from what's
/// really on disk rather than a hardcoded (and possibly shareware-truncated)
/// list.
pub fn convert_episode1(sh_path: &Path, out_dir: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(out_dir)?;

    let mut zip = shell::open_installer_zip(sh_path)?;
    let vol_bytes = shell::read_zip_entry(&mut zip, "COSMO1.VOL")?;
    let stn_bytes = shell::read_zip_entry(&mut zip, "COSMO1.STN")?;
    let vol_entries = vol::parse(&vol_bytes)?;
    let stn_entries = vol::parse(&stn_bytes)?;
    let vol_map = entries_by_name(&vol_entries);
    let stn_map = entries_by_name(&stn_entries);

    let get = |map: &HashMap<String, &[u8]>, name: &str| -> Result<Vec<u8>> {
        map.get(&name.to_ascii_uppercase())
            .map(|d| d.to_vec())
            .with_context(|| format!("missing entry {name}"))
    };

    // --- Tileset ---
    let solid_tiles = tile::decode_all_solid(&get(&stn_map, "TILES.MNI")?);
    let masked_tiles = tile::decode_all_masked(&get(&stn_map, "MASKTILE.MNI")?);
    save_atlas(&out_dir.join("tileset_solid.png"), &solid_tiles, ATLAS_COLS)?;
    save_atlas(&out_dir.join("tileset_masked.png"), &masked_tiles, ATLAS_COLS)?;
    std::fs::write(
        out_dir.join("tileset.json"),
        serde_json::to_vec_pretty(&TilesetJson {
            tile_size: 8,
            atlas_cols: ATLAS_COLS,
            solid_tile_count: solid_tiles.len(),
            masked_tile_count: masked_tiles.len(),
        })?,
    )?;
    std::fs::write(out_dir.join("tile_attrs.bin"), get(&stn_map, "TILEATTR.MNI")?)?;

    // --- Levels: convert every A*.MNI / bonus*.mni present, in a stable order ---
    let mut level_names: Vec<&str> = vol_entries
        .iter()
        .map(|e| e.name.as_str())
        .filter(|n| {
            let u = n.to_ascii_uppercase();
            u.starts_with('A') && u.ends_with(".MNI") && u[1..u.len() - 4].parse::<u32>().is_ok()
        })
        .collect();
    level_names.sort_by_key(|n| {
        let u = n.to_ascii_uppercase();
        u[1..u.len() - 4].parse::<u32>().unwrap_or(0)
    });
    for extra in ["BONUS1.MNI", "BONUS2.MNI"] {
        if vol_map.contains_key(extra) {
            level_names.push(extra);
        }
    }

    let levels_dir = out_dir.join("levels");
    std::fs::create_dir_all(&levels_dir)?;
    let mut converted = Vec::new();
    for name in &level_names {
        let data = *vol_map
            .get(&name.to_ascii_uppercase())
            .expect("name came from this map");
        let lvl = level::parse(data).with_context(|| format!("parsing level {name}"))?;
        let json = LevelJson {
            name: name.to_string(),
            width: lvl.width,
            height: lvl.height,
            tiles: lvl.tiles,
            actors: lvl
                .actors
                .iter()
                .map(|a| ActorJson {
                    map_type: a.map_type,
                    x: a.x,
                    y: a.y,
                })
                .collect(),
        };
        let stem = name.trim_end_matches(".MNI").trim_end_matches(".mni");
        std::fs::write(
            levels_dir.join(format!("{}.json", stem.to_ascii_lowercase())),
            serde_json::to_vec(&json)?,
        )?;
        converted.push(stem.to_ascii_lowercase());
    }

    // --- Backdrops: every BD*.MNI in either container. These are a plain
    // 40x18 grid of individually-encoded 32-byte solid tiles (BACKDROP_SIZE
    // = 40*18*32), *not* a single plane-major fullscreen bitmap.
    let backdrops_dir = out_dir.join("backdrops");
    std::fs::create_dir_all(&backdrops_dir)?;
    for (map, entries) in [(&vol_map, &vol_entries), (&stn_map, &stn_entries)] {
        for e in entries.iter() {
            if !e.name.to_ascii_uppercase().starts_with("BD") {
                continue;
            }
            let data = map[&e.name.to_ascii_uppercase()];
            let bd_tiles = tile::decode_all_solid(data);
            let mut px = vec![[0u8; 4]; 320 * 144];
            for (t, tile) in bd_tiles.iter().enumerate() {
                let tx = (t % 40) * 8;
                let ty = (t / 40) * 8;
                for (i, p) in tile.iter().enumerate() {
                    px[(ty + i / 8) * 320 + (tx + i % 8)] = *p;
                }
            }
            save_rgba(
                &backdrops_dir.join(format!(
                    "{}.png",
                    e.name
                        .trim_end_matches(".MNI")
                        .trim_end_matches(".mni")
                        .to_ascii_lowercase()
                )),
                320,
                144,
                &px,
            )?;
        }
    }

    // --- Full-screen images ---
    let screens_dir = out_dir.join("screens");
    std::fs::create_dir_all(&screens_dir)?;
    for name in ["TITLE1.MNI", "END1.MNI", "PRETITLE.MNI", "BONUS.MNI", "CREDIT.MNI"] {
        let data = if let Some(d) = vol_map.get(name) {
            *d
        } else if let Some(d) = stn_map.get(name) {
            *d
        } else {
            continue;
        };
        let px = tile::decode_fullscreen(data, 320, 200);
        save_rgba(
            &screens_dir.join(format!("{}.png", name.trim_end_matches(".MNI").to_ascii_lowercase())),
            320,
            200,
            &px,
        )?;
    }

    // --- Player sprite: all frames ---
    let plyrinfo = to_u16le(&get(&stn_map, "PLYRINFO.MNI")?);
    let players_mni = get(&stn_map, "PLAYERS.MNI")?;
    let player_dir = out_dir.join("sprites/player");
    std::fs::create_dir_all(&player_dir)?;
    // The player has no per-type header table (there's only one implicit
    // "type"), so the max_frames_for_type boundary heuristic doesn't apply -
    // it would mistake early frame-record words for other types' bases.
    // Fall back to a generous flat cap; the width/height/bounds sanity
    // checks find the real natural end (confirmed empirically at 48 frames,
    // well under this cap, with every frame a clean sprite).
    let player_frames =
        convert_frames_bounded(&plyrinfo, 0, &players_mni, &player_dir, 96)?;
    std::fs::write(
        player_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&SpriteManifest {
            frames: player_frames,
        })?,
    )?;

    // --- Actor sprites: only the types actually spawned in converted levels ---
    let actrinfo = to_u16le(&get(&stn_map, "ACTRINFO.MNI")?);
    let actors_mni = get(&stn_map, "ACTORS.MNI")?;
    let mut used_types: Vec<u16> = Vec::new();
    for name in &level_names {
        let data = vol_map[&name.to_ascii_uppercase()];
        let lvl = level::parse(data)?;
        for a in &lvl.actors {
            if a.map_type >= 31 {
                let act = a.map_type - 31;
                if !used_types.contains(&act) {
                    used_types.push(act);
                }
            }
        }
    }
    used_types.retain(|&t| (t as usize) < actrinfo.len() && (t as usize) < MAX_ACTOR_TYPES);

    let actors_dir = out_dir.join("sprites/actors");
    std::fs::create_dir_all(&actors_dir)?;
    for &t in &used_types {
        let dir = actors_dir.join(t.to_string());
        std::fs::create_dir_all(&dir)?;
        let frames = convert_sprite_frames(&actrinfo, t as usize, &actors_mni, &dir)?;
        if frames.is_empty() {
            std::fs::remove_dir_all(&dir).ok();
            continue;
        }
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec_pretty(&SpriteManifest { frames })?,
        )?;
    }

    Ok(converted)
}

fn to_u16le(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// Decodes frames 0.. for one sprite type until data stops looking valid,
/// writing each as its own PNG. actorTileData is banked (up to 3 buffers);
/// `data` is the *whole* source blob (ACTORS.MNI or PLAYERS.MNI) and we
/// re-split per frame since only ACTORS.MNI actually needs banking.
fn convert_sprite_frames(
    info: &[u16],
    type_index: usize,
    tile_data: &[u8],
    out_dir: &Path,
) -> Result<Vec<FrameMeta>> {
    if type_index >= info.len() {
        return Ok(Vec::new());
    }
    let frame_cap = sprite::max_frames_for_type(info, type_index).min(MAX_FRAMES_PER_TYPE);
    convert_frames_bounded(info, type_index, tile_data, out_dir, frame_cap)
}

fn convert_frames_bounded(
    info: &[u16],
    type_index: usize,
    tile_data: &[u8],
    out_dir: &Path,
    frame_cap: usize,
) -> Result<Vec<FrameMeta>> {
    let banks = sprite::split_actor_banks(tile_data);
    let mut frames = Vec::new();
    if type_index >= info.len() {
        return Ok(frames);
    }
    for frame in 0..frame_cap {
        let base = info[type_index] as usize;
        let off = base + frame * 4;
        if off + 3 >= info.len() {
            break;
        }
        let fi = sprite::frame_info(info, type_index, frame);
        if fi.width_tiles == 0
            || fi.height_tiles == 0
            || fi.width_tiles > 10
            || fi.height_tiles > 10
        {
            break;
        }
        let bank = banks[(fi.bank as usize).min(2)];
        let need = fi.data_offset as usize + fi.width_tiles as usize * fi.height_tiles as usize * 40;
        if need > bank.len() {
            break;
        }
        let tiles = sprite::decode_frame_tiles(bank, &fi);
        let (w, h, px) = sprite::composite_frame(&fi, &tiles);
        let file = format!("frame_{frame:02}.png");
        save_rgba(&out_dir.join(&file), w, h, &px)?;
        frames.push(FrameMeta {
            file,
            width_px: w,
            height_px: h,
        });
    }
    Ok(frames)
}

#[allow(dead_code)]
fn palette_preview() -> &'static [[u8; 4]; 16] {
    &palette::EGA_PALETTE
}
