//! Smooth motion between gameplay ticks.
//!
//! The game logic runs at 18.2Hz because that is the rate the DOS timer
//! interrupted at, and every physics constant is expressed per tick - the
//! jump curve is a table of ten per-tick offsets, walking is one tile per
//! tick, gravity is a per-tick counter. Raising the tick rate does not make
//! the game smoother, it makes Cosmo run three times faster.
//!
//! So the tick rate stays, and the *drawing* is decoupled from it instead.
//! Each tick records where things were before it ran; between ticks the
//! renderer draws them part-way along, using how far through the current
//! tick we are (`Time<Fixed>::overstep_fraction`). At 120Hz that turns one
//! 8-pixel jump every 55ms into a 1-2 pixel step every 8ms.
//!
//! Two things this deliberately does *not* do:
//!
//! - It does not interpolate to sub-pixel positions. The whole point of the
//!   320x200 virtual screen is that everything lands on a real pixel; a
//!   sprite drawn at x=100.4 samples its own texture off-grid and shimmers.
//!   Positions are rounded after interpolating, so motion gains steps
//!   without losing the pixel grid - and stays whole-pixel, which is what
//!   lets Scale3x run over the composited frame without flickering.
//! - It does not reduce input latency. Interpolating between the previous
//!   and current tick means drawing up to one tick behind the simulation,
//!   so it trades a little response for a lot of smoothness. Extrapolating
//!   forward instead would avoid that, but a tile-stepped game with hard
//!   collisions would visibly overshoot walls and snap back.

use crate::camera::Scroll;
use crate::enemy_ai::Enemy;
use crate::player::Player;
use crate::presentation::PresentationMode;
use bevy::prelude::*;
use bevy::time::Fixed;

/// Where things were before the current tick ran.
#[derive(Component, Clone, Copy, Default)]
pub struct PrevPos {
    pub x: i32,
    pub y: i32,
}

/// Snapshot of the scroll position before the current tick.
#[derive(Resource, Default, Clone, Copy)]
pub struct PrevScroll {
    pub x: i32,
    pub y: i32,
}

/// An explicit `COSMO_SMOOTH_MOTION` override, resolved once. This is a
/// run condition, so Bevy evaluates it every frame - reading the
/// environment here allocated a `String` per frame to answer a question
/// fixed at launch.
#[derive(Resource, Default)]
pub struct MotionOverride(pub Option<bool>);

impl MotionOverride {
    pub fn from_env() -> Self {
        MotionOverride(match std::env::var("COSMO_SMOOTH_MOTION").as_deref() {
            Ok("on") => Some(true),
            Ok("off") => Some(false),
            _ => None,
        })
    }
}

pub fn interpolation_enabled(mode: Res<PresentationMode>, over: Res<MotionOverride>) -> bool {
    // Part of the remastered feel rather than its own switch, and off in
    // authentic mode - where 18.2Hz motion is the point.
    over.0.unwrap_or(*mode == PresentationMode::Remaster)
}

/// Runs first in the tick, before anything moves.
pub fn snapshot_positions(
    mut player: Query<(&Player, &mut PrevPos), Without<Enemy>>,
    mut enemies: Query<(&Enemy, &mut PrevPos), Without<Player>>,
    scroll: Res<Scroll>,
    mut prev_scroll: ResMut<PrevScroll>,
) {
    for (p, mut prev) in &mut player {
        prev.x = p.x;
        prev.y = p.y;
    }
    for (e, mut prev) in &mut enemies {
        prev.x = e.x;
        prev.y = e.y;
    }
    prev_scroll.x = scroll.x;
    prev_scroll.y = scroll.y;
}

/// How far through the current tick we are, 0..1.
fn alpha(time: &Time<Fixed>) -> f32 {
    time.overstep_fraction().clamp(0.0, 1.0)
}

/// Interpolates in pixels, then rounds - see the module docs on why the
/// rounding matters.
fn lerp_px(prev_tiles: i32, now_tiles: i32, alpha: f32) -> f32 {
    let a = prev_tiles as f32 * crate::tileset::TILE_PX;
    let b = now_tiles as f32 * crate::tileset::TILE_PX;
    // A jump of more than a few tiles is a teleport - a respawn, a level
    // change, a death rewind - and must not be smeared across the screen.
    if (b - a).abs() > TELEPORT_PX {
        return b;
    }
    (a + (b - a) * alpha).round()
}

/// Beyond this, a position change is a teleport rather than movement. The
/// fastest legitimate motion is a couple of tiles per tick.
const TELEPORT_PX: f32 = 6.0 * crate::tileset::TILE_PX;

pub fn interpolate_player(
    time: Res<Time<Fixed>>,
    mut query: Query<(&Player, &PrevPos, &mut Transform)>,
) {
    let a = alpha(&time);
    for (p, prev, mut t) in &mut query {
        let top_row = p.y - (crate::player::PLAYER_HEIGHT - 1);
        let prev_top = prev.y - (crate::player::PLAYER_HEIGHT - 1);
        let half_w = crate::player::PLAYER_WIDTH as f32 * crate::tileset::TILE_PX / 2.0;
        let half_h = crate::player::PLAYER_HEIGHT as f32 * crate::tileset::TILE_PX / 2.0;
        t.translation.x = lerp_px(prev.x, p.x, a) + half_w;
        t.translation.y = -lerp_px(prev_top, top_row, a) - half_h;
        t.translation.z = 10.0;
    }
}

pub fn interpolate_enemies(
    time: Res<Time<Fixed>>,
    mut query: Query<(&Enemy, &PrevPos, &mut Transform)>,
) {
    let a = alpha(&time);
    for (e, prev, mut t) in &mut query {
        let top = e.y - (e.height_tiles - 1);
        let prev_top = prev.y - (e.height_tiles - 1);
        let half_w = e.width_tiles as f32 * crate::tileset::TILE_PX / 2.0;
        let half_h = e.height_tiles as f32 * crate::tileset::TILE_PX / 2.0;
        t.translation.x = lerp_px(prev.x, e.x, a) + half_w;
        t.translation.y = -lerp_px(prev_top, top, a) - half_h;
    }
}

pub fn interpolate_scroll(
    time: Res<Time<Fixed>>,
    scroll: Res<Scroll>,
    prev: Res<PrevScroll>,
    mut cam_q: Query<&mut Transform, With<crate::camera::GameCamera>>,
) {
    let Ok(mut cam_t) = cam_q.single_mut() else {
        return;
    };
    let a = alpha(&time);
    cam_t.translation.x =
        lerp_px(prev.x, scroll.x, a) + crate::presentation::PLAY_W as f32 / 2.0;
    cam_t.translation.y =
        -lerp_px(prev.y, scroll.y, a) - crate::presentation::PLAY_H as f32 / 2.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: f32 = 8.0;

    #[test]
    fn a_tick_is_walked_across_in_whole_pixels() {
        // One tile of movement, seen part-way through the tick, must land on
        // a real pixel - not 3.2 of one.
        for alpha in [0.0, 0.25, 0.4, 0.5, 0.75, 1.0] {
            let px = lerp_px(10, 11, alpha);
            assert_eq!(px, px.round(), "alpha {alpha} gave a sub-pixel position");
        }
    }

    #[test]
    fn the_ends_of_the_tick_are_exact() {
        assert_eq!(lerp_px(10, 11, 0.0), 80.0);
        assert_eq!(lerp_px(10, 11, 1.0), 88.0);
    }

    #[test]
    fn motion_advances_monotonically_through_the_tick() {
        let mut last = f32::NEG_INFINITY;
        for step in 0..=10 {
            let px = lerp_px(10, 11, step as f32 / 10.0);
            assert!(px >= last, "went backwards at alpha {}", step as f32 / 10.0);
            last = px;
        }
    }

    #[test]
    fn a_single_tile_step_is_broken_into_several_pixel_steps() {
        // The entire point: at 120Hz there are ~6 frames per tick, and they
        // should not all show the same position.
        let positions: std::collections::BTreeSet<i32> = (0..6)
            .map(|f| lerp_px(10, 11, f as f32 / 6.0) as i32)
            .collect();
        assert!(
            positions.len() >= 4,
            "only {} distinct positions across a tick: {positions:?}",
            positions.len()
        );
    }

    #[test]
    fn a_teleport_is_not_smeared_across_the_screen() {
        // Respawning, changing level or being rewound by a death moves the
        // player a long way in one tick; interpolating that would fly them
        // across the map.
        let far = lerp_px(10, 300, 0.5);
        assert_eq!(far, 300.0 * TILE, "teleport should snap, not glide");
        // ...but ordinary fast motion still interpolates.
        assert_ne!(lerp_px(10, 12, 0.5), 12.0 * TILE);
    }

    #[test]
    fn standing_still_stays_exactly_put() {
        for alpha in [0.0, 0.3, 0.9, 1.0] {
            assert_eq!(lerp_px(42, 42, alpha), 42.0 * TILE);
        }
    }
}
