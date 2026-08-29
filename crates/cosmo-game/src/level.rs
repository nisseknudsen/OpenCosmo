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
    /// The parsed map, kept here rather than re-read per system.
    ///
    /// It used to be loaded from disk and deserialised inside every system
    /// that needed to test a tile - four of them, every tick. That is a
    /// 96KB file read and a 32768-element parse at 268us a time, so around
    /// 1.07ms of every tick spent rebuilding something that cannot change
    /// while a level is being played, plus a quarter of a megabyte of
    /// allocation churn per tick.
    pub level: LevelJson,
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

/// Z of a tile the map says draws in front of everything. Above the
/// player (10) and every actor (5..8).
const FOREGROUND_Z: f32 = 12.0;

pub fn spawn_level_tiles(
    commands: &mut Commands,
    tileset: &TilesetAssets,
    level: &LevelJson,
    data: &GameData,
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
            // `TILE_IN_FRONT` (game1.c:252). `DrawSprite` skips any 8x8
            // chunk of a sprite whose destination map cell has this set
            // (game1.c:1250-1254), so flagged tiles hide whatever is behind
            // them - the game's own foreground layer, and the reason Cosmo
            // disappears behind foliage rather than walking over it.
            //
            // Reproduced by drawing those tiles *over* the sprites instead
            // of masking the sprites, which is equivalent wherever the tile
            // is opaque - 97.5% of them. For the masked 2.5% the sprite
            // shows through the tile's gaps here, where the original hides
            // it in that whole cell.
            let z = if data.tile_attr(raw) & crate::data::TILE_ATTR_IN_FRONT != 0 {
                FOREGROUND_Z
            } else {
                0.0
            };
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
                Transform::from_translation(pos.extend(z)),
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

/// Per-axis backdrop movement rate relative to the camera.
///
/// The original draws the backdrop straight into the 320x144 game window
/// every frame (game1.c:885-901), so it is fundamentally **screen-locked**,
/// not pinned to any world position. `hasHScrollBackdrop`/
/// `hasVScrollBackdrop` (the level's map-flags bitfield, level.rs::parse)
/// only choose whether that screen-space image is *offset* as you scroll:
///
/// - flag set   -> backdrop moves at **half** the camera's rate, the
///   classic parallax depth effect (game1.c:708-713).
/// - flag clear -> backdrop moves at exactly the camera's rate, i.e. it
///   appears completely static behind the action.
///
/// Anchoring a non-scrolling axis to a fixed *world* position instead (as
/// this did before) makes the backdrop drift out of the viewport as the
/// camera moves - which is what put A1's mountains down behind the ground
/// instead of along the horizon.
#[derive(Resource)]
pub struct BackdropScroll {
    pub rate_x: f32,
    pub rate_y: f32,
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
    let handle: Handle<Image> = asset_server.load(crate::data::asset_path(&format!("backdrops/{name}.png")));
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
    let _ = bounds; // backdrop is screen-locked; level bounds no longer factor in
    commands.insert_resource(BackdropScroll {
        rate_x: if level.has_h_scroll_backdrop { 0.5 } else { 1.0 },
        rate_y: if level.has_v_scroll_backdrop { 0.5 } else { 1.0 },
    });
}

pub fn scroll_backdrop(
    camera_q: Query<
        (&Transform, &Projection),
        (With<crate::camera::GameCamera>, Without<BackdropTile>),
    >,
    scroll: Option<Res<BackdropScroll>>,
    mut tiles: Query<(&BackdropTile, &mut Transform)>,
) {
    let Ok((cam_t, projection)) = camera_q.single() else {
        return;
    };
    let Some(scroll) = scroll else {
        return;
    };
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    let cam_x = cam_t.translation.x;
    let cam_y = cam_t.translation.y;

    // Pin the reference tile's top-left to the viewport's top-left, then
    // slide it back by the parallax lag. `1.0 - rate` is how far the layer
    // falls behind the camera per unit of travel: 0 for a static backdrop
    // (rate 1.0), half for a parallax one (rate 0.5). Taking that modulo
    // the backdrop size keeps the lag inside one tile so the surrounding
    // grid covers whatever the shift exposes.
    let lag_x = (cam_x * (1.0 - scroll.rate_x)).rem_euclid(BACKDROP_PX);
    let lag_y = (cam_y * (1.0 - scroll.rate_y)).rem_euclid(BACKDROP_PX_H);
    let base_x = cam_x - ortho.area.width() / 2.0 - lag_x;
    let base_y = cam_y + ortho.area.height() / 2.0 + lag_y;

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
