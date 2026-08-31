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

/// Which entity is drawing each map cell, so a cell can be redrawn when
/// the map changes under it. The original just writes a new value into the
/// map array and the next frame's draw picks it up; here the map is a
/// grid of entities spawned once, so a write has to find and rebuild one.
///
/// Cells holding air have no entity and so no row here.
#[derive(Resource, Default)]
pub struct TileIndex {
    by_cell: bevy::platform::collections::HashMap<(usize, usize), Entity>,
}

impl TileIndex {
    pub fn clear(&mut self) {
        self.by_cell.clear();
    }
}

/// Builds the entity for one map cell, if it needs one. Shared by the
/// initial load and by later writes so a rebuilt cell is indistinguishable
/// from one that was there all along.
fn spawn_tile_cell(
    commands: &mut Commands,
    tileset: &TilesetAssets,
    data: &GameData,
    index: &mut TileIndex,
    x: usize,
    y: usize,
    raw: u16,
) {
    // Values below TILE_STRIPED_PLATFORM (80) are "air" or a
    // platform-direction command, not a real graphic - the original just
    // shows backdrop through them (game1.c:889).
    if raw < 80 {
        return;
    }
    let pos = tile_topleft_to_center(x as f32, y as f32, TILE_PX, TILE_PX);
    let z = if data.tile_attr(raw) & crate::data::TILE_ATTR_IN_FRONT != 0 {
        FOREGROUND_Z
    } else {
        0.0
    };
    let (image, layout, idx) = if raw >= MASKED_TILE_THRESHOLD {
        let idx = ((raw - MASKED_TILE_THRESHOLD) / 40) as usize;
        (tileset.masked_image.clone(), tileset.masked_layout.clone(), idx)
    } else {
        let idx = (raw / 8) as usize;
        (tileset.solid_image.clone(), tileset.solid_layout.clone(), idx)
    };
    let entity = commands
        .spawn((
            Sprite {
                image,
                texture_atlas: Some(TextureAtlas { layout, index: idx }),
                ..default()
            },
            Transform::from_translation(pos.extend(z)),
            TileMarker,
            LevelScoped,
        ))
        .id();
    index.by_cell.insert((x, y), entity);
}

/// Applies one `SetMapTile`-style write: updates the map the physics reads
/// and rebuilds the cell being drawn, so the two never disagree.
pub fn set_map_tile(
    commands: &mut Commands,
    tileset: &TilesetAssets,
    data: &GameData,
    index: &mut TileIndex,
    level: &mut LevelJson,
    x: i32,
    y: i32,
    raw: u16,
) {
    if x < 0 || y < 0 || x as usize >= level.width || y as usize >= level.height {
        return;
    }
    let (x, y) = (x as usize, y as usize);
    level.tiles[y * level.width + x] = raw;
    if let Some(old) = index.by_cell.remove(&(x, y)) {
        commands.entity(old).despawn();
    }
    spawn_tile_cell(commands, tileset, data, index, x, y, raw);
}

pub fn spawn_level_tiles(
    commands: &mut Commands,
    tileset: &TilesetAssets,
    level: &LevelJson,
    data: &GameData,
    index: &mut TileIndex,
) {
    index.clear();
    for y in 0..level.height {
        for x in 0..level.width {
            spawn_tile_cell(commands, tileset, data, index, x, y, level.tile_at(x, y));
        }
    }
}

#[allow(dead_code)]
fn spawn_level_tiles_old(
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

/// Builds every light in the level: the cone's own cell with its ramp,
/// then straight down until a floor stops it or the cast distance runs out
/// (game1.c:1745-1755).
pub fn spawn_level_lights(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    level: &LevelJson,
    data: &GameData,
) {
    let reach = light_cast_distance(data.episode);
    // One image per shape, shared by every cone that uses it.
    let handles: Vec<Handle<Image>> = [LightSide::West, LightSide::Middle, LightSide::East]
        .into_iter()
        .map(|s| images.add(s.image()))
        .collect();
    let full = handles[1].clone();

    for a in &level.actors {
        let side = match a.map_type {
            6 => LightSide::West,
            7 => LightSide::Middle,
            8 => LightSide::East,
            _ => continue,
        };
        let (x, y) = (a.x as i32, a.y as i32);
        let mut cell = |image: Handle<Image>, x: i32, y: i32| {
            commands.spawn((
                Sprite { image, ..default() },
                Transform::from_translation(
                    tile_topleft_to_center(x as f32, y as f32, TILE_PX, TILE_PX).extend(LIGHT_Z),
                ),
                LightCone { x, y, side },
                LevelScoped,
            ));
        };
        cell(handles[side as usize].clone(), x, y);

        // The cone below is always full-width; only the source cell is
        // ramped.
        for row in (y + 1)..(y + reach) {
            if row < 0 || row as usize >= level.height {
                break;
            }
            if data.tile_attr(level.tile_at(x.max(0) as usize, row as usize))
                & crate::data::TILE_ATTR_BLOCK_SOUTH
                != 0
            {
                break;
            }
            cell(full.clone(), x, row);
        }
    }
}

/// Above the foreground tiles: a light falls on what is drawn, including
/// the layer that covers the player.
const LIGHT_Z: f32 = 13.0;

/// Hides every cone while the lights are switched off (game1.c:1721).
pub fn apply_light_switch(
    switches: Res<crate::enemy_ai::SwitchState>,
    mut cones: Query<&mut Visibility, With<LightCone>>,
) {
    let want = if switches.lights_active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut cones {
        vis.set_if_neq(want);
    }
}

/// A light cone cast by `SPA_LIGHT_*` (map_type 6-8).
///
/// The original does not draw a sprite for these: `LightenScreenTile`
/// (lowlevel.asm:704) sets the EGA intensity bit on plane 3 of the tile
/// already on screen, brightening whatever is behind it. The two edge
/// variants ramp that in over eight rows, which is what gives a cone its
/// sloped sides (lowlevel.asm:626, and the mirrored east version).
///
/// Reproduced here by compositing white additively rather than by
/// remapping each colour to its high-intensity twin. On the EGA palette
/// those are close - every colour's bright form is the same hue lit - but
/// it is an approximation, not the hardware operation.
#[derive(Component)]
pub struct LightCone {
    pub x: i32,
    pub y: i32,
    pub side: LightSide,
}

/// `LIGHT_SIDE_*` (def.h:107-109).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LightSide {
    West = 0,
    Middle = 1,
    East = 2,
}

impl LightSide {
    /// The eight row masks the assembly writes into the EGA bit mask
    /// register, as leftmost-pixel-first booleans.
    fn row_mask(self, row: usize) -> [bool; 8] {
        let mut out = [false; 8];
        match self {
            // Full brightness everywhere.
            LightSide::Middle => out = [true; 8],
            // 00000001b .. 11111111b - fills in from the east edge, so the
            // cone's west boundary slopes away downward.
            LightSide::West => {
                for i in 0..=row {
                    out[7 - i] = true;
                }
            }
            // 10000000b .. 11111111b - the mirror.
            LightSide::East => {
                for i in 0..=row {
                    out[i] = true;
                }
            }
        }
        out
    }

    /// An 8x8 RGBA image of the ramp, white where the mask is set.
    fn image(self) -> Image {
        let mut px = vec![0u8; 8 * 8 * 4];
        for row in 0..8 {
            let mask = self.row_mask(row);
            for (col, lit) in mask.iter().enumerate() {
                if *lit {
                    let i = (row * 8 + col) * 4;
                    px[i] = 255;
                    px[i + 1] = 255;
                    px[i + 2] = 255;
                    px[i + 3] = LIGHT_ALPHA;
                }
            }
        }
        Image::new(
            bevy::render::render_resource::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            px,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
    }
}

/// How strongly a lit cell is brightened. The hardware sets one palette
/// bit, which is a large jump; this is tuned to read as a light cone
/// without washing out the artwork underneath.
const LIGHT_ALPHA: u8 = 56;

/// `LIGHT_CAST_DISTANCE` (episode1.h:20, episode2.h:20, episode3.h:20).
/// Episode 1 casts eleven rows, the later two thirteen.
pub fn light_cast_distance(episode: u8) -> i32 {
    if episode == 1 {
        11
    } else {
        13
    }
}

#[cfg(test)]
mod light_tests {
    use super::*;

    #[test]
    fn a_platforms_route_is_read_out_of_the_map() {
        // The cell a platform sits on holds `DIR8 * 8` as its raw tile
        // value. Those are all below TILE_STRIPED_PLATFORM (80), which is
        // why the renderer treats them as air - so a route command is
        // invisible, which is the point.
        for (dir, (dx, dy)) in crate::effects::DIR8.iter().enumerate() {
            let raw = (dir * 8) as u16;
            assert!(
                raw < 80,
                "a route command must be invisible, but {raw} would draw"
            );
            assert_eq!(crate::effects::DIR8[(raw / 8) as usize], (*dx, *dy));
        }
    }

    #[test]
    fn the_middle_of_a_cone_is_fully_lit() {
        for row in 0..8 {
            assert_eq!(LightSide::Middle.row_mask(row), [true; 8]);
        }
    }

    #[test]
    fn the_cone_edges_ramp_in_opposite_directions() {
        // Transcribed from the EGA bit masks: west fills from the east
        // edge (00000001b upward), east from the west edge (10000000b).
        // Mirroring these the wrong way puts the slope on the wrong side
        // of every light in the game.
        assert_eq!(
            LightSide::West.row_mask(0),
            [false, false, false, false, false, false, false, true],
            "the west edge starts as a single lit pixel on the right"
        );
        assert_eq!(
            LightSide::East.row_mask(0),
            [true, false, false, false, false, false, false, false],
            "and the east edge on the left"
        );
        // Both are full by the last row.
        assert_eq!(LightSide::West.row_mask(7), [true; 8]);
        assert_eq!(LightSide::East.row_mask(7), [true; 8]);
        // ...and each one widens by exactly one pixel per row.
        for row in 0..8 {
            assert_eq!(
                LightSide::West.row_mask(row).iter().filter(|b| **b).count(),
                row + 1
            );
            assert_eq!(
                LightSide::East.row_mask(row).iter().filter(|b| **b).count(),
                row + 1
            );
        }
    }

    #[test]
    fn episode_one_casts_shorter_than_the_others() {
        // episode1.h:20 against episode2.h:20 and episode3.h:20.
        assert_eq!(light_cast_distance(1), 11);
        assert_eq!(light_cast_distance(2), 13);
        assert_eq!(light_cast_distance(3), 13);
    }
}

/// A moving platform placed by `SPA_PLATFORM` (map_type 1).
///
/// The platform is five tiles wide and does not exist as artwork: it
/// stamps `TILE_BLUE_PLATFORM` into the map under itself and stashes
/// whatever it covered, restoring that as it moves on (game1.c:1660-1690).
///
/// Its route is in the map too. The cell it currently sits on holds a
/// direction command - a raw tile value below `TILE_STRIPED_PLATFORM`,
/// which is why the renderer treats those as air - and `value / 8` is the
/// `DIR8` index it travels next.
#[derive(Component)]
pub struct Platform {
    pub x: i32,
    pub y: i32,
    /// The five tiles currently underneath it, west to east.
    pub stash: [u16; 5],
}

/// `TILE_BLUE_PLATFORM` (graphics.h:130); the five cells step by 8.
const TILE_BLUE_PLATFORM: u16 = 0x3dd0;

pub fn spawn_level_platforms(commands: &mut Commands, level: &LevelJson) {
    for a in &level.actors {
        if a.map_type != 1 {
            continue;
        }
        commands.spawn((
            Platform {
                x: a.x as i32,
                y: a.y as i32,
                stash: [0; 5],
            },
            LevelScoped,
        ));
    }
}

/// `MovePlatforms` (game1.c:1660-1690), and the part of
/// `MovePlayerPlatform` that carries a rider (game1.c:1500-1560).
///
/// NOT PORTED: the scroll nudges the original applies while riding, which
/// depend on the look-up/down commands.
pub fn move_platforms(
    mut commands: Commands,
    tileset: Option<Res<crate::tileset::TilesetAssets>>,
    data: Res<GameData>,
    mut tile_index: ResMut<TileIndex>,
    mut current: ResMut<crate::level::CurrentLevel>,
    switches: Res<crate::enemy_ai::SwitchState>,
    mut platforms: Query<&mut Platform>,
    mut player_q: Query<&mut crate::player::Player>,
) {
    let Some(tileset) = tileset else {
        return;
    };
    for mut plat in &mut platforms {
        // Put back what it was covering before reading anything.
        for i in 0..5 {
            let x = plat.x + i as i32 - 2;
            set_map_tile(
                &mut commands,
                &tileset,
                &data,
                &mut tile_index,
                &mut current.level,
                x,
                plat.y,
                plat.stash[i],
            );
        }

        // The route command lives in the cell the platform sits on.
        let raw = if plat.x >= 0 && plat.y >= 0 {
            current.level.tile_at(plat.x as usize, plat.y as usize)
        } else {
            0
        };
        let dir = (raw / 8) as usize % 9;
        let (dx, dy) = crate::effects::DIR8[dir];

        if switches.platforms_active {
            if let Ok(mut player) = player_q.single_mut() {
                // A rider is anyone whose feet are on the platform's row.
                let on_it = player.dead_timer == 0
                    && plat.y - 1 == player.y
                    && player.x >= plat.x - 2
                    && player.x <= plat.x + 2;
                if on_it {
                    player.x += dx;
                    player.y += dy;
                }
            }
            plat.x += dx;
            plat.y += dy;
        }

        // Stash the new footprint, then stamp the platform over it.
        for i in 0..5 {
            let x = plat.x + i as i32 - 2;
            plat.stash[i] = if x >= 0 && plat.y >= 0 {
                current.level.tile_at(x as usize, plat.y as usize)
            } else {
                0
            };
        }
        for i in 0..5 {
            let x = plat.x + i as i32 - 2;
            set_map_tile(
                &mut commands,
                &tileset,
                &data,
                &mut tile_index,
                &mut current.level,
                x,
                plat.y,
                TILE_BLUE_PLATFORM + (i as u16 * 8),
            );
        }
    }
}
