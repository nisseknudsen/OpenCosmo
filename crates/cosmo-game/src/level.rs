use crate::data::{GameData, LevelJson, MASKED_TILE_THRESHOLD};
use crate::tileset::{TilesetAssets, TILE_PX};
use bevy::prelude::*;

#[derive(Component)]
pub struct TileMarker;

#[derive(Resource, Default)]
pub struct CurrentLevel {
    pub name: String,
    pub width: usize,
    pub height: usize,
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
