//! Loads the generated (converted) assets from disk. These are small local
//! JSON/PNG files produced by `opencosmo-assets` at build time, so plain
//! synchronous `std::fs` reads at startup are simpler than routing through
//! Bevy's async asset server for the structured (non-image) data.

use bevy::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const MASKED_TILE_THRESHOLD: u16 = 16000;
pub const TILE_ATTR_BLOCK_SOUTH: u8 = 0x01;
pub const TILE_ATTR_BLOCK_NORTH: u8 = 0x02;
pub const TILE_ATTR_BLOCK_WEST: u8 = 0x04;
pub const TILE_ATTR_BLOCK_EAST: u8 = 0x08;
pub const TILE_ATTR_SLIPPERY: u8 = 0x10;
pub const TILE_ATTR_IN_FRONT: u8 = 0x20;
pub const TILE_ATTR_SLOPED: u8 = 0x40;
pub const TILE_ATTR_CAN_CLING: u8 = 0x80;

#[derive(Deserialize, Clone)]
pub struct LevelActorJson {
    pub map_type: u16,
    pub x: u16,
    pub y: u16,
}

#[derive(Deserialize, Clone, Default)]
pub struct LevelJson {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<u16>,
    pub actors: Vec<LevelActorJson>,
    pub backdrop: Option<String>,
    pub music: Option<String>,
    pub has_h_scroll_backdrop: bool,
    pub has_v_scroll_backdrop: bool,
}

impl LevelJson {
    pub fn tile_at(&self, x: usize, y: usize) -> u16 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.tiles[y * self.width + x]
    }
}

#[derive(Deserialize)]
pub struct TilesetJson {
    pub tile_size: u32,
    pub atlas_cols: u32,
    pub solid_tile_count: usize,
    pub masked_tile_count: usize,
}

#[derive(Deserialize)]
pub struct FrameMetaJson {
    pub file: String,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Deserialize)]
pub struct SpriteManifestJson {
    pub frames: Vec<FrameMetaJson>,
}

/// Set once when `GameData` loads, so the many `AssetServer` call sites
/// scattered across the game don't each have to thread the episode through.
static ASSET_PREFIX: OnceLock<String> = OnceLock::new();

/// An `AssetServer`-relative path inside the loaded episode's asset tree,
/// e.g. `asset_path("screens/title.png")`.
pub fn asset_path(rel: &str) -> String {
    let prefix = ASSET_PREFIX.get().map(String::as_str).unwrap_or("generated/ep1");
    format!("{prefix}/{rel}")
}

/// Root resource: where the generated asset tree lives, and cached lookups.
#[derive(Resource)]
pub struct GameData {
    /// Absolute path to the selected episode's converted assets.
    pub root: PathBuf,
    /// Path of `root` relative to the Bevy asset root, for handing to the
    /// `AssetServer` (which resolves its own paths, and won't take an
    /// absolute one).
    pub asset_prefix: String,
    pub episode: u8,
    pub tileset: TilesetJson,
    pub tile_attrs: Vec<u8>,
}

impl GameData {
    /// Reads the episode to load from `COSMO_EPISODE`, defaulting to 1.
    pub fn selected_episode() -> u8 {
        std::env::var("COSMO_EPISODE")
            .ok()
            .and_then(|v| v.trim().parse::<u8>().ok())
            .filter(|n| opencosmo_assets::convert::EPISODES.contains(n))
            .unwrap_or(1)
    }

    pub fn load(assets_dir: &Path) -> Self {
        Self::load_episode(assets_dir, Self::selected_episode())
    }

    pub fn load_episode(assets_dir: &Path, episode: u8) -> Self {
        let root = assets_dir.join("generated").join(format!("ep{episode}"));
        let tileset: TilesetJson = read_json(&root.join("tileset.json")).unwrap_or_else(|_| {
            panic!(
                "tileset.json missing for episode {episode} at {} - did build.rs run?",
                root.display()
            )
        });
        let tile_attrs = std::fs::read(root.join("tile_attrs.bin")).unwrap_or_default();
        let asset_prefix = format!("generated/ep{episode}");
        let _ = ASSET_PREFIX.set(asset_prefix.clone());
        Self {
            root,
            asset_prefix,
            episode,
            tileset,
            tile_attrs,
        }
    }

    /// Builds an `AssetServer`-relative path inside this episode's tree,
    /// e.g. `asset_path("screens/title.png")`.
    pub fn asset_path(&self, rel: &str) -> String {
        format!("{}/{}", self.asset_prefix, rel)
    }

    /// The episode's level progression, as emitted by the converter. Falls
    /// back to whatever levels exist if the manifest is missing.
    pub fn level_order(&self) -> Vec<String> {
        read_json::<Vec<String>>(&self.root.join("levels").join("order.json"))
            .unwrap_or_else(|_| self.list_levels())
    }

    pub fn tile_attr(&self, raw_value: u16) -> u8 {
        if raw_value == 0 {
            return 0;
        }
        // Attribute lookup is a single uniform formula for both solid and
        // masked values - no threshold branching, unlike the graphic index
        // (game1.c:247-254: `tileAttributeData[raw_value / 8]`). This is a
        // *different* indexing scheme than the masked-tile graphic lookup
        // (which is `(raw - 16000) / 40`, a direct byte offset into
        // MASKTILE.MNI) - don't conflate the two.
        let index = raw_value / 8;
        self.tile_attrs
            .get(index as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn load_level(&self, stem: &str) -> Option<LevelJson> {
        read_json(&self.root.join("levels").join(format!("{stem}.json"))).ok()
    }

    pub fn list_levels(&self) -> Vec<String> {
        let dir = self.root.join("levels");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                if p.extension()?.to_str()? == "json" {
                    Some(p.file_stem()?.to_str()?.to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    pub fn load_sprite_manifest(&self, rel_dir: &str) -> Option<SpriteManifestJson> {
        read_json(&self.root.join(rel_dir).join("manifest.json")).ok()
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A measurement rather than an assertion, kept because its answer is
    /// why `CurrentLevel` carries the parsed map instead of systems
    /// re-reading it: four per-tick calls at this cost is about 1ms of
    /// every tick spent rebuilding something that cannot change.
    #[test]
    fn level_load_cost() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        if !dir.join("generated/ep1/tileset.json").exists() {
            // Assets are generated from the user's own installer, so a
            // checkout without one must not fail the suite.
            return;
        }
        let data = GameData::load_episode(&dir, 1);
        let start = std::time::Instant::now();
        const N: u32 = 200;
        let mut tiles = 0usize;
        for _ in 0..N {
            tiles += data.load_level("a1").map(|l| l.tiles.len()).unwrap_or(0);
        }
        let each = start.elapsed() / N;
        println!("load_level(\"a1\") = {each:?} each ({tiles} tiles touched)");
        println!("  4 calls per tick at 18.2Hz = {:?}/s", each * 4 * 18);
    }
}
