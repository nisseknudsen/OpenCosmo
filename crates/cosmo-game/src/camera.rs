//! The scrolling game window.
//!
//! This is deliberately *stateful*. It used to recompute a centred camera
//! from the player's position every frame, which looks superficially right
//! but cannot express the original's camera at all: there the view is a
//! persistent `scrollX`/`scrollY` pair, nudged one tile at a time by
//! whatever happens during a tick, and clamped afterwards. Several
//! mechanics are *defined* as nudges to that pair and simply have nowhere
//! to live under a recomputed camera - looking up and down being the one
//! that was reported missing.

use crate::data::LevelJson;
use crate::level::CurrentLevel;
use crate::player::Player;
use crate::tileset::TILE_PX;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

/// Size of the scrolling game window in tiles (def.h:138-139).
pub const SCROLL_W: i32 = 38;
pub const SCROLL_H: i32 = 18;

/// The top-left of the game window, in map tiles.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Scroll {
    pub x: i32,
    pub y: i32,
}

/// The map's real height in rows.
///
/// `maxScrollY = 0x10000 / (mapWidth * 2) - (SCROLLH + 1)` (game1.c:10334).
/// The 0x10000 is the 64KB map buffer and the `* 2` its 16-bit cells, so
/// `0x10000 / (mapWidth * 2)` is however many rows fit in it - the map's
/// true height, which is *not* the bounding box of non-empty tiles.
pub fn map_rows(map_width: usize) -> i32 {
    (0x10000 / (map_width.max(1) as i32 * 2)).max(1)
}

pub fn max_scroll_y(map_width: usize) -> i32 {
    (map_rows(map_width) - (SCROLL_H + 1)).max(0)
}

impl Scroll {
    /// `if (scrollY > maxScrollY) scrollY = maxScrollY` (game1.c:863), plus
    /// the horizontal equivalent the scroll nudges enforce inline.
    pub fn clamp_to(&mut self, map_width: usize) {
        self.x = self.x.clamp(0, (map_width as i32 - SCROLL_W).max(0));
        self.y = self.y.clamp(0, max_scroll_y(map_width));
    }

    /// The initial framing `SPA_PLAYER_START` sets up (game1.c:10195-10209):
    /// the player sits 15 columns in and 10 rows down, not centred.
    pub fn centre_on(&mut self, player: &Player, level: &CurrentLevel) {
        let map_w = level.width as i32;
        self.x = if player.x > map_w - 15 {
            map_w - SCROLL_W
        } else if player.x - 15 >= 0 && map_w > 32 {
            player.x - 15
        } else {
            0
        };
        self.y = (player.y - 10).max(0);
        self.clamp_to(level.width);
    }

    /// The scroll-follow tail of `MovePlayer` (game1.c:8933-8949).
    ///
    /// The player is not centred: the view only gives chase once they leave
    /// a dead zone spanning rows 7..SCROLLH-4 and columns 12..SCROLLW-15.
    /// `mapYPower > 5` is the original's way of saying "wider than 32
    /// tiles"; narrower maps never scroll horizontally.
    pub fn follow(&mut self, player: &Player, level: &LevelJson, cling_slip: bool) {
        let map_w = level.width as i32;
        if player.y - self.y > SCROLL_H - 4 {
            self.y += 1;
        }
        if cling_slip && player.y - self.y > SCROLL_H - 4 {
            self.y += 1;
        } else {
            // A high recoil (a fresh pounce) scrolls twice as fast, so the
            // view keeps up with the launch.
            if player.recoil_left > 10 && player.y - self.y < 7 && self.y > 0 {
                self.y -= 1;
            }
            if player.y - self.y < 7 && self.y > 0 {
                self.y -= 1;
            }
        }
        if player.x - self.x > SCROLL_W - 15 && map_w - SCROLL_W > self.x && map_w > 32 {
            self.x += 1;
        } else if player.x - self.x < 12 && self.x > 0 {
            self.x -= 1;
        }
        self.clamp_to(level.width);
    }
}

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

/// Places the camera at whatever the tick decided the scroll should be.
/// Pure presentation - all the movement logic lives in `Scroll`.
pub fn apply_scroll(
    scroll: Res<Scroll>,
    mut cam_q: Query<(&mut Transform, &Projection), With<GameCamera>>,
) {
    let Ok((mut cam_t, projection)) = cam_q.single_mut() else {
        return;
    };
    // The offset must come from the *actual* rendered viewport rather than
    // a guessed constant - a mismatch either leaves dead space or lets the
    // view stop short, truncating the ground out of frame.
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    cam_t.translation.x = scroll.x as f32 * TILE_PX + ortho.area.width() / 2.0;
    cam_t.translation.y = -(scroll.y as f32 * TILE_PX) - ortho.area.height() / 2.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(width: usize) -> LevelJson {
        LevelJson {
            name: "test".into(),
            width,
            height: 64,
            tiles: vec![0; width * 64],
            actors: Vec::new(),
            backdrop: None,
            music: None,
            has_h_scroll_backdrop: false,
            has_v_scroll_backdrop: false,
        }
    }

    fn player_at(x: i32, y: i32) -> Player {
        Player::spawn_at(x, y)
    }

    #[test]
    fn the_view_does_not_chase_inside_the_dead_zone() {
        let lvl = level(128);
        let mut scroll = Scroll { x: 40, y: 20 };
        // Column 12..23 and row 7..14 relative to the scroll are "resting".
        let p = player_at(40 + 15, 20 + 10);
        let before = scroll;
        scroll.follow(&p, &lvl, false);
        assert_eq!((scroll.x, scroll.y), (before.x, before.y));
    }

    #[test]
    fn the_view_chases_once_the_player_leaves_it() {
        let lvl = level(128);
        let mut scroll = Scroll { x: 40, y: 20 };
        scroll.follow(&player_at(40 + 30, 20 + 10), &lvl, false);
        assert_eq!(scroll.x, 41, "player past the right edge pulls the view east");

        let mut scroll = Scroll { x: 40, y: 20 };
        scroll.follow(&player_at(40 + 2, 20 + 10), &lvl, false);
        assert_eq!(scroll.x, 39, "and past the left edge pulls it back west");

        let mut scroll = Scroll { x: 40, y: 20 };
        scroll.follow(&player_at(40 + 15, 20 + 17), &lvl, false);
        assert_eq!(scroll.y, 21);
    }

    #[test]
    fn narrow_maps_never_scroll_horizontally() {
        // mapYPower > 5 gates the east nudge, and SCROLLW already exceeds
        // a 32-tile map's width, so there is nothing to scroll to.
        let lvl = level(32);
        let mut scroll = Scroll { x: 0, y: 20 };
        scroll.follow(&player_at(31, 20 + 10), &lvl, false);
        assert_eq!(scroll.x, 0);
    }

    #[test]
    fn scrolling_stops_at_the_bottom_of_the_map() {
        let lvl = level(128);
        let limit = max_scroll_y(128);
        let mut scroll = Scroll { x: 0, y: limit };
        scroll.follow(&player_at(15, limit + 17), &lvl, false);
        assert_eq!(scroll.y, limit, "must not expose backdrop below the last row");
    }

    #[test]
    fn the_start_framing_puts_the_player_off_centre() {
        let lvl = CurrentLevel {
            name: "test".into(),
            width: 128,
            height: 64,
            content_min: (0, 0),
            content_max: (127, 63),
            music: None,
        };
        let mut scroll = Scroll::default();
        scroll.centre_on(&player_at(60, 40), &lvl);
        assert_eq!((scroll.x, scroll.y), (45, 30));
    }
}
