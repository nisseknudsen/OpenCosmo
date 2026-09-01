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

use crate::hud::HudAssets;
use crate::panel::TextFrame;
use crate::screen::UiCamera;
use bevy::prelude::*;

/// Set while a modal frame (this menu, or a hint globe's message) is open;
/// gameplay systems don't run.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}

#[derive(Component)]
pub struct HelpUi;

/// `UnfoldTextFrame(2, 12, 22, ...)` (game1.c:9630).
fn menu_frame() -> TextFrame {
    // The original's own list (game1.c:9630-9636), plus the level warp,
    // which is a development aid with no counterpart in the shipped game.
    TextFrame::new(2, 14, 22, "HELP MENU", "Press ESC to quit.")
        .text(5, " S)ave your game")
        .text(6, " R)estore a game")
        .text(7, " V)iew High Scores")
        .text(8, " T)ry level again")
        .text(9, " L)evel Warp")
        .text(10, " Q)uit Game")
}

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
    mut restart: EventWriter<crate::flow::RestartLevel>,
    mut warp: EventWriter<crate::devmenu::OpenLevelWarp>,
    mut opened_once: Local<bool>,
    mut slots: EventWriter<crate::savegame::OpenSlotPrompt>,
) {
    if !paused.0 {
        // COSMO_HELP=1 opens the menu on the first tick, so it can be
        // screenshot-verified in a headless run where keyboard focus is
        // unreliable.
        if !*opened_once && std::env::var("COSMO_HELP").is_ok() {
            *opened_once = true;
            paused.0 = true;
            menu_frame().spawn(&mut commands, &hud, ui_camera.0, HelpUi);
            return;
        }
        if keys.just_pressed(KeyCode::F1) {
            paused.0 = true;
            menu_frame().spawn(&mut commands, &hud, ui_camera.0, HelpUi);
        }
        return;
    }

    // Only act on our own keys - a hint globe's frame also sets `Paused`,
    // and it must not be closed by this system.
    if open.is_empty() {
        return;
    }
    let chosen = keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::KeyQ)
        || keys.just_pressed(KeyCode::KeyS)
        || keys.just_pressed(KeyCode::KeyR)
        || keys.just_pressed(KeyCode::KeyV)
        || keys.just_pressed(KeyCode::KeyT)
        || keys.just_pressed(KeyCode::KeyL);
    if !chosen {
        return;
    }

    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;

    if keys.just_pressed(KeyCode::KeyQ) {
        next.set(crate::screen::GameState::Menu);
    } else if keys.just_pressed(KeyCode::KeyT) {
        // "R)estart" moved to T, since R is the original's restore.
        restart.write(crate::flow::RestartLevel);
    } else if keys.just_pressed(KeyCode::KeyS) {
        slots.write(crate::savegame::OpenSlotPrompt(
            crate::savegame::SlotPrompt::Save,
        ));
    } else if keys.just_pressed(KeyCode::KeyR) {
        slots.write(crate::savegame::OpenSlotPrompt(
            crate::savegame::SlotPrompt::Restore,
        ));
    } else if keys.just_pressed(KeyCode::KeyV) {
        next.set(crate::screen::GameState::HighScores);
    } else if keys.just_pressed(KeyCode::KeyL) {
        warp.write(crate::devmenu::OpenLevelWarp);
    }
}

/// Closes the menu if the game is left while it's open.
pub fn close_help(
    mut commands: Commands,
    mut paused: ResMut<Paused>,
    open: Query<Entity, With<HelpUi>>,
) {
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
}
