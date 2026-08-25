//! The in-game status bar, rebuilt from the original's own artwork.
//!
//! `DrawStaticGameScreen()` (game2.c:3590-3610) blits STATUS.MNI as a plain
//! 38x6 tile panel into screen rows 19..24 / columns 1..38, then draws the
//! live numbers over it. Those number positions are given in *screen* tile
//! coordinates, so each is converted here into a bar-relative tile by
//! subtracting the panel's own origin (column 1, row 19):
//!
//! | field  | screen pos      | bar-relative  | drawn as              |
//! |--------|-----------------|---------------|-----------------------|
//! | score  | (9, 22) right   | (8, 3) right  | digits, flush right   |
//! | health | (17, 22)        | (16, 3)       | meter, grows leftward |
//! | bombs  | (24, 23) right  | (23, 4) right | digits, flush right   |
//! | stars  | (35, 22) right  | (34, 3) right | digits, flush right   |
//!
//! Digits are font tiles 26..35 (FONT_0 = byte offset 0x0410, 40 bytes per
//! masked tile). Each health cell is two font tiles stacked vertically -
//! filled (95/96) while `health - 1 > cell`, empty (9/8) otherwise
//! (DrawStatusBarHealth, game2.c:1314-1337).

use crate::flow::{Score, Stars};
use crate::player::Player;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

const BAR_W_TILES: f32 = 38.0;
const BAR_H_TILES: f32 = 6.0;

const DIGIT_0: usize = 26;
const FONT_LOWER_BAR_0: usize = 8;
const FONT_UPPER_BAR_0: usize = 9;
const FONT_UPPER_BAR_1: usize = 95;
const FONT_LOWER_BAR_1: usize = 96;

const SCORE_X: usize = 8;
const SCORE_ROW: usize = 3;
const HEALTH_X: usize = 16;
const HEALTH_ROW: usize = 3;
const BOMBS_X: usize = 23;
const BOMBS_ROW: usize = 4;
const STARS_X: usize = 34;
const STARS_ROW: usize = 3;

const SCORE_DIGITS: usize = 8;
const STARS_DIGITS: usize = 3;
const BOMBS_DIGITS: usize = 2;
/// The original allows for more cells than the bar has room for
/// ("Why 8 if there are only 5 health cell spaces?" - game2.c:1320).
const HEALTH_CELLS: usize = 5;

/// Fraction of the window the play area occupies. The original screen is
/// 200px tall: a 152px game window (rows 0..18) above a 48px status bar
/// (rows 19..24). Keeping that split means the bar sits *below* the action
/// rather than covering it.
pub const GAME_VIEW_FRACTION: f32 = 152.0 / 200.0;

/// Keeps the HUD camera from also redrawing the world's sprites.
const HUD_RENDER_LAYER: usize = 1;

#[derive(Resource)]
pub struct HudAssets {
    pub font_image: Handle<Image>,
    pub font_layout: Handle<TextureAtlasLayout>,
}

/// `index` counts right-to-left from the field's anchor tile, matching the
/// original's flush-right number drawing.
#[derive(Component)]
pub struct ScoreDigit(usize);
#[derive(Component)]
pub struct StarDigit(usize);
#[derive(Component)]
pub struct BombDigit(usize);
#[derive(Component)]
pub struct HealthCell {
    index: usize,
    upper: bool,
}

fn tile_node(tile_x: f32, tile_y: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(tile_x / BAR_W_TILES * 100.0),
        top: Val::Percent(tile_y / BAR_H_TILES * 100.0),
        width: Val::Percent(100.0 / BAR_W_TILES),
        height: Val::Percent(100.0 / BAR_H_TILES),
        ..default()
    }
}

fn atlas_image(hud: &HudAssets, index: usize) -> ImageNode {
    ImageNode::from_atlas_image(
        hud.font_image.clone(),
        TextureAtlas {
            layout: hud.font_layout.clone(),
            index,
        },
    )
}

pub fn spawn_hud(commands: &mut Commands, hud: &HudAssets, status_bar: Handle<Image>) {
    // The game camera is clipped to the play area, and Bevy lays UI out
    // inside its target camera's viewport - so the bar would end up
    // squeezed into that same clipped strip. Give the UI its own
    // full-window camera, drawn after (order 1) without clearing what the
    // game camera already rendered, and keep it on a separate render layer
    // so it doesn't redraw every world sprite on top of the game view.
    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: bevy::render::camera::ClearColorConfig::None,
                ..default()
            },
            RenderLayers::layer(HUD_RENDER_LAYER),
        ))
        .id();

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent((1.0 - GAME_VIEW_FRACTION) * 100.0),
                ..default()
            },
            UiTargetCamera(ui_camera),
            RenderLayers::layer(HUD_RENDER_LAYER),
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode::new(status_bar),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ))
            .with_children(|bar| {
                for i in 0..SCORE_DIGITS {
                    bar.spawn((
                        atlas_image(hud, DIGIT_0),
                        tile_node((SCORE_X - i) as f32, SCORE_ROW as f32),
                        ScoreDigit(i),
                    ));
                }
                for i in 0..STARS_DIGITS {
                    bar.spawn((
                        atlas_image(hud, DIGIT_0),
                        tile_node((STARS_X - i) as f32, STARS_ROW as f32),
                        StarDigit(i),
                    ));
                }
                for i in 0..BOMBS_DIGITS {
                    bar.spawn((
                        atlas_image(hud, DIGIT_0),
                        tile_node((BOMBS_X - i) as f32, BOMBS_ROW as f32),
                        BombDigit(i),
                    ));
                }
                for i in 0..HEALTH_CELLS {
                    for (upper, row_offset) in [(true, 0.0), (false, 1.0)] {
                        bar.spawn((
                            atlas_image(hud, FONT_UPPER_BAR_0),
                            tile_node((HEALTH_X - i) as f32, HEALTH_ROW as f32 + row_offset),
                            HealthCell { index: i, upper },
                        ));
                    }
                }
            });
        });
}

/// Sets each slot to its digit, hiding leading slots that aren't needed.
/// Slot 0 always shows, so a value of 0 renders as "0" rather than blank.
fn apply_digits(value: u32, index: usize, image: &mut ImageNode, visibility: &mut Visibility) {
    let divisor = 10u32.saturating_pow(index as u32);
    if index > 0 && value < divisor {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Inherited;
    if let Some(atlas) = image.texture_atlas.as_mut() {
        atlas.index = DIGIT_0 + (value / divisor % 10) as usize;
    }
}

pub fn update_status_bar(
    score: Res<Score>,
    stars: Res<Stars>,
    player_q: Query<&Player>,
    mut score_q: Query<
        (&ScoreDigit, &mut ImageNode, &mut Visibility),
        (Without<StarDigit>, Without<BombDigit>, Without<HealthCell>),
    >,
    mut star_q: Query<
        (&StarDigit, &mut ImageNode, &mut Visibility),
        (Without<ScoreDigit>, Without<BombDigit>, Without<HealthCell>),
    >,
    mut bomb_q: Query<
        (&BombDigit, &mut ImageNode, &mut Visibility),
        (Without<ScoreDigit>, Without<StarDigit>, Without<HealthCell>),
    >,
    mut health_q: Query<
        (&HealthCell, &mut ImageNode, &mut Visibility),
        (Without<ScoreDigit>, Without<StarDigit>, Without<BombDigit>),
    >,
) {
    let Ok(player) = player_q.single() else {
        return;
    };

    for (digit, mut image, mut visibility) in &mut score_q {
        apply_digits(score.0, digit.0, &mut image, &mut visibility);
    }
    for (digit, mut image, mut visibility) in &mut star_q {
        apply_digits(stars.0, digit.0, &mut image, &mut visibility);
    }
    for (digit, mut image, mut visibility) in &mut bomb_q {
        apply_digits(player.bombs, digit.0, &mut image, &mut visibility);
    }
    for (cell, mut image, mut visibility) in &mut health_q {
        // Only `health_cells` of the meter's slots exist at any time; the
        // rest of the pre-spawned slots stay hidden until a power-up
        // widens the meter.
        if cell.index >= player.health_cells as usize {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Inherited;
        let filled = player.health - 1 > cell.index as i32;
        let index = match (filled, cell.upper) {
            (true, true) => FONT_UPPER_BAR_1,
            (true, false) => FONT_LOWER_BAR_1,
            (false, true) => FONT_UPPER_BAR_0,
            (false, false) => FONT_LOWER_BAR_0,
        };
        if let Some(atlas) = image.texture_atlas.as_mut() {
            atlas.index = index;
        }
    }
}

/// Confines the 3D/2D camera to the play area so the status bar occupies
/// its own strip at the bottom instead of overlapping the action.
pub fn fit_camera_to_play_area(
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera, With<crate::camera::GameCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }
    let play_height = (size.y as f32 * GAME_VIEW_FRACTION).round() as u32;
    let physical_size = UVec2::new(size.x, play_height.max(1));
    // Viewport has no PartialEq, so compare the fields we actually set.
    let unchanged = camera
        .viewport
        .as_ref()
        .is_some_and(|v| v.physical_size == physical_size && v.physical_position == UVec2::ZERO);
    if !unchanged {
        camera.viewport = Some(bevy::render::camera::Viewport {
            physical_position: UVec2::ZERO,
            physical_size,
            ..default()
        });
    }
}
