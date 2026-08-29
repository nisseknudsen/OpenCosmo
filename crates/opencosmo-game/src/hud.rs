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

/// A pixel offset within the virtual screen, as a percentage of it.
fn pct(px: u32, of: u32) -> f32 {
    px as f32 / of as f32 * 100.0
}

/// Keeps the HUD camera from also redrawing the world's sprites.
pub const HUD_RENDER_LAYER: usize = 1;

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

/// Spawns the full-window camera every piece of UI targets.
///
/// The game camera is clipped to the play area, and Bevy lays UI out inside
/// its target camera's viewport - so anything targeting it would be
/// squeezed into that same clipped strip. This one covers the whole window,
/// draws after the game camera (order 1) without clearing it, and sits on
/// its own render layer so it doesn't redraw every world sprite on top.
pub fn spawn_ui_camera_on(world: &mut World, target: bevy::render::camera::RenderTarget) -> Entity {
    world
        .spawn((
            Camera2d,
            Camera {
                // Over the whole virtual screen, including the border, so
                // UI tile coordinates are the original's screen coordinates.
                target,
                order: 1,
                clear_color: bevy::render::camera::ClearColorConfig::None,
                ..default()
            },
            RenderLayers::layer(HUD_RENDER_LAYER),
        ))
        .id()
}

#[derive(Component)]
pub struct StatusBarUi;

pub fn spawn_hud(
    commands: &mut Commands,
    hud: &HudAssets,
    status_bar: Handle<Image>,
    ui_camera: Entity,
) {
    commands
        .spawn((
            StatusBarUi,
            // Exactly where `DrawStaticGameScreen` puts it (game2.c:3596):
            // screen tiles x 1..38, y 19..24. The UI camera covers the whole
            // 320x200 virtual screen, so these percentages are exact.
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(pct(crate::presentation::BAR_X, crate::presentation::SCREEN_W)),
                top: Val::Percent(pct(crate::presentation::BAR_Y, crate::presentation::SCREEN_H)),
                width: Val::Percent(pct(crate::presentation::BAR_W, crate::presentation::SCREEN_W)),
                height: Val::Percent(pct(crate::presentation::BAR_H, crate::presentation::SCREEN_H)),
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
/// Sets an atlas index only when it differs.
///
/// Writing through `Mut` marks the component changed whether or not the
/// value moved, and Bevy's UI re-extracts changed nodes every frame. The
/// status bar is written from scratch each frame, so assigning
/// unconditionally dirtied every digit and health cell forever, for a
/// display that changes a few times a minute. `set_if_neq` is the built-in
/// for the visibility half of this; the atlas index needs doing by hand
/// because it sits behind an `Option` inside the component.
fn set_atlas(image: &mut Mut<ImageNode>, index: usize) {
    let current = image.texture_atlas.as_ref().map(|a| a.index);
    if current == Some(index) {
        return;
    }
    if let Some(atlas) = image.bypass_change_detection().texture_atlas.as_mut() {
        atlas.index = index;
        image.set_changed();
    }
}

fn apply_digits(
    value: u32,
    index: usize,
    image: &mut Mut<ImageNode>,
    visibility: &mut Mut<Visibility>,
) {
    let divisor = 10u32.saturating_pow(index as u32);
    if index > 0 && value < divisor {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Inherited);
    set_atlas(image, DIGIT_0 + (value / divisor % 10) as usize);
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
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        visibility.set_if_neq(Visibility::Inherited);
        let filled = player.health - 1 > cell.index as i32;
        set_atlas(
            &mut image,
            match (filled, cell.upper) {
                (true, true) => FONT_UPPER_BAR_1,
                (true, false) => FONT_LOWER_BAR_1,
                (false, true) => FONT_UPPER_BAR_0,
                (false, false) => FONT_LOWER_BAR_0,
            },
        );
    }
}

