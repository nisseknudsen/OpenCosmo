//! The shipped cheat code and the debug-mode key chords.
//!
//! `ProcessGameInputHelper` (game1.c:9755-9812). Both are chords - keys
//! held together rather than typed in sequence - which is why they are
//! read with `pressed` rather than `just_pressed`.

use bevy::prelude::*;

/// `usedCheatCode` (game1.c:69). The cheat fires once per game; the flag
/// travels into a save file, so a cheated game stays cheated.
#[derive(Resource, Default)]
pub struct CheatState {
    pub used: bool,
    /// `isDebugMode` (game1.c:77), toggled by its own chord.
    pub debug: bool,
    /// Held down so a chord does not retrigger every tick it is held.
    latched_cheat: bool,
    latched_debug: bool,
}

#[derive(Component)]
pub struct CheatUi;

/// What the cheat awards (game1.c:9805-9809).
pub const CHEAT_HEALTH_CELLS: u32 = 5;
pub const CHEAT_BOMBS: u32 = 9;
pub const CHEAT_HEALTH: i32 = 6;

/// `C` + `0` + `F10` held together, once per game (game1.c:9799).
pub fn cheat_chord_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::KeyC) && keys.pressed(KeyCode::Digit0) && keys.pressed(KeyCode::F10)
}

/// `Tab` + `F12` + `Del` toggles debug mode (game1.c:9762).
pub fn debug_chord_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::Tab) && keys.pressed(KeyCode::F12) && keys.pressed(KeyCode::Delete)
}

#[allow(clippy::too_many_arguments)]
pub fn cheat_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CheatState>,
    mut paused: ResMut<crate::help::Paused>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
    mut player_q: Query<&mut crate::player::Player>,
) {
    // Debug mode first: it is a toggle, so it needs its own latch.
    if debug_chord_held(&keys) {
        if !state.latched_debug {
            state.latched_debug = true;
            state.debug = !state.debug;
            sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::PAUSE_GAME));
            info!("debug mode {}", if state.debug { "on" } else { "off" });
        }
    } else {
        state.latched_debug = false;
    }

    if !cheat_chord_held(&keys) {
        state.latched_cheat = false;
        return;
    }
    if state.latched_cheat || state.used {
        return;
    }
    state.latched_cheat = true;
    state.used = true;

    if let Ok(mut player) = player_q.single_mut() {
        player.health_cells = CHEAT_HEALTH_CELLS;
        player.bombs = CHEAT_BOMBS;
        player.health = CHEAT_HEALTH;
    }
    sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::PAUSE_GAME));
    paused.0 = true;
    crate::panel::TextFrame::new(3, 9, 32, "You are now cheating!", "Press ANY key.")
        .text(6, "  You have been awarded full")
        .text(7, " health and maximum amount of")
        .text(8, "            bombs!")
        .spawn(&mut commands, &hud, ui_camera.0, CheatUi);
}

pub fn close_cheat_message(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<crate::help::Paused>,
    open: Query<Entity, With<CheatUi>>,
) {
    if open.is_empty() {
        return;
    }
    // Not while the chord that opened it is still held, or it would close
    // itself on the same keypress.
    if cheat_chord_held(&keys) || keys.get_just_pressed().next().is_none() {
        return;
    }
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(pressed: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut input = ButtonInput::default();
        for k in pressed {
            input.press(*k);
        }
        input
    }

    #[test]
    fn the_cheat_needs_all_three_keys_at_once() {
        assert!(cheat_chord_held(&held(&[
            KeyCode::KeyC,
            KeyCode::Digit0,
            KeyCode::F10
        ])));
        // Any two of them is not the code.
        assert!(!cheat_chord_held(&held(&[KeyCode::KeyC, KeyCode::Digit0])));
        assert!(!cheat_chord_held(&held(&[KeyCode::KeyC, KeyCode::F10])));
        assert!(!cheat_chord_held(&held(&[KeyCode::Digit0, KeyCode::F10])));
        assert!(!cheat_chord_held(&held(&[])));
    }

    #[test]
    fn the_debug_chord_is_a_different_three() {
        assert!(debug_chord_held(&held(&[
            KeyCode::Tab,
            KeyCode::F12,
            KeyCode::Delete
        ])));
        assert!(!debug_chord_held(&held(&[KeyCode::Tab, KeyCode::F12])));
        // ...and the two chords cannot be confused for each other.
        assert!(!debug_chord_held(&held(&[
            KeyCode::KeyC,
            KeyCode::Digit0,
            KeyCode::F10
        ])));
        assert!(!cheat_chord_held(&held(&[
            KeyCode::Tab,
            KeyCode::F12,
            KeyCode::Delete
        ])));
    }

    #[test]
    fn the_awards_are_the_ones_the_game_gives() {
        // game1.c:9805-9809.
        assert_eq!(CHEAT_HEALTH_CELLS, 5);
        assert_eq!(CHEAT_BOMBS, 9);
        assert_eq!(CHEAT_HEALTH, 6);
    }
}
