//! The in-game F1 help menu.
//!
//! `ShowHelpMenu` (game1.c:9628-9670) opens a text frame over the paused
//! game offering Save / Restore / Help / Game redefine / View High Scores /
//! Quit, and returns one of continue, restart or quit
//! (`HELP_MENU_*`, def.h:93-95). Save states, the text-page viewer, the
//! key-redefine screen and the high-score table aren't ported, so - as with
//! the main menu - only the entries that actually do something are listed.
//!
//! This pauses rather than switching game state: leaving `GameState::Playing`
//! tears the level down, which is exactly what a pause must not do.

use crate::data::GameData;
use crate::flow::{Checkpoint, Score, Stars};
use crate::hud::{HudAssets, HUD_RENDER_LAYER};
use crate::level::{self, CurrentLevel, LevelScoped};
use crate::player::Player;
use crate::screen::{font_image, UiCamera, FONT_BACKGROUND_GRAY};
use crate::tileset::TilesetAssets;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

/// Set while the help menu is open; gameplay systems don't run.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}

#[derive(Component)]
pub struct HelpUi;


/// Panel geometry as a fraction of the window.
///
/// Deliberately *not* built on the aspect-ratio-sized screen panel the
/// title and main menu use: that sizes itself from its parent's height,
/// which collapses inside the status bar's UI camera, so the panel laid out
/// with zero height and drew nothing. Positioning directly against the
/// window removes the dependency.
const PANEL_LEFT_PCT: f32 = 27.0;
const PANEL_TOP_PCT: f32 = 18.0;
const PANEL_W_PCT: f32 = 46.0;
const PANEL_H_PCT: f32 = 40.0;

/// Text is laid out on the panel's own tile grid, matching the frame the
/// original opens (`UnfoldTextFrame(2, 12, 22, ...)`, game1.c:9630).
const PANEL_COLS: f32 = 22.0;
const PANEL_ROWS: f32 = 9.0;

fn panel_text(parent: &mut ChildSpawnerCommands, hud: &HudAssets, col: f32, row: f32, text: &str) {
    for (i, c) in text.chars().enumerate() {
        let Some(tile) = crate::screen::font_tile_for_char(c) else {
            continue;
        };
        parent.spawn((
            font_image(hud, tile),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((col + i as f32) / PANEL_COLS * 100.0),
                top: Val::Percent(row / PANEL_ROWS * 100.0),
                width: Val::Percent(100.0 / PANEL_COLS),
                height: Val::Percent(100.0 / PANEL_ROWS),
                ..default()
            },
        ));
    }
}

fn spawn_panel(commands: &mut Commands, hud: &HudAssets, ui_camera: Entity) {
    commands
        .spawn((
            HelpUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(PANEL_LEFT_PCT),
                top: Val::Percent(PANEL_TOP_PCT),
                width: Val::Percent(PANEL_W_PCT),
                height: Val::Percent(PANEL_H_PCT),
                ..default()
            },
            UiTargetCamera(ui_camera),
            RenderLayers::layer(HUD_RENDER_LAYER),
        ))
        .with_children(|panel| {
            // Flat gray fill, stretched rather than tiled - see the main
            // menu for why a single stretched copy avoids seams.
            panel.spawn((
                font_image(hud, FONT_BACKGROUND_GRAY),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            panel_text(panel, hud, 6.0, 0.5, "HELP MENU");
            panel_text(panel, hud, 1.0, 2.5, "R)estart Level");
            panel_text(panel, hud, 1.0, 4.0, "Q)uit To Menu");
            panel_text(panel, hud, 1.0, 5.5, "ESC) Continue");
        });
}

/// Raised when the player picks "Restart Level"; handled separately so
/// neither system needs an unwieldy parameter list.
#[derive(Event)]
pub struct RestartLevel;

/// F1 opens the menu; while it's open the keys below act on it.
#[allow(clippy::too_many_arguments)]
pub fn help_menu_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    open: Query<Entity, With<HelpUi>>,
    mut next: ResMut<NextState<crate::screen::GameState>>,
    mut restart: EventWriter<RestartLevel>,
    mut opened_once: Local<bool>,
) {
    if !paused.0 {
        // COSMO_HELP=1 opens the menu on the first tick, so it can be
        // screenshot-verified in a headless run where keyboard focus is
        // unreliable.
        if !*opened_once && std::env::var("COSMO_HELP").is_ok() {
            *opened_once = true;
            paused.0 = true;
            spawn_panel(&mut commands, &hud, ui_camera.0);
            return;
        }
        if keys.just_pressed(KeyCode::F1) {
            paused.0 = true;
            spawn_panel(&mut commands, &hud, ui_camera.0);
        }
        return;
    }

    let chosen = keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::KeyQ)
        || keys.just_pressed(KeyCode::KeyR);
    if !chosen {
        return;
    }

    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;

    if keys.just_pressed(KeyCode::KeyQ) {
        next.set(crate::screen::GameState::Menu);
    } else if keys.just_pressed(KeyCode::KeyR) {
        restart.write(RestartLevel);
    }
}

/// Reloads the current level and rewinds to its entry snapshot - the same
/// rewind a death performs (game1.c:9821-9822 reloads on HELP_MENU_RESTART).
#[allow(clippy::too_many_arguments)]
pub fn handle_restart(
    mut commands: Commands,
    mut events: EventReader<RestartLevel>,
    asset_server: Res<AssetServer>,
    data: Res<GameData>,
    tileset: Option<Res<TilesetAssets>>,
    mut current: ResMut<CurrentLevel>,
    scoped: Query<Entity, With<LevelScoped>>,
    mut player_q: Query<&mut Player>,
    checkpoint: Res<Checkpoint>,
    mut score: ResMut<Score>,
    mut stars: ResMut<Stars>,
) {
    if events.read().next().is_none() {
        return;
    }
    let (Some(tileset), Ok(mut player)) = (tileset, player_q.single_mut()) else {
        return;
    };
    for entity in &scoped {
        commands.entity(entity).despawn();
    }
    let name = current.name.clone();
    if let Some(reloaded) =
        crate::flow::load_level_into_world(&mut commands, &asset_server, &data, &tileset, &name)
    {
        *current = reloaded;
    }
    if let Some(level) = data.load_level(&name) {
        let (sx, sy) = level::find_player_start(&level);
        player.x = sx as i32;
        player.y = sy as i32;
    }
    player.is_falling = true;
    player.jump_time = 0;
    player.fall_time = 0;
    player.cling_dir = None;
    player.dead_timer = 0;
    player.hurt_cooldown = 0;
    checkpoint.restore(&mut score, &mut stars, &mut player);
}

/// Closes the menu if the game is left while it's open.
pub fn close_help(mut commands: Commands, mut paused: ResMut<Paused>, open: Query<Entity, With<HelpUi>>) {
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
}
