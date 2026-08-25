use crate::data::{GameData, LevelJson, MASKED_TILE_THRESHOLD};
use crate::tileset::{TilesetAssets, TILE_PX};
use bevy::prelude::*;

#[derive(Component)]
pub struct TileMarker;

#[derive(Component)]
pub struct BackdropMarker;

/// Entities that belong to the currently-loaded level and should be
/// despawned wholesale when switching to a new one.
#[derive(Component)]
pub struct LevelScoped;

#[derive(Resource, Default)]
pub struct CurrentLevel {
    pub name: String,
    pub width: usize,
    pub height: usize,
    /// Tile-space bounding box of actually-populated (non-air) cells; the
    /// level buffer itself is always a fixed power-of-two size (up to
    /// 512x64) with real content filling only a fraction of it.
    pub content_min: (usize, usize),
    pub content_max: (usize, usize),
    pub music: Option<String>,
}

/// Tile-space bounding box `(min_x, min_y, max_x, max_y)` of non-air cells.
pub fn content_bounds(level: &LevelJson) -> (usize, usize, usize, usize) {
    let (mut min_x, mut min_y) = (level.width, level.height);
    let (mut max_x, mut max_y) = (0, 0);
    let mut any = false;
    for y in 0..level.height {
        for x in 0..level.width {
            if level.tile_at(x, y) != 0 {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if !any {
        return (0, 0, level.width, level.height);
    }
    (min_x, min_y, max_x + 1, max_y + 1)
}

/// Converts a tile-grid coordinate (row 0 = top) into the world-space
/// position of that tile's top-left pixel corner (Y-up).
pub fn tile_to_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * TILE_PX, -y * TILE_PX)
}

/// Same, but for a Bevy `Sprite`'s default `Anchor::Center` - offsets by
/// half the given pixel size so the sprite's top-left corner lands exactly
/// on the tile grid.
pub fn tile_topleft_to_center(x: f32, y: f32, width_px: f32, height_px: f32) -> Vec2 {
    tile_to_world(x, y) + Vec2::new(width_px / 2.0, -height_px / 2.0)
}

pub fn spawn_level_tiles(
    commands: &mut Commands,
    tileset: &TilesetAssets,
    level: &LevelJson,
    _data: &GameData,
) {
    for y in 0..level.height {
        for x in 0..level.width {
            let raw = level.tile_at(x, y);
            if raw == 0 {
                continue;
            }
            let pos = tile_topleft_to_center(x as f32, y as f32, TILE_PX, TILE_PX);
            let (image, layout, index) = if raw >= MASKED_TILE_THRESHOLD {
                let idx = ((raw - MASKED_TILE_THRESHOLD) / 8) as usize;
                (tileset.masked_image.clone(), tileset.masked_layout.clone(), idx)
            } else {
                let idx = (raw / 8) as usize;
                (tileset.solid_image.clone(), tileset.solid_layout.clone(), idx)
            };
            commands.spawn((
                Sprite {
                    image,
                    texture_atlas: Some(TextureAtlas { layout, index }),
                    ..default()
                },
                Transform::from_translation(pos.extend(0.0)),
                TileMarker,
                LevelScoped,
            ));
        }
    }
}

const BACKDROP_PX: f32 = TILE_PX * 40.0; // 320
const BACKDROP_PX_H: f32 = TILE_PX * 18.0; // 144

/// Tiles the level's backdrop image edge-to-edge behind the populated tile
/// area. This is a static (non-parallax) tiling for now - the original
/// scrolls the backdrop at half the foreground's rate for a depth effect,
/// which would need a scrolling/wrapping material to do seamlessly; a
/// reasonable follow-up rather than blocking this on it.
pub fn spawn_backdrop(
    commands: &mut Commands,
    asset_server: &AssetServer,
    level: &LevelJson,
    bounds: (usize, usize, usize, usize),
) {
    let Some(name) = &level.backdrop else {
        return;
    };
    let handle: Handle<Image> = asset_server.load(format!("generated/backdrops/{name}.png"));
    let (min_x, min_y, max_x, max_y) = bounds;
    let world_w = (max_x - min_x) as f32 * TILE_PX;
    let world_h = (max_y - min_y) as f32 * TILE_PX;
    let cols = (world_w / BACKDROP_PX).ceil() as i32 + 1;
    let rows = (world_h / BACKDROP_PX_H).ceil() as i32 + 1;
    for row in 0..rows {
        for col in 0..cols {
            let x = min_x as f32 * TILE_PX + col as f32 * BACKDROP_PX;
            let y = min_y as f32 * TILE_PX + row as f32 * BACKDROP_PX_H;
            commands.spawn((
                Sprite {
                    image: handle.clone(),
                    anchor: bevy::sprite::Anchor::TopLeft,
                    ..default()
                },
                Transform::from_translation(Vec3::new(x, -y, -10.0)),
                BackdropMarker,
                LevelScoped,
            ));
        }
    }
}

/// Player-start (SPA_PLAYER_START, map_type 0) tile position, or the level's
/// top-left as a fallback.
pub fn find_player_start(level: &LevelJson) -> (f32, f32) {
    level
        .actors
        .iter()
        .find(|a| a.map_type == 0)
        .map(|a| (a.x as f32, a.y as f32))
        .unwrap_or((2.0, 2.0))
}
