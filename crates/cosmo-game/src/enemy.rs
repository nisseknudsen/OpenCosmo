//! Basic enemy behavior: a curated hazard list (damages the player on
//! contact) and a "walker" subset of it (patrols left/right, reversing at
//! walls or ledges). This is a pragmatic first pass, not a per-type port of
//! each ACT_*'s real behavior function (`ActXxx` in game1.c) - there are
//! ~250 actor types each with their own hand-written behavior in the
//! original, and porting them individually is a much larger undertaking.
//! The ACT_* ids below were curated by eye from actor.h's names (creature/
//! trap-sounding entries), not extracted from source logic - expect some
//! misclassification until a real per-type behavior port happens.

use crate::data::{
    GameData, LevelJson, TILE_ATTR_BLOCK_EAST, TILE_ATTR_BLOCK_SOUTH, TILE_ATTR_BLOCK_WEST,
};
use crate::level::{tile_topleft_to_center, CurrentLevel};
use crate::player::Player;
use bevy::prelude::*;

/// ACT_* ids (map_type - 31) that hurt the player on contact.
pub const HAZARD_ACT_IDS: &[u16] = &[
    17, 18, 20, 22, 24, 25, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 63, 65, 66, 67, 68,
    69, 78, 80, 83, 84, 87, 88, 89, 90, 92, 95, 96, 101, 102, 106, 109, 110, 111, 112, 113, 118,
    124, 126, 127, 128, 129, 151, 152, 162, 187, 233, 234, 236, 237, 251,
];

/// Subset of the above that patrols left/right instead of standing still.
pub const WALKER_ACT_IDS: &[u16] = &[
    25, 42, 43, 51, 65, 69, 78, 80, 101, 106, 118, 124, 126, 127, 128, 129, 187, 236, 237, 251,
];

#[derive(Component)]
pub struct Hazard;

#[derive(Component)]
pub struct Walker {
    pub x: i32,
    pub y: i32,
    pub dir: i32,
    pub width_tiles: i32,
    pub height_tiles: i32,
}

fn attr_at(level: &LevelJson, data: &GameData, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 {
        return 0;
    }
    data.tile_attr(level.tile_at(x as usize, y as usize))
}

pub fn move_walkers(
    mut query: Query<(&mut Walker, &mut Transform)>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
    mut tick: Local<u32>,
) {
    *tick += 1;
    if *tick % 3 != 0 {
        return; // enemies pace slower than the player
    }
    let Some(level) = data.load_level(&level_data.name) else {
        return;
    };
    for (mut w, mut t) in &mut query {
        let new_x = w.x + w.dir;
        let edge_x = if w.dir > 0 {
            new_x + w.width_tiles - 1
        } else {
            new_x
        };
        let block_flag = if w.dir > 0 {
            TILE_ATTR_BLOCK_EAST
        } else {
            TILE_ATTR_BLOCK_WEST
        };
        let mut blocked = false;
        for dy in 0..w.height_tiles {
            let row = w.y - dy;
            if attr_at(&level, &data, edge_x, row) & block_flag != 0 {
                blocked = true;
                break;
            }
        }
        let has_ground = attr_at(&level, &data, edge_x, w.y + 1) & TILE_ATTR_BLOCK_SOUTH != 0;
        if blocked || !has_ground {
            w.dir = -w.dir;
        } else {
            w.x = new_x;
        }
        let top_row = w.y - (w.height_tiles - 1);
        let world = tile_topleft_to_center(
            w.x as f32,
            top_row as f32,
            w.width_tiles as f32 * 8.0,
            w.height_tiles as f32 * 8.0,
        );
        t.translation.x = world.x;
        t.translation.y = world.y;
        t.scale.x = w.dir as f32;
    }
}

pub fn hazard_damage(
    mut player_q: Query<&mut Player>,
    static_hazards: Query<&Transform, (With<Hazard>, Without<Walker>)>,
    walker_hazards: Query<(&Walker, &Transform), With<Hazard>>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.hurt_cooldown > 0 {
        player.hurt_cooldown -= 1;
        return;
    }
    let player_center = tile_topleft_to_center(
        player.x as f32,
        (player.y - crate::player::PLAYER_HEIGHT + 1) as f32,
        crate::player::PLAYER_WIDTH as f32 * 8.0,
        crate::player::PLAYER_HEIGHT as f32 * 8.0,
    );
    let mut touched = false;
    for t in &static_hazards {
        if (t.translation.x - player_center.x).abs() < 16.0
            && (t.translation.y - player_center.y).abs() < 20.0
        {
            touched = true;
            break;
        }
    }
    if !touched {
        for (_, t) in &walker_hazards {
            if (t.translation.x - player_center.x).abs() < 16.0
                && (t.translation.y - player_center.y).abs() < 20.0
            {
                touched = true;
                break;
            }
        }
    }
    if !touched {
        return;
    }

    player.health -= 1;
    player.hurt_cooldown = 44; // matches HurtPlayer()'s cooldown, game1.c:6927
    if player.health <= 0 {
        let Some(level) = data.load_level(&level_data.name) else {
            return;
        };
        let (sx, sy) = crate::level::find_player_start(&level);
        player.x = sx as i32;
        player.y = sy as i32;
        player.is_falling = true;
        player.jump_time = 0;
        player.fall_time = 0;
        player.cling_dir = None;
        player.health = 4; // starting health, game1.c:10580
    }
}
