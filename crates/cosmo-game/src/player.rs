//! Player movement, faithfully ported (core cases only - bombs, damage
//! recoil, ice-slide application, dizzy/scooter states are not yet ported)
//! from cosmore's `MovePlayer()` (game1.c:8438-8822) and `TestPlayerMove()`
//! (game1.c:1022-1127). Movement is tile-grid stepped, not sub-pixel, driven
//! by a fixed-timestep tick matching the original's ~18.2Hz PC timer.

use crate::data::{
    GameData, TILE_ATTR_BLOCK_EAST, TILE_ATTR_BLOCK_NORTH, TILE_ATTR_BLOCK_SOUTH,
    TILE_ATTR_BLOCK_WEST, TILE_ATTR_CAN_CLING, TILE_ATTR_SLOPED,
};
use crate::level::{tile_topleft_to_center, CurrentLevel};
use crate::tileset::TILE_PX;
use bevy::prelude::*;

pub const PLAYER_WIDTH: i32 = 3;
pub const PLAYER_HEIGHT: i32 = 5;
const JUMP_TABLE: [i32; 10] = [-2, -1, -1, -1, -1, -1, -1, 0, 0, 0];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FaceDir {
    West,
    East,
}

#[derive(PartialEq, Eq)]
enum MoveResult {
    Free,
    Blocked,
    Sloped,
}

#[derive(Component)]
pub struct Player {
    pub x: i32, // leftmost column
    pub y: i32, // bottommost row
    pub face_dir: FaceDir,
    pub jump_time: u32,
    pub fall_time: u32,
    pub is_falling: bool,
    pub cling_dir: Option<FaceDir>,
    pub cmd_jump_latch: bool,
    pub can_cling: bool,
    pub frame: usize,
    pub anim_timer: f32,
    pub on_ground: bool,
    /// Starting value 4 (game1.c:10580). HurtPlayer() decrements on
    /// contact damage; 0 triggers a respawn (our simplified stand-in for
    /// death) in enemy.rs::hazard_damage.
    pub health: i32,
    /// Invincibility frames after taking damage - 44 ticks matches the
    /// original's HurtPlayer() (game1.c:6927).
    pub hurt_cooldown: u32,
    /// 0 = alive. >0 = playing the death animation (PLAYER_DEAD_1/2,
    /// floating upward), counting ticks - mirrors playerDeadTime
    /// (game1.c:9230-9256), which respawns once it passes 36.
    pub dead_timer: u32,
}

impl Player {
    pub fn spawn_at(x: i32, y: i32) -> Self {
        Player {
            x,
            y,
            face_dir: FaceDir::East,
            jump_time: 0,
            fall_time: 0,
            is_falling: true,
            cling_dir: None,
            cmd_jump_latch: false,
            can_cling: false,
            frame: 0,
            anim_timer: 0.0,
            on_ground: false,
            health: 4,
            hurt_cooldown: 0,
            dead_timer: 0,
        }
    }
}

#[derive(Resource, Default)]
pub struct PlayerInput {
    pub west: bool,
    pub east: bool,
    pub jump: bool,
    pub look_up: bool,
    pub look_down: bool,
}

pub fn read_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<PlayerInput>, time: Res<Time>) {
    if std::env::var("COSMO_AUTOPLAY").is_ok() {
        input.west = false;
        input.east = true;
        input.jump = (time.elapsed_secs() as u32) % 2 == 0;
        return;
    }
    input.west = keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA);
    input.east = keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD);
    input.jump = keys.pressed(KeyCode::Space);
    input.look_up = keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW);
    input.look_down = keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS);
}

fn attr_at(level: &crate::data::LevelJson, data: &GameData, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 {
        return 0;
    }
    data.tile_attr(level.tile_at(x as usize, y as usize))
}

/// Faithful port of TestPlayerMove; `can_cling` is an out-param mirroring
/// the original's side-effecting global write during WEST/EAST checks.
fn test_move(
    dir: Direction,
    x: i32,
    y: i32,
    level: &crate::data::LevelJson,
    data: &GameData,
    can_cling: &mut bool,
) -> MoveResult {
    match dir {
        Direction::North => {
            if y - 3 == 0 || y - 2 == 0 {
                return MoveResult::Blocked;
            }
            let row = y - 4;
            for i in 0..PLAYER_WIDTH {
                if attr_at(level, data, x + i, row) & TILE_ATTR_BLOCK_NORTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Direction::South => {
            let row = y;
            for i in 0..PLAYER_WIDTH {
                let a = attr_at(level, data, x + i, row);
                if a & TILE_ATTR_SLOPED != 0 {
                    return MoveResult::Sloped;
                }
                if a & TILE_ATTR_BLOCK_SOUTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Direction::West => {
            *can_cling = attr_at(level, data, x, y - 2) & TILE_ATTR_CAN_CLING != 0;
            for i in 0..PLAYER_HEIGHT {
                let row = y - i;
                let a = attr_at(level, data, x, row);
                if a & TILE_ATTR_BLOCK_WEST != 0 {
                    return MoveResult::Blocked;
                }
                if i == 0
                    && a & TILE_ATTR_SLOPED != 0
                    && attr_at(level, data, x, row - 1) & TILE_ATTR_BLOCK_WEST == 0
                {
                    return MoveResult::Sloped;
                }
            }
            MoveResult::Free
        }
        Direction::East => {
            *can_cling = attr_at(level, data, x + 2, y - 2) & TILE_ATTR_CAN_CLING != 0;
            for i in 0..PLAYER_HEIGHT {
                let row = y - i;
                let a = attr_at(level, data, x + 2, row);
                if a & TILE_ATTR_BLOCK_EAST != 0 {
                    return MoveResult::Blocked;
                }
                if i == 0
                    && a & TILE_ATTR_SLOPED != 0
                    && attr_at(level, data, x + 2, row - 1) & TILE_ATTR_BLOCK_EAST == 0
                {
                    return MoveResult::Sloped;
                }
            }
            MoveResult::Free
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    North,
    South,
    West,
    East,
}

pub fn move_player_tick(
    mut query: Query<&mut Player>,
    input: Res<PlayerInput>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer != 0 {
        return; // frozen during the death animation, game1.c:8452
    }
    let Some(level) = data.load_level(&level_data.name) else {
        return;
    };
    let mut dummy_cling = false;

    // --- Horizontal movement ---
    // NOTE: the original (game1.c:8606-8687) moves speculatively first
    // (decrements playerX immediately when already facing the movement
    // direction) then tests collision at the *new* position, reverting on
    // block - and has a documented uninitialized-variable bug in its slope
    // handling along that path. This is a test-before-move rewrite that
    // reaches the same outcome (blocked at walls, free otherwise, steps up
    // one row on a slope) without replicating that UB; worth revisiting if
    // a one-tile boundary discrepancy against the original ever matters.
    if input.west && p.cling_dir.is_none() && !input.east {
        if p.face_dir == FaceDir::West {
            if p.x > 0 {
                let m = test_move(Direction::West, p.x, p.y, &level, &data, &mut p.can_cling);
                if m == MoveResult::Blocked {
                    if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                        == MoveResult::Free
                        && p.can_cling
                    {
                        p.cling_dir = Some(FaceDir::West);
                        p.is_falling = false;
                        p.jump_time = 0;
                        p.fall_time = 0;
                        p.cmd_jump_latch = input.jump;
                    }
                } else {
                    p.x -= 1;
                    if m == MoveResult::Sloped {
                        p.y -= 1;
                    }
                }
            }
        } else {
            p.face_dir = FaceDir::West;
        }
    }
    if input.east && p.cling_dir.is_none() && !input.west {
        if p.face_dir == FaceDir::East {
            let m = test_move(Direction::East, p.x, p.y, &level, &data, &mut p.can_cling);
            if m == MoveResult::Blocked {
                if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                    == MoveResult::Free
                    && p.can_cling
                {
                    p.cling_dir = Some(FaceDir::East);
                    p.is_falling = false;
                    p.jump_time = 0;
                    p.fall_time = 0;
                    p.cmd_jump_latch = input.jump;
                }
            } else {
                p.x += 1;
                if m == MoveResult::Sloped {
                    p.y -= 1;
                }
            }
        } else {
            p.face_dir = FaceDir::East;
        }
    }

    if p.cling_dir.is_some() && p.cmd_jump_latch && !input.jump {
        p.cmd_jump_latch = false;
    }

    // --- Jump initiation / continuation ---
    let jumping_now = (input.jump && !p.is_falling && !p.cmd_jump_latch)
        || (p.cling_dir.is_some() && input.jump && !p.cmd_jump_latch);
    if jumping_now {
        if p.cling_dir == Some(FaceDir::West) && input.west {
            p.cling_dir = None;
        }
        if p.cling_dir == Some(FaceDir::East) && input.east {
            p.cling_dir = None;
        }
        if p.cling_dir.is_none() {
            let jt = (p.jump_time as usize).min(JUMP_TABLE.len() - 1);
            p.y = (p.y as i32 + JUMP_TABLE[jt]).max(0);
        }
        p.cling_dir = None;

        if test_move(Direction::North, p.x, p.y, &level, &data, &mut dummy_cling)
            != MoveResult::Free
        {
            p.y += 1;
            p.is_falling = true;
            if input.jump {
                p.cmd_jump_latch = true;
            }
            p.fall_time = 0;
        }
        if p.jump_time + 1 > 6 {
            p.is_falling = true;
            if input.jump {
                p.cmd_jump_latch = true;
            }
            p.fall_time = 0;
        }
        p.jump_time += 1;
    }

    // --- Gravity / falling ---
    if p.cling_dir.is_none() {
        if p.is_falling && input.jump {
            p.cmd_jump_latch = true;
        }
        if (!input.jump || p.cmd_jump_latch) && !p.is_falling {
            p.is_falling = true;
            p.fall_time = 0;
        }
        p.on_ground = false;
        if p.is_falling {
            p.y += 1;
            if test_move(Direction::South, p.x, p.y, &level, &data, &mut dummy_cling)
                != MoveResult::Free
            {
                p.is_falling = false;
                p.on_ground = true;
                p.y -= 1;
                p.jump_time = 0;
                p.cmd_jump_latch = input.jump;
                p.fall_time = 0;
            }
            if p.is_falling && p.fall_time > 3 {
                p.y += 1;
                if test_move(Direction::South, p.x, p.y, &level, &data, &mut dummy_cling)
                    != MoveResult::Free
                {
                    p.is_falling = false;
                    p.on_ground = true;
                    p.y -= 1;
                    p.jump_time = 0;
                    p.cmd_jump_latch = input.jump;
                    p.fall_time = 0;
                }
            }
            if p.fall_time < 25 {
                p.fall_time += 1;
            }
        }
    }

    // Safety net: nothing in TestPlayerMove bounds a fall below the map
    // buffer or a bottomless hazard (e.g. deep water with no BLOCK_SOUTH
    // tiles), so without this the player can fall forever. Respawn at the
    // level's start once clearly past the bottom of the playable content.
    if p.y as i64 > level_data.content_max.1 as i64 + 20 {
        let (sx, sy) = crate::level::find_player_start(&level);
        p.x = sx as i32;
        p.y = sy as i32;
        p.is_falling = true;
        p.jump_time = 0;
        p.fall_time = 0;
        p.cling_dir = None;
    }
}

/// Plays the death animation (frozen movement, alternating DEAD frames,
/// floating upward) then respawns at the level start - a simplified stand-in
/// for the original's checkpoint-reload (`LoadGameState('T')`); we don't
/// have a save-state system, so "checkpoint" is just "level start" here.
/// Timing mirrors game1.c:9230-9256: ~10 ticks stationary, then floats up
/// until tick 36 triggers the reload.
pub fn update_death(
    mut query: Query<&mut Player>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer == 0 {
        return;
    }
    p.dead_timer += 1;
    if p.dead_timer > 10 {
        p.y -= 1;
    }
    if p.dead_timer > 36 {
        let Some(level) = data.load_level(&level_data.name) else {
            return;
        };
        let (sx, sy) = crate::level::find_player_start(&level);
        p.x = sx as i32;
        p.y = sy as i32;
        p.is_falling = true;
        p.jump_time = 0;
        p.fall_time = 0;
        p.cling_dir = None;
        p.health = 4;
        p.hurt_cooldown = 0;
        p.dead_timer = 0;
    }
}

pub fn sync_transform(mut query: Query<(&Player, &mut Transform)>) {
    for (p, mut t) in &mut query {
        let top_row = p.y - (PLAYER_HEIGHT - 1);
        let world = tile_topleft_to_center(
            p.x as f32,
            top_row as f32,
            PLAYER_WIDTH as f32 * TILE_PX,
            PLAYER_HEIGHT as f32 * TILE_PX,
        );
        t.translation.x = world.x;
        t.translation.y = world.y;
        t.translation.z = 10.0;
    }
}

// Named frame offsets within one facing direction's 23-frame block
// (player.h: PLAYER_BASE_WEST=0, PLAYER_BASE_EAST=23, frames 0..22 per side).
mod frame {
    pub const WALK: [usize; 4] = [0, 1, 2, 3];
    pub const STAND: usize = 4;
    pub const LOOK_NORTH: usize = 5;
    pub const LOOK_SOUTH: usize = 6;
    pub const JUMP: usize = 7;
    pub const FALL: usize = 8;
    pub const CLING: usize = 9;
    pub const FALL_LONG: usize = 13;
    pub const PAIN: usize = 15;
    /// Shared by both facings, not offset by `base` - PLAYER_DEAD_1/2 = 46/47.
    pub const DEAD_1: usize = 46;
}

const WALK_FRAME_SECONDS: f32 = 0.1;

pub fn animate_player(
    mut query: Query<&mut Player>,
    input: Res<PlayerInput>,
    time: Res<Time>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer != 0 {
        // PLAYER_DEAD_1/2 alternate every tick, shared by both facings
        // (game1.c:9236: `PLAYER_DEAD_1 + (playerDeadTime % 2)`).
        p.frame = frame::DEAD_1 + (p.dead_timer as usize % 2);
        return;
    }
    let base = match p.face_dir {
        FaceDir::West => 0,
        FaceDir::East => 23,
    };
    if p.hurt_cooldown > 40 {
        // The "ouch" pose: shown only for the first 4 of the 44
        // invincibility-cooldown ticks (game1.c:9214-9218) - the original
        // also flashes it solid white for exactly the very first tick,
        // which we don't replicate (no white draw-mode), just the pose.
        p.frame = base + frame::PAIN;
        return;
    }
    let moving = (input.west || input.east) && p.on_ground && p.cling_dir.is_none();
    let local = if p.cling_dir.is_some() {
        frame::CLING
    } else if p.is_falling {
        if p.fall_time > 3 {
            frame::FALL_LONG
        } else {
            frame::FALL
        }
    } else if p.jump_time > 0 {
        frame::JUMP
    } else if moving {
        p.anim_timer += time.delta_secs();
        let step = (p.anim_timer / WALK_FRAME_SECONDS) as usize % frame::WALK.len();
        frame::WALK[step]
    } else if !input.west && !input.east && input.look_up {
        frame::LOOK_NORTH
    } else if !input.west && !input.east && input.look_down {
        frame::LOOK_SOUTH
    } else {
        p.anim_timer = 0.0;
        frame::STAND
    };
    p.frame = base + local;
}

#[derive(Resource, Default)]
pub struct PlayerFrames(pub Vec<Handle<Image>>);

impl PlayerFrames {
    pub fn load(asset_server: &AssetServer, data: &GameData) -> Self {
        let Some(manifest) = data.load_sprite_manifest("sprites/player") else {
            return Self::default();
        };
        let handles = manifest
            .frames
            .iter()
            .map(|f| asset_server.load(format!("generated/sprites/player/{}", f.file)))
            .collect();
        Self(handles)
    }
}

pub fn apply_player_frame(mut query: Query<(&Player, &mut Sprite)>, frames: Res<PlayerFrames>) {
    for (p, mut sprite) in &mut query {
        if let Some(h) = frames.0.get(p.frame) {
            sprite.image = h.clone();
        }
    }
}
