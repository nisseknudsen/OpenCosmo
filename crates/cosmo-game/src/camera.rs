use crate::level::CurrentLevel;
use crate::player::Player;
use crate::tileset::TILE_PX;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

#[derive(Component)]
pub struct GameCamera;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            // Locks the visible area to a classic EGA-screen-ish extent
            // regardless of window size/DPI; Bevy letterboxes to fit.
            scaling_mode: ScalingMode::AutoMin {
                min_width: 160.0,
                min_height: 100.0,
            },
            ..OrthographicProjection::default_2d()
        }),
        GameCamera,
    ));
}

pub fn follow_player(
    player_q: Query<&Player>,
    level: Res<CurrentLevel>,
    mut cam_q: Query<(&mut Transform, &Projection), With<GameCamera>>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    let Ok((mut cam_t, projection)) = cam_q.single_mut() else {
        return;
    };
    // The clamp margin must match the *actual* rendered viewport, not a
    // guessed constant - a mismatch here either leaves dead space or (as
    // reported) lets the camera stop short, truncating the player/ground
    // out of frame at level sections shorter than the guessed margin.
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    let half_view_w = ortho.area.width() / 2.0;
    let half_view_h = ortho.area.height() / 2.0;

    let target_x = (player.x as f32 + 1.5) * TILE_PX;
    let target_y = -(player.y as f32 - 2.0) * TILE_PX;

    let min_x = level.content_min.0 as f32 * TILE_PX;
    let max_x = level.content_max.0 as f32 * TILE_PX;
    let min_y = -(level.content_max.1 as f32) * TILE_PX;
    let max_y = -(level.content_min.1 as f32) * TILE_PX;

    cam_t.translation.x = if max_x - min_x > half_view_w * 2.0 {
        target_x.clamp(min_x + half_view_w, max_x - half_view_w)
    } else {
        (min_x + max_x) / 2.0
    };
    cam_t.translation.y = if max_y - min_y > half_view_h * 2.0 {
        target_y.clamp(min_y + half_view_h, max_y - half_view_h)
    } else {
        (min_y + max_y) / 2.0
    };
}
