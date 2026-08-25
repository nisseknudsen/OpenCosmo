use crate::level::CurrentLevel;
use crate::player::Player;
use crate::tileset::TILE_PX;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

/// Size of the scrolling game window in tiles (def.h:138-139).
const SCROLL_W: i32 = 38;
const SCROLL_H: i32 = 18;

#[derive(Component)]
pub struct GameCamera;

pub fn spawn_camera(commands: &mut Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            // Locks the visible area to a classic EGA-screen-ish extent
            // regardless of window size/DPI; Bevy letterboxes to fit.
            // Exactly the original's game window: SCROLLW x SCROLLH =
            // 38x18 tiles (def.h:138-139) = 304x144 px. Matching this
            // matters beyond framing - the backdrop images are 40x18
            // tiles (320x144) and are meant to fill this window exactly,
            // so a mismatched viewport height puts the horizon at the
            // wrong place.
            scaling_mode: ScalingMode::AutoMin {
                min_width: 304.0,
                min_height: 144.0,
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

    // Work in the original's own terms: a scroll position measured in
    // whole tiles for the top-left of the game window, clamped exactly the
    // way the original clamps it, then converted back to a camera centre.
    //
    // The vertical limit is `maxScrollY = 0x10000 / (mapWidth * 2) -
    // (SCROLLH + 1)` (game1.c:10334). That 0x10000 is the 64KB map buffer,
    // so `0x10000 / (mapWidth * 2)` is simply how many rows fit in it -
    // the map's real height, which is *not* the same as the bounding box
    // of non-empty tiles we were clamping to before. Using the bounding
    // box let the view slip below the map's last row, exposing a strip of
    // bare backdrop between the ground and the status bar.
    let map_w = level.width.max(1) as i32;
    let map_h = (0x10000 / (map_w * 2)).max(1);
    let max_scroll_x = (map_w - SCROLL_W).max(0);
    let max_scroll_y = (map_h - (SCROLL_H + 1)).max(0);

    let scroll_x = (player.x - SCROLL_W / 2).clamp(0, max_scroll_x) as f32;
    let scroll_y = (player.y - SCROLL_H / 2).clamp(0, max_scroll_y) as f32;

    cam_t.translation.x = scroll_x * TILE_PX + half_view_w;
    cam_t.translation.y = -(scroll_y * TILE_PX) - half_view_h;
}
