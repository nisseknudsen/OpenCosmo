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
            // Values below TILE_STRIPED_PLATFORM (80) are "air" or a
            // platform-direction command, not a real graphic - the
            // original just shows backdrop through them (game1.c:889:
            // `if (*mapcell < TILE_STRIPED_PLATFORM) { ...just backdrop... }`).
            if raw < 80 {
                continue;
            }
            let pos = tile_topleft_to_center(x as f32, y as f32, TILE_PX, TILE_PX);
            let (image, layout, index) = if raw >= MASKED_TILE_THRESHOLD {
                // Masked tiles are addressed as a direct byte offset into
                // MASKTILE.MNI (40 bytes/tile), not the tile_index*8
                // EGA-VRAM scheme solid tiles use - see level.rs's doc
                // comment on MASKED_TILE_THRESHOLD for the source citation.
                let idx = ((raw - MASKED_TILE_THRESHOLD) / 40) as usize;
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

pub const BACKDROP_PX: f32 = TILE_PX * 40.0; // 320
pub const BACKDROP_PX_H: f32 = TILE_PX * 18.0; // 144
const GRID: i32 = 5; // NxN backdrop tiles - generous margin around the viewport,
                      // especially for the axis with scrolling disabled (fixed
                      // position, so it needs static coverage rather than wrap)

/// Grid offset (in backdrop-tile units) this entity occupies; repositioned
/// every frame by `scroll_backdrop` to wrap around the camera.
#[derive(Component)]
pub struct BackdropTile {
    pub col: i32,
    pub row: i32,
}

/// Whether the backdrop tracks the camera (wrapping, at half rate for a
/// parallax depth effect) on each axis - `hasHScrollBackdrop`/
/// `hasVScrollBackdrop` from the level's map-flags bitfield
/// (level.rs::parse, game1.c:10490-10504). An axis with scrolling disabled
/// stays fixed at its spawn position instead of tracking the camera -
/// getting this wrong is exactly what caused the reported "mountain
/// repeats above the clouds when jumping": we used to tile the backdrop
/// across the whole level in *both* axes unconditionally, even though
/// e.g. level A1 has v-scroll disabled.
#[derive(Resource)]
pub struct BackdropScroll {
    pub h: bool,
    pub v: bool,
    pub anchor_x: f32,
    pub anchor_y: f32,
}

/// Spawns a fixed 3x3 grid of backdrop tiles; `scroll_backdrop` repositions
/// them every frame to wrap seamlessly around the camera instead of tiling
/// the whole level up front.
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
    for row in -(GRID / 2)..=(GRID / 2) {
        for col in -(GRID / 2)..=(GRID / 2) {
            commands.spawn((
                Sprite {
                    image: handle.clone(),
                    anchor: bevy::sprite::Anchor::TopLeft,
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
                BackdropTile { col, row },
                BackdropMarker,
                LevelScoped,
            ));
        }
    }
    let (min_x, min_y, ..) = bounds;
    commands.insert_resource(BackdropScroll {
        h: level.has_h_scroll_backdrop,
        v: level.has_v_scroll_backdrop,
        anchor_x: min_x as f32 * TILE_PX,
        anchor_y: -(min_y as f32) * TILE_PX, // world Y is negative going down
    });
}

pub fn scroll_backdrop(
    camera_q: Query<&Transform, (With<crate::camera::GameCamera>, Without<BackdropTile>)>,
    scroll: Option<Res<BackdropScroll>>,
    mut tiles: Query<(&BackdropTile, &mut Transform)>,
) {
    let Ok(cam_t) = camera_q.single() else {
        return;
    };
    let Some(scroll) = scroll else {
        return;
    };
    // Half the camera's rate, for a parallax depth effect (the original
    // achieves this with pre-shifted backdrop copies; we just move it).
    let base_x = if scroll.h {
        (cam_t.translation.x * 0.5 / BACKDROP_PX).floor() * BACKDROP_PX
    } else {
        scroll.anchor_x
    };
    let base_y = if scroll.v {
        (cam_t.translation.y * 0.5 / BACKDROP_PX_H).floor() * BACKDROP_PX_H
    } else {
        scroll.anchor_y
    };
    for (tile, mut t) in &mut tiles {
        t.translation.x = base_x + tile.col as f32 * BACKDROP_PX;
        t.translation.y = base_y + tile.row as f32 * BACKDROP_PX_H;
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
