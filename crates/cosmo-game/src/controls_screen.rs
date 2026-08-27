//! The controls screen, reached from the main menu.
//!
//! Laid out after the original's own `ShowKeyboardConfiguration`
//! (game2.c:1934-1990): a numbered list of the six actions with their
//! current binding shown to the right, pick a number to change one, ESC to
//! leave. The original had a separate joystick screen; here one screen
//! covers both, because a binding can be a key, a pad button or a stick
//! direction and rebinding just takes whatever you press next.

use crate::hud::HudAssets;
use crate::input::{first_pressed, save_bindings, Action, Bindings};
use crate::panel::TextFrame;
use crate::screen::{GameState, ScreenUi, UiCamera};
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

/// Which action is waiting for a new binding, if any.
#[derive(Resource, Default)]
pub struct Rebinding(pub Option<Action>);

#[derive(Component)]
pub struct ControlsUi;

/// `UnfoldTextFrame(3, 15, 27, "Keyboard Config.", "Press ESC to quit.")`
/// (game2.c:1942), widened to 34 to fit gamepad binding names alongside the
/// keyboard ones.
const FRAME_WIDTH: i32 = 34;
/// Column the binding is drawn at, relative to the frame's text column. The
/// original uses `x + 19` for a narrower frame and shorter labels.
const VALUE_COLUMN: i32 = 16;

fn frame(bindings: &Bindings, rebinding: Option<Action>) -> TextFrame {
    let mut f = TextFrame::new(3, 16, FRAME_WIDTH, "Controls", "Press ESC to quit.");
    let x = f.text_x();
    for (i, action) in Action::ALL.iter().enumerate() {
        let row = 6 + i as i32;
        f = f.line(x, row, &format!(" #{}) {} is:", i + 1, action.label()));
        let value = if rebinding == Some(*action) {
            "???".to_string()
        } else {
            bindings.summary(*action)
        };
        f = f.line(x + VALUE_COLUMN, row, &value);
    }
    if rebinding.is_some() {
        f = f.line(x, 14, "Press the new control");
    } else {
        f = f.line(x, 14, "Select # to change,");
        f = f.line(x, 15, "R) restore defaults");
    }
    f
}

fn redraw(
    commands: &mut Commands,
    open: &Query<Entity, With<ControlsUi>>,
    hud: &HudAssets,
    ui_camera: Entity,
    bindings: &Bindings,
    rebinding: Option<Action>,
) {
    for entity in open.iter() {
        commands.entity(entity).despawn();
    }
    frame(bindings, rebinding).spawn(commands, hud, ui_camera, (ControlsUi, ScreenUi));
}

pub fn spawn_controls(
    mut commands: Commands,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    bindings: Res<Bindings>,
    mut rebinding: ResMut<Rebinding>,
    screen: Res<crate::presentation::VirtualScreen>,
) {
    rebinding.0 = None;
    crate::screen::spawn_state_camera(&mut commands, &screen);
    frame(&bindings, None).spawn(&mut commands, &hud, ui_camera.0, (ControlsUi, ScreenUi));
}

#[allow(clippy::too_many_arguments)]
pub fn controls_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut bindings: ResMut<Bindings>,
    mut rebinding: ResMut<Rebinding>,
    mut next: ResMut<NextState<GameState>>,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    open: Query<Entity, With<ControlsUi>>,
) {
    // Waiting for a control to assign.
    if let Some(action) = rebinding.0 {
        if keys.just_pressed(KeyCode::Escape) {
            rebinding.0 = None;
            redraw(&mut commands, &open, &hud, ui_camera.0, &bindings, None);
            return;
        }
        if let Some(binding) = first_pressed(&keys, &pads) {
            bindings.rebind(action, binding);
            save_bindings(&bindings);
            rebinding.0 = None;
            redraw(&mut commands, &open, &hud, ui_camera.0, &bindings, None);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Menu);
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *bindings = Bindings::default();
        save_bindings(&bindings);
        redraw(&mut commands, &open, &hud, ui_camera.0, &bindings, None);
        return;
    }

    const DIGITS: [KeyCode; 6] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    for (i, digit) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*digit) {
            let action = Action::ALL[i];
            rebinding.0 = Some(action);
            redraw(
                &mut commands,
                &open,
                &hud,
                ui_camera.0,
                &bindings,
                Some(action),
            );
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_gets_a_numbered_row() {
        let f = frame(&Bindings::default(), None);
        // Six actions on rows 6..11, as the original lays them out.
        assert_eq!(Action::ALL.len(), 6);
        assert_eq!(f.top, 3);
        assert!(f.height >= 6 + 6, "frame too short for six rows plus footer");
    }

    #[test]
    fn the_value_column_leaves_room_for_the_widest_binding() {
        // The frame spans `left..left + width`; a binding starts at
        // text_x + VALUE_COLUMN and may not run past the right border.
        let f = frame(&Bindings::default(), None);
        let available = (f.left() + f.width - 1) - (f.text_x() + VALUE_COLUMN);
        assert!(
            available as usize >= crate::input::CONTROLS_VALUE_WIDTH,
            "value column has {available} tiles, needs {}",
            crate::input::CONTROLS_VALUE_WIDTH
        );
    }

    #[test]
    fn no_row_of_the_frame_overflows_its_border() {
        let bindings = Bindings::default();
        let f = frame(&bindings, None);
        let right = f.left() + f.width - 1;
        for (x, row, text) in f.lines_for_test() {
            assert!(
                x + text.chars().count() as i32 <= right,
                "row {row} ({text:?}) runs past the frame's right edge"
            );
        }
    }

    #[test]
    fn the_pending_action_is_shown_as_waiting_rather_than_its_old_value() {
        let bindings = Bindings::default();
        let normal = format!("{:?}", frame(&bindings, None).lines_for_test());
        let waiting = format!("{:?}", frame(&bindings, Some(Action::Jump)).lines_for_test());
        assert!(waiting.contains("???"));
        assert!(!normal.contains("???"));
    }
}
