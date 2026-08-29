//! A developer level-warp menu: jump straight to any slot in the episode's
//! progression instead of playing up to it.
//!
//! Not a port of anything - the original's equivalent is a debug build's
//! warp mode, and its shipped cheat codes are elsewhere. It exists to make
//! the later levels reachable for testing without an hour of play, and is
//! reached from the F1 menu's "L)evel Warp".
//!
//! The list is the *progression*, not the set of level files: episode 1
//! visits `bonus1`/`bonus2` repeatedly, so warping to a named file alone
//! would lose the player's place in the sequence. Selecting entry `n` sets
//! the sequence index to `n`, so play continues correctly from there.

use crate::flow::{EnterLevel, LevelSequence};
use crate::help::Paused;
use crate::hud::HudAssets;
use crate::panel::TextFrame;
use crate::screen::UiCamera;
use bevy::prelude::*;

#[derive(Event)]
pub struct OpenLevelWarp;

#[derive(Component)]
pub struct LevelWarpUi;

/// Which entry the cursor is on. Kept across openings so re-warping while
/// iterating on one level is two keystrokes.
#[derive(Resource, Default)]
pub struct WarpCursor(pub usize);

/// Frame geometry. Two columns so a 23-slot episode fits above the status
/// bar rather than running underneath it.
const ROWS_PER_COLUMN: usize = 12;
const FRAME_TOP: i32 = 1;
const FRAME_WIDTH: i32 = 34;
const COLUMN_WIDTH: i32 = 16;
const FIRST_ROW: i32 = FRAME_TOP + 3;

fn warp_frame(sequence: &LevelSequence, cursor: usize) -> TextFrame {
    let mut frame = TextFrame::new(
        FRAME_TOP,
        ROWS_PER_COLUMN as i32 + 5,
        FRAME_WIDTH,
        "LEVEL WARP",
        "ENTER go   ESC back",
    );
    let x0 = frame.text_x();
    for (i, name) in sequence.order.iter().enumerate() {
        let column = i / ROWS_PER_COLUMN;
        let row = i % ROWS_PER_COLUMN;
        // Beyond two columns the frame would have to grow; episodes ship
        // far fewer slots than that, so clamp rather than scroll.
        if column > 1 {
            break;
        }
        let marker = if i == cursor { ">" } else { " " };
        frame = frame.line(
            x0 + column as i32 * COLUMN_WIDTH,
            FIRST_ROW + row as i32,
            &format!("{marker}{:2} {}", i + 1, name),
        );
    }
    frame
}

/// Steps the cursor by one entry, wrapping. Down/up move within a column,
/// left/right jump a whole column - which is how the two-column list reads.
pub fn step_cursor(cursor: usize, len: usize, dx: isize, dy: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let delta = dx * ROWS_PER_COLUMN as isize + dy;
    let len = len as isize;
    (((cursor as isize + delta) % len + len) % len) as usize
}

pub fn open_level_warp(
    mut commands: Commands,
    mut events: EventReader<OpenLevelWarp>,
    mut paused: ResMut<Paused>,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    sequence: Res<LevelSequence>,
    mut cursor: ResMut<WarpCursor>,
    mut opened_once: Local<bool>,
) {
    // COSMO_WARP=1 opens it on the first frame, for screenshot checks in a
    // headless run where keyboard focus is unreliable.
    let auto = !*opened_once && std::env::var("COSMO_WARP").is_ok();
    if auto {
        *opened_once = true;
    } else if events.read().next().is_none() {
        return;
    }
    cursor.0 = sequence.index.min(sequence.order.len().saturating_sub(1));
    paused.0 = true;
    warp_frame(&sequence, cursor.0).spawn(&mut commands, &hud, ui_camera.0, LevelWarpUi);
}

#[allow(clippy::too_many_arguments)]
pub fn level_warp_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    open: Query<Entity, With<LevelWarpUi>>,
    mut paused: ResMut<Paused>,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    mut sequence: ResMut<LevelSequence>,
    mut cursor: ResMut<WarpCursor>,
    mut enter: EventWriter<EnterLevel>,
) {
    if open.is_empty() {
        return;
    }
    let (dx, dy) = if keys.just_pressed(KeyCode::ArrowDown) {
        (0, 1)
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        (0, -1)
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        (1, 0)
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        (-1, 0)
    } else {
        (0, 0)
    };
    if (dx, dy) != (0, 0) {
        cursor.0 = step_cursor(cursor.0, sequence.order.len(), dx, dy);
        // Redraw: the frame is a static tree of glyph nodes, so moving the
        // cursor means rebuilding it. At this size that is cheaper than
        // keeping per-row entities addressable.
        for entity in &open {
            commands.entity(entity).despawn();
        }
        warp_frame(&sequence, cursor.0).spawn(&mut commands, &hud, ui_camera.0, LevelWarpUi);
        return;
    }

    let go = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    if !go && !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
    if go {
        sequence.index = cursor.0;
        // A developer warp skips the ceremony.
        enter.write(EnterLevel {
            level: sequence.current().to_string(),
        });
    }
}

pub fn close_level_warp(
    mut commands: Commands,
    open: Query<Entity, With<LevelWarpUi>>,
    mut paused: ResMut<Paused>,
) {
    for entity in &open {
        commands.entity(entity).despawn();
        paused.0 = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_wraps_both_ways() {
        assert_eq!(step_cursor(0, 23, 0, -1), 22);
        assert_eq!(step_cursor(22, 23, 0, 1), 0);
    }

    #[test]
    fn left_and_right_jump_a_whole_column() {
        assert_eq!(step_cursor(0, 23, 1, 0), ROWS_PER_COLUMN);
        assert_eq!(step_cursor(ROWS_PER_COLUMN, 23, -1, 0), 0);
    }

    #[test]
    fn an_empty_progression_does_not_panic() {
        assert_eq!(step_cursor(0, 0, 0, 1), 0);
    }
}
