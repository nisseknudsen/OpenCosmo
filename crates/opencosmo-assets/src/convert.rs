//! Orchestrates turning the raw GOG installer into Bevy-ready assets
//! (PNG tile atlases, JSON levels, PNG backdrops/sprites) under a target
//! directory, skipping the work entirely when a cache stamp already matches.

use crate::{level, palette, shell, sprite, tile, vol};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Writes interleaved stereo i16 PCM to a WAV file.
fn write_wav_stereo(pcm: &[i16], sample_rate: u32, out_path: &Path) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(out_path, spec)
        .with_context(|| format!("creating {}", out_path.display()))?;
    for sample in pcm {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

const ATLAS_COLS: u32 = 40;
/// Status bar geometry from DrawStaticGameScreen() (game2.c:3597-3604):
/// screen rows 19..24 x columns 1..38.
pub const STATUS_W_TILES: usize = 38;
pub const STATUS_H_TILES: usize = 6;
/// Font atlas is 100 masked tiles; 10 per row keeps digit rows aligned
/// (digits are font tiles 26..35, i.e. FONT_0 = byte offset 0x0410 / 40).
pub const FONT_ATLAS_COLS: u32 = 10;

/// `SPR_*` ids that no map actor spawns directly, so they'd never be picked
/// up by scanning the levels - but the game creates them at runtime for
/// explosions, pounce debris, bombs and score pop-ups. From sprite.h:
/// SPARKLE_SHORT=15, POUNCE_DEBRIS=21, SPARKLE_LONG=23, BOMB_ARMED=24,
/// EXPLOSION=26, SMOKE=97, SMOKE_LARGE=98, SCORE_EFFECT_100..12800=177..184.
pub const EFFECT_SPRITES: &[u16] = &[
    15, 21, 23, 24, 26, 97, 98, 177, 178, 179, 180, 181, 182, 183, 184,
];

/// Sprites for actors that exist only at runtime.
///
/// The scan above only sees what a *level places*, which is the right rule
/// for scenery but silently wrong for anything spawned mid-play: a turret's
/// projectile appears in no map, so its sprite was never converted, so the
/// turret fired nothing. The failure is quiet - `spawn_one_actor` cannot
/// load a manifest and returns - which is why it survived being "ported".
///
/// Anything added to `Enemy::spawns` needs its sprite here unless a level
/// also places one.
pub const RUNTIME_SPAWNED_SPRITES: &[u16] = &[
    68, // SPR_PROJECTILE - fired by the turret and the wall plants
    82, // SPR_HAMBURGER - dropped by a destroyed satellite
    65, // SPR_BABY_GHOST - hatched from an egg; placed only in episodes 2-3
    79, // SPR_FOUNTAIN - placed as SPA_FOUNTAIN_*, which is a special
        // actor (map_type < 31) and so never seen by the ACT_* scan above
];
const MAX_ACTOR_TYPES: usize = 400; // generous upper bound on ACT_*/SPR_* ids
const MAX_FRAMES_PER_TYPE: usize = 24;
const MUSIC_SAMPLE_RATE: u32 = 44100;
const SFX_SAMPLE_RATE: u32 = 44100;

#[derive(Serialize)]
struct LevelJson {
    name: String,
    width: usize,
    height: usize,
    tiles: Vec<u16>,
    actors: Vec<ActorJson>,
    backdrop: Option<String>,
    music: Option<String>,
    has_h_scroll_backdrop: bool,
    has_v_scroll_backdrop: bool,
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

/// One entry of `sfx/manifest.json`, so the game can look effects up by
/// their `SND_*` number without re-deriving filenames.
#[derive(Serialize)]
struct SoundJson {
    number: usize,
    stem: String,
    priority: u8,
    label: String,
    ticks: usize,
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

/// The three episodes shipped in the retail release.
pub const EPISODES: [u8; 3] = [1, 2, 3];

/// How one episode names its data, from `episode{1,2,3}.h`.
///
/// Only the `.VOL` differs between episodes - every `.STN` in the retail
/// release holds byte-identical shared assets (tiles, sprites, font,
/// sounds), so the tileset/sprite/sfx conversion below is repeated per
/// episode rather than shared purely for the simplicity of one
/// self-contained output tree each.
pub struct EpisodeSpec {
    pub number: u8,
    /// Main levels are `A1..`, `B1..`, `C1..` for episodes 1/2/3
    /// (episode1.h:23-32, episode2.h:23-32, episode3.h:23-32).
    pub level_prefix: char,
    /// Each episode has its own pair of bonus stages, interleaved into the
    /// progression after every two main levels.
    pub bonus_levels: [&'static str; 2],
}

pub fn episode_spec(number: u8) -> Result<EpisodeSpec> {
    Ok(match number {
        1 => EpisodeSpec {
            number: 1,
            level_prefix: 'A',
            bonus_levels: ["BONUS1.MNI", "BONUS2.MNI"],
        },
        2 => EpisodeSpec {
            number: 2,
            level_prefix: 'B',
            bonus_levels: ["BONUS3.MNI", "BONUS4.MNI"],
        },
        3 => EpisodeSpec {
            number: 3,
            level_prefix: 'C',
            bonus_levels: ["BONUS5.MNI", "BONUS6.MNI"],
        },
        n => anyhow::bail!("no such episode: {n}"),
    })
}

/// Where one episode's converted assets live under the shared output root.
pub fn episode_dir(out_dir: &Path, episode: u8) -> PathBuf {
    out_dir.join(format!("ep{episode}"))
}

/// Converts every episode, unless `out_dir`'s cache stamp already matches
/// the installer's contents + this crate's converter version. One stamp
/// covers all three, written only after they all succeed, so a partial
/// conversion can never be mistaken for a complete one. Returns `true` if a
/// (re)conversion actually happened.
pub fn convert_all_episodes_if_stale(sh_path: &Path, out_dir: &Path) -> Result<bool> {
    let source_bytes = std::fs::read(sh_path)
        .with_context(|| format!("reading installer {}", sh_path.display()))?;
    let fp = crate::cache::fingerprint(&source_bytes);
    if crate::cache::is_fresh(out_dir, &fp) {
        return Ok(false);
    }
    for episode in EPISODES {
        convert_episode(sh_path, &episode_dir(out_dir, episode), episode)
            .with_context(|| format!("converting episode {episode}"))?;
    }
    crate::cache::write_stamp(out_dir, &fp)?;
    Ok(true)
}

/// Converts everything needed for one episode: full tileset, every shipped
/// main/bonus level, every backdrop, and the player + in-use actor sprites.
/// Returns the list of level stems actually converted, in progression
/// order, so the game can follow what's really on disk rather than a
/// hardcoded (and possibly shareware-truncated) list.
pub fn convert_episode(sh_path: &Path, out_dir: &Path, episode: u8) -> Result<Vec<String>> {
    let spec = episode_spec(episode)?;
    std::fs::create_dir_all(out_dir)?;

    let mut zip = shell::open_installer_zip(sh_path)?;
    let vol_bytes = shell::read_zip_entry(&mut zip, &format!("COSMO{episode}.VOL"))?;
    let stn_bytes = shell::read_zip_entry(&mut zip, &format!("COSMO{episode}.STN"))?;
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

    // --- Status bar background: a plain 38x6 grid of solid tiles (7296
    // bytes = 38*6*32), blitted to screen rows 19..24 / columns 1..38 by
    // DrawStaticGameScreen() (game2.c:3590-3610). ---
    let status_tiles = tile::decode_all_solid(&get(&stn_map, "STATUS.MNI")?);
    let mut status_px = vec![[0u8; 4]; STATUS_W_TILES * 8 * STATUS_H_TILES * 8];
    let status_w_px = STATUS_W_TILES * 8;
    for (t, tile) in status_tiles.iter().enumerate() {
        let tx = (t % STATUS_W_TILES) * 8;
        let ty = (t / STATUS_W_TILES) * 8;
        for (i, p) in tile.iter().enumerate() {
            status_px[(ty + i / 8) * status_w_px + (tx + i % 8)] = *p;
        }
    }
    save_rgba(
        &out_dir.join("status_bar.png"),
        status_w_px as u32,
        (STATUS_H_TILES * 8) as u32,
        &status_px,
    )?;

    // --- Font: 100 masked tiles. LoadFontTileData() (game1.c:541-553)
    // inverts every 5th byte on load - i.e. the AND-mask byte of each
    // 5-byte row - so the on-disk mask is stored inverted relative to the
    // normal masked-tile convention. Undo that before decoding. ---
    let mut fonts_mni = get(&stn_map, "FONTS.MNI")?;
    for i in (0..fonts_mni.len()).step_by(5) {
        fonts_mni[i] = !fonts_mni[i];
    }
    let font_tiles = tile::decode_all_masked(&fonts_mni);
    save_atlas(&out_dir.join("font.png"), &font_tiles, FONT_ATLAS_COLS)?;

    // --- Levels: every <prefix>N.MNI plus this episode's two bonus stages ---
    let mut main_levels: Vec<&str> = vol_entries
        .iter()
        .map(|e| e.name.as_str())
        .filter(|n| {
            let u = n.to_ascii_uppercase();
            u.starts_with(spec.level_prefix)
                && u.ends_with(".MNI")
                && u[1..u.len() - 4].parse::<u32>().is_ok()
        })
        .collect();
    main_levels.sort_by_key(|n| {
        let u = n.to_ascii_uppercase();
        u[1..u.len() - 4].parse::<u32>().unwrap_or(0)
    });
    let bonus_present: Vec<&str> = spec
        .bonus_levels
        .iter()
        .copied()
        .filter(|b| vol_map.contains_key(*b))
        .collect();

    let mut level_names: Vec<&str> = main_levels.clone();
    level_names.extend(bonus_present.iter().copied());

    let levels_dir = out_dir.join("levels");
    std::fs::create_dir_all(&levels_dir)?;
    let mut converted = Vec::new();
    for name in &level_names {
        let data = *vol_map
            .get(&name.to_ascii_uppercase())
            .expect("name came from this map");
        let lvl = level::parse(data).with_context(|| format!("parsing level {name}"))?;
        let backdrop = level::BACKDROP_NAMES
            .get(lvl.backdrop_num as usize)
            .map(|n| n.trim_end_matches(".mni").to_string());
        let music = level::MUSIC_NAMES
            .get(lvl.music_num as usize)
            .map(|n| n.trim_end_matches(".mni").to_string());
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
            backdrop,
            music,
            has_h_scroll_backdrop: lvl.has_h_scroll_backdrop,
            has_v_scroll_backdrop: lvl.has_v_scroll_backdrop,
        };
        let stem = name.trim_end_matches(".MNI").trim_end_matches(".mni");
        std::fs::write(
            levels_dir.join(format!("{}.json", stem.to_ascii_lowercase())),
            serde_json::to_vec(&json)?,
        )?;
        converted.push(stem.to_ascii_lowercase());
    }

    // The progression interleaves the two bonus stages after every pair of
    // main levels (episode1.h:23-32 and its siblings). Emitting it here
    // keeps the episode's naming scheme - A/B/C prefixes, bonus1..bonus6 -
    // out of the game, which otherwise has to hardcode episode 1's.
    let stem_of = |n: &str| {
        n.trim_end_matches(".MNI")
            .trim_end_matches(".mni")
            .to_ascii_lowercase()
    };
    let mut order: Vec<String> = Vec::new();
    for pair in main_levels.chunks(2) {
        order.extend(pair.iter().map(|n| stem_of(n)));
        order.extend(bonus_present.iter().map(|n| stem_of(n)));
    }
    std::fs::write(
        levels_dir.join("order.json"),
        serde_json::to_vec(&order)?,
    )?;

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
    // The title and end screens are per-episode (TITLE2.MNI, END3.MNI, ...)
    // but are written under episode-neutral names so the game can ask for
    // "the title screen" without knowing which episode is loaded. The rest
    // are shared and keep their own names.
    let title = format!("TITLE{episode}.MNI");
    let end = format!("END{episode}.MNI");
    let screens: [(&str, &str); 5] = [
        (title.as_str(), "title"),
        (end.as_str(), "end"),
        ("PRETITLE.MNI", "pretitle"),
        ("BONUS.MNI", "bonus"),
        ("CREDIT.MNI", "credit"),
    ];
    for (name, stem) in screens {
        let data = if let Some(d) = vol_map.get(name) {
            *d
        } else if let Some(d) = stn_map.get(name) {
            *d
        } else {
            continue;
        };
        let px = tile::decode_fullscreen(data, 320, 200);
        save_rgba(&screens_dir.join(format!("{stem}.png")), 320, 200, &px)?;
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

    // --- Actor sprites: only the SPR_* types actually needed by actors
    // spawned in converted levels. ACT_* (map actor type) and SPR_* (the
    // graphic actually drawn) are different numbering spaces that only
    // sometimes coincide - actor_sprite_map.rs, extracted from every
    // `case ACT_X: ConstructActor(SPR_Y, ...)` arm of NewActorAtIndex()
    // (game1.c:5618-6371), is the ground truth for this. Folders are keyed
    // by SPR id (not ACT id) since many ACT_* types share one sprite.
    let actrinfo = to_u16le(&get(&stn_map, "ACTRINFO.MNI")?);
    let actors_mni = get(&stn_map, "ACTORS.MNI")?;
    let mut used_sprites: Vec<u16> = Vec::new();
    for name in &level_names {
        let data = vol_map[&name.to_ascii_uppercase()];
        let lvl = level::parse(data)?;
        for a in &lvl.actors {
            if a.map_type >= 31 {
                let act = a.map_type - 31;
                let spr = crate::actor_sprite_map::ACT_TO_SPRITE
                    .iter()
                    .find(|(id, ..)| *id == act)
                    .map(|(_, spr, ..)| *spr)
                    .unwrap_or(act); // fall back to direct mapping if genuinely unlisted
                if !used_sprites.contains(&spr) {
                    used_sprites.push(spr);
                }
            }
        }
    }
    used_sprites.extend_from_slice(EFFECT_SPRITES);
    used_sprites.extend_from_slice(RUNTIME_SPAWNED_SPRITES);
    used_sprites.sort_unstable();
    used_sprites.dedup();
    used_sprites.retain(|&t| (t as usize) < actrinfo.len() && (t as usize) < MAX_ACTOR_TYPES);

    let actors_dir = out_dir.join("sprites/actors");
    std::fs::create_dir_all(&actors_dir)?;
    for &t in &used_sprites {
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

    // --- Music: every M*.MNI track this episode's data actually shipped,
    // rendered from IMF to WAV via an OPL2 emulator (see music.rs). ---
    let music_dir = out_dir.join("music");
    std::fs::create_dir_all(&music_dir)?;
    // The remastered soundtrack: the same score, re-voiced. See notes.rs
    // (recovering notes from the register stream) and lofi.rs (rendering
    // them). Emitted alongside the authentic mix rather than replacing it,
    // so the game can switch between them at runtime.
    let remaster_dir = out_dir.join("music_remaster");
    std::fs::create_dir_all(&remaster_dir)?;
    for name in level::MUSIC_NAMES {
        let upper = name.to_ascii_uppercase();
        let Some(&raw) = vol_map.get(&upper) else {
            continue;
        };
        let events = crate::music::parse_imf(raw)
            .with_context(|| format!("parsing music track {name}"))?;
        let stem = name.trim_end_matches(".mni");
        crate::music::render_to_wav(&events, MUSIC_SAMPLE_RATE, &music_dir.join(format!("{stem}.wav")))
            .with_context(|| format!("rendering music track {name}"))?;

        // Rhythm mode would put percussion on channels 6-8 and make the
        // melodic decode wrong. No shipped track uses it, but a silently
        // wrong remaster is worse than none.
        if crate::notes::uses_rhythm_mode(&events) {
            eprintln!("opencosmo-assets: {name} uses OPL rhythm mode; skipping remaster");
            continue;
        }
        let notes = crate::notes::extract_notes(&events);
        let voices = crate::lofi::assign_voices(&crate::notes::channel_median_pitch(&notes));
        let loop_secs = crate::music::duration_ticks(&events) as f64 / crate::music::TICK_HZ;
        let settings = crate::lofi::RenderSettings {
            sample_rate: MUSIC_SAMPLE_RATE,
            ..Default::default()
        };
        let pcm = crate::lofi::render(&notes, loop_secs, &voices, &settings);
        write_wav_stereo(&pcm, MUSIC_SAMPLE_RATE, &remaster_dir.join(format!("{stem}.wav")))
            .with_context(|| format!("rendering remastered track {name}"))?;
    }

    // --- PC speaker sound effects: three banks of 23, stitched into one
    // contiguous 1-based numbering matching the SND_* constants, exactly
    // as LoadSoundData() does (game1.c:8255-8257). See sound.rs. ---
    let sfx_dir = out_dir.join("sfx");
    std::fs::create_dir_all(&sfx_dir)?;
    let mut sfx_manifest: Vec<SoundJson> = Vec::new();
    for (bank_index, bank_name) in crate::sound::SOUND_BANKS.iter().enumerate() {
        let raw = get(&stn_map, bank_name)?;
        let effects = crate::sound::parse_sound_bank(&raw)
            .with_context(|| format!("parsing sound bank {bank_name}"))?;
        for (i, effect) in effects.iter().enumerate() {
            let sound_number = bank_index * crate::sound::SOUNDS_PER_BANK + i + 1;
            let stem = crate::sound::sound_stem(sound_number);
            crate::sound::render_to_wav(
                &effect.samples,
                SFX_SAMPLE_RATE,
                &sfx_dir.join(format!("{stem}.wav")),
            )
            .with_context(|| format!("rendering sound {sound_number} ({stem})"))?;
            sfx_manifest.push(SoundJson {
                number: sound_number,
                stem,
                priority: effect.priority,
                label: effect.label.clone(),
                ticks: effect.samples.len(),
            });
        }
    }
    std::fs::write(
        sfx_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&sfx_manifest)?,
    )?;

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
