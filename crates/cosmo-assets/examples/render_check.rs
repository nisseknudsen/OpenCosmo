//! Manual visual-verification tool: decodes real game assets from the
//! installer and dumps PNGs so a human (or Claude) can eyeball whether the
//! format decoders are actually correct.

use anyhow::Result;
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

fn main() -> Result<()> {
    let sh_path = PathBuf::from(std::env::args().nth(1).expect("path to installer .sh"));
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/cosmo-render-check".to_string()),
    );
    std::fs::create_dir_all(&out_dir)?;

    let mut zip = cosmo_assets::shell::open_installer_zip(&sh_path)?;
    let vol_bytes = cosmo_assets::shell::read_zip_entry(&mut zip, "COSMO1.VOL")?;
    let stn_bytes = cosmo_assets::shell::read_zip_entry(&mut zip, "COSMO1.STN")?;
    let vol = cosmo_assets::vol::parse(&vol_bytes)?;
    let stn = cosmo_assets::vol::parse(&stn_bytes)?;

    let find = |entries: &[cosmo_assets::vol::VolEntry], name: &str| -> Vec<u8> {
        entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("missing entry {name}"))
            .data
            .to_vec()
    };

    // 1. Title screen (plane-major fullscreen decode check).
    let title = find(&vol, "TITLE1.MNI");
    let px = cosmo_assets::tile::decode_fullscreen(&title, 320, 200);
    save_rgba(&out_dir.join("title1.png"), 320, 200, &px)?;

    // 2. Full solid tileset as a sprite sheet (sanity check on tile decode).
    let tiles_mni = find(&stn, "TILES.MNI");
    let solid_tiles = cosmo_assets::tile::decode_all_solid(&tiles_mni);
    save_tile_sheet(&out_dir.join("tiles_solid.png"), &solid_tiles, 40)?;

    // 3. Full masked tileset as a sprite sheet.
    let masktile_mni = find(&stn, "MASKTILE.MNI");
    let masked_tiles = cosmo_assets::tile::decode_all_masked(&masktile_mni);
    save_tile_sheet(&out_dir.join("tiles_masked.png"), &masked_tiles, 40)?;

    // 4. Render level A1 using the decoded tileset.
    let a1_mni = find(&vol, "A1.MNI");
    let level = cosmo_assets::level::parse(&a1_mni)?;
    println!(
        "A1.MNI: {}x{} tiles, {} actors",
        level.width,
        level.height,
        level.actors.len()
    );
    render_level(
        &out_dir.join("a1_level.png"),
        &level,
        &solid_tiles,
        &masked_tiles,
    )?;

    // 5. Player sprite: decode a handful of frames.
    let plyrinfo = to_u16le(&find(&stn, "PLYRINFO.MNI"));
    let players_mni = find(&stn, "PLAYERS.MNI");
    let mut sheet_tiles: Vec<Vec<[[u8; 4]; 64]>> = Vec::new();
    let mut sheet_dims: Vec<(usize, usize)> = Vec::new();
    for frame in 0..12usize {
        let info = cosmo_assets::sprite::frame_info(&plyrinfo, 0, frame);
        if info.width_tiles == 0 || info.height_tiles == 0 {
            break;
        }
        let tiles = cosmo_assets::sprite::decode_frame_tiles(&players_mni, &info);
        sheet_dims.push((info.width_tiles as usize, info.height_tiles as usize));
        sheet_tiles.push(tiles);
    }
    save_sprite_strip(&out_dir.join("player_frames.png"), &sheet_tiles, &sheet_dims)?;

    // 6. A handful of actor sprite types.
    let actrinfo = to_u16le(&find(&stn, "ACTRINFO.MNI"));
    let actors_mni = find(&stn, "ACTORS.MNI");
    let banks = cosmo_assets::sprite::split_actor_banks(&actors_mni);
    let mut actor_tiles: Vec<Vec<[[u8; 4]; 64]>> = Vec::new();
    let mut actor_dims: Vec<(usize, usize)> = Vec::new();
    for sprite_type in 0..40usize {
        if sprite_type >= actrinfo.len() {
            break;
        }
        let info = cosmo_assets::sprite::frame_info(&actrinfo, sprite_type, 0);
        if info.width_tiles == 0
            || info.height_tiles == 0
            || info.width_tiles > 8
            || info.height_tiles > 8
        {
            continue;
        }
        let bank = banks[info.bank.min(2) as usize];
        if info.data_offset as usize + info.width_tiles as usize * info.height_tiles as usize * 40
            > bank.len()
        {
            continue;
        }
        let tiles = cosmo_assets::sprite::decode_frame_tiles(bank, &info);
        actor_dims.push((info.width_tiles as usize, info.height_tiles as usize));
        actor_tiles.push(tiles);
    }
    save_sprite_strip(&out_dir.join("actor_frame0_samples.png"), &actor_tiles, &actor_dims)?;

    println!("wrote PNGs to {}", out_dir.display());
    Ok(())
}

fn to_u16le(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn save_sprite_strip(
    path: &std::path::Path,
    frames: &[Vec<[[u8; 4]; 64]>],
    dims: &[(usize, usize)],
) -> Result<()> {
    if frames.is_empty() {
        return Ok(());
    }
    let max_h = dims.iter().map(|(_, h)| *h).max().unwrap_or(1) * 8;
    let total_w: usize = dims.iter().map(|(w, _)| *w * 8 + 2).sum();
    let mut img = RgbaImage::new(total_w as u32, max_h as u32);
    let mut x_cursor = 0usize;
    for (tiles, (w, h)) in frames.iter().zip(dims.iter()) {
        for ty in 0..*h {
            for tx in 0..*w {
                let tile = &tiles[ty * w + tx];
                for i in 0..64 {
                    let px = x_cursor + tx * 8 + i % 8;
                    let py = ty * 8 + i / 8;
                    let p = tile[i];
                    if p[3] != 0 {
                        img.put_pixel(px as u32, py as u32, Rgba(p));
                    }
                }
            }
        }
        x_cursor += w * 8 + 2;
    }
    img.save(path)?;
    Ok(())
}

fn save_rgba(path: &std::path::Path, w: usize, h: usize, px: &[[u8; 4]]) -> Result<()> {
    let mut img = RgbaImage::new(w as u32, h as u32);
    for (i, p) in px.iter().enumerate() {
        img.put_pixel((i % w) as u32, (i / w) as u32, Rgba(*p));
    }
    img.save(path)?;
    Ok(())
}

fn save_tile_sheet(
    path: &std::path::Path,
    tiles: &[[[u8; 4]; 64]],
    cols: usize,
) -> Result<()> {
    let rows = tiles.len().div_ceil(cols);
    let mut img = RgbaImage::new((cols * 8) as u32, (rows * 8) as u32);
    for (t, tile) in tiles.iter().enumerate() {
        let tx = (t % cols) * 8;
        let ty = (t / cols) * 8;
        for (i, p) in tile.iter().enumerate() {
            img.put_pixel((tx + i % 8) as u32, (ty + i / 8) as u32, Rgba(*p));
        }
    }
    img.save(path)?;
    Ok(())
}

fn render_level(
    path: &std::path::Path,
    level: &cosmo_assets::level::Level,
    solid: &[[[u8; 4]; 64]],
    masked: &[[[u8; 4]; 64]],
) -> Result<()> {
    let mut img = RgbaImage::new((level.width * 8) as u32, (level.height * 8) as u32);
    for y in 0..level.height {
        for x in 0..level.width {
            let raw = level.tiles[y * level.width + x];
            let tile: Option<&[[u8; 4]; 64]> = if raw < 80 {
                None
            } else if raw >= cosmo_assets::level::MASKED_TILE_THRESHOLD {
                let idx = ((raw - cosmo_assets::level::MASKED_TILE_THRESHOLD) / 40) as usize;
                masked.get(idx)
            } else {
                let idx = (raw / 8) as usize;
                solid.get(idx)
            };
            if let Some(tile) = tile {
                for (i, p) in tile.iter().enumerate() {
                    if p[3] == 0 {
                        continue;
                    }
                    let px = (x * 8 + i % 8) as u32;
                    let py = (y * 8 + i / 8) as u32;
                    img.put_pixel(px, py, Rgba(*p));
                }
            }
        }
    }
    img.save(path)?;
    Ok(())
}
