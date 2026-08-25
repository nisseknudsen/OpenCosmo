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
    mut cam_q: Query<&mut Transform, With<GameCamera>>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    let Ok(mut cam_t) = cam_q.single_mut() else {
        return;
    };
    let target_x = (player.x as f32 + 1.5) * TILE_PX;
    let target_y = -(player.y as f32 - 2.0) * TILE_PX;

    let level_w = level.width as f32 * TILE_PX;
    let level_h = level.height as f32 * TILE_PX;
    let half_view_w = 160.0; // ~38 tiles at 8px, half-width
    let half_view_h = 90.0;

    cam_t.translation.x = if level_w > half_view_w * 2.0 {
        target_x.clamp(half_view_w, level_w - half_view_w)
    } else {
        level_w / 2.0
    };
    cam_t.translation.y = if level_h > half_view_h * 2.0 {
        target_y.clamp(-(level_h - half_view_h), -half_view_h)
    } else {
        -level_h / 2.0
    };
}
