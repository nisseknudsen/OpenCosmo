//! Per-actor enemy behavior, ported from cosmore's `ActXxx()` tick
//! functions in game1.c.
//!
//! The original gives every actor type its own hand-written tick function
//! plus five generic scratch words (`data1`..`data5`) whose meaning is
//! entirely per-behavior - see the `Actor` struct at glue.h:95-103. Rather
//! than invent nicer names that would stop matching the source, this port
//! keeps `d1`..`d5` verbatim so each behavior below reads line-for-line
//! against its original and stays checkable. `NewActorAtIndex()`
//! (game1.c:5618-6371) supplies each type's starting values; those are
//! reproduced in `spawn_state_for`.
//!
//! Actors move in whole tiles on the same ~18.2Hz fixed tick as the
//! player, and `y` is the *bottom* row of the sprite (the original's
//! convention, shared with `Player`).
//!
//! NOT PORTED, and stubbed where a behavior reaches for them: explosions
//! and shards, score effects, sounds, pouncing, map-tile mutation, and
//! actor-vs-actor interaction. Each site is marked `NOT PORTED` inline.

use crate::data::{
    GameData, LevelJson, TILE_ATTR_BLOCK_EAST, TILE_ATTR_BLOCK_NORTH, TILE_ATTR_BLOCK_SOUTH,
    TILE_ATTR_BLOCK_WEST, TILE_ATTR_SLOPED,
};
use crate::level::{tile_topleft_to_center, CurrentLevel};
use crate::player::Player;
use bevy::prelude::*;

/// Which ported `ActXxx()` a given actor runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    /// `ActPrize` (game1.c:4902-4928) - the floating collectibles. By far
    /// the most common actor in Episode 1.
    Prize,
    /// `ActRoamerSlug` (game1.c:2966-3046).
    RoamerSlug,
    /// `ActRedChomper` (game1.c:4271-4340).
    RedChomper,
    /// `ActPinkWorm` (game1.c:4393-4443).
    PinkWorm,
    /// `ActCabbage` (game1.c:2336-2389).
    Cabbage,
    /// `ActParachuteBall` (game1.c:3188-3272).
    ParachuteBall,
    /// `ActGhost` (game1.c:2699-2742).
    Ghost,
    /// `ActBird` (game1.c:5105-5173).
    Bird,
    /// `ActClamPlant` (game1.c:3145-3181).
    ClamPlant,
    /// `ActReciprocatingSpikes` (game1.c:2227-2253).
    ReciprocatingSpikes,
    /// `ActReciprocatingSpear` (game1.c:2394-2410).
    ReciprocatingSpear,
    /// `ActSuctionWalker` (game1.c:3919-4065).
    SuctionWalker,
    /// `ActFallingFloor` (game1.c:4977-5014).
    FallingFloor,
    /// `ActPyramid` (game1.c:2657-2693).
    Pyramid,
}

/// Ported behavior + starting `data1..data5` for each ACT_* id, taken from
/// that type's `ConstructActor(...)` call in `NewActorAtIndex()`.
///
/// Types absent from this table keep the previous static/`Walker`
/// treatment. In particular every actor whose tick function is
/// `ActFootSwitch` is deliberately excluded: that function returns
/// immediately for any sprite other than the foot-switch knob
/// (game1.c:1900), so those ~43 actor types genuinely are inert scenery
/// and already render correctly as static sprites.
const ENEMY_TABLE: &[(u16, EnemyKind, [i32; 5])] = &[
    // --- ActPrize ---
    (1, EnemyKind::Prize, [0, 0, 0, 0, 4]),       // ACT_STAR_FLOAT
    (28, EnemyKind::Prize, [0, 0, 0, 1, 6]),      // ACT_POWER_UP_FLOAT
    (94, EnemyKind::Prize, [0, 0, 0, 1, 2]),      // ACT_DANCING_MUSHROOM
    (153, EnemyKind::Prize, [0, 0, 0, 0, 4]),     // ACT_ROTATING_ORNAMENT
    (154, EnemyKind::Prize, [0, 0, 0, 0, 5]),     // ACT_BLU_CRYSTAL
    (155, EnemyKind::Prize, [0, 0, 0, 0, 6]),     // ACT_RED_CRYSTAL_FLOOR
    (174, EnemyKind::Prize, [0, 0, 0, 0, 5]),     // ACT_GRN_EMERALD
    (176, EnemyKind::Prize, [0, 0, 0, 0, 4]),     // ACT_CLR_DIAMOND
    (189, EnemyKind::Prize, [0, 0, 0, 0, 4]),     // ACT_INVINCIBILITY_CUBE
    (194, EnemyKind::Prize, [3, 2, 0, 0, 1]),     // ACT_CYA_DIAMOND
    (196, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_RED_DIAMOND
    (198, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_GRY_OCTAHEDRON
    (200, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_BLU_EMERALD
    (202, EnemyKind::Prize, [0, 0, 0, 0, 4]),     // ACT_THRUSTER_JET
    (213, EnemyKind::Prize, [3, 2, 0, 0, 1]),     // ACT_CYA_DIAMOND_FLOAT
    (214, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_RED_DIAMOND_FLOAT
    (215, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_GRY_OCTAHED_FLOAT
    (216, EnemyKind::Prize, [2, 2, 0, 0, 1]),     // ACT_BLU_EMERALD_FLOAT
    (252, EnemyKind::Prize, [1, 0, 0, 0, 6]),     // ACT_RED_CRYSTAL_CEIL
    (263, EnemyKind::Prize, [0, 0, 0, 1, 6]),     // ACT_POWER_UP
    (264, EnemyKind::Prize, [0, 0, 0, 0, 4]),     // ACT_STAR
    // --- creatures ---
    (25, EnemyKind::Cabbage, [1, 0, 0, 0, 0]),          // ACT_CABBAGE
    (51, EnemyKind::Ghost, [0, 0, 0, 0, 4]),            // ACT_GHOST
    (69, EnemyKind::RoamerSlug, [0, 3, 0, 0, 0]),       // ACT_ROAMER_SLUG
    (86, EnemyKind::ParachuteBall, [0, 20, 0, 0, 2]),   // ACT_PARACHUTE_BALL
    (106, EnemyKind::SuctionWalker, [DIR2_WEST, 0, 0, 0, 0]), // ACT_SUCTION_WALKER
    (118, EnemyKind::RedChomper, [DIR2_WEST, 0, 0, 0, 0]),    // ACT_RED_CHOMPER
    (124, EnemyKind::PinkWorm, [DIR2_WEST, 0, 0, 0, 0]),      // ACT_PINK_WORM
    (187, EnemyKind::Bird, [0, 0, 0, 0, 0]),            // ACT_BIRD
    // --- hazards / scenery with motion ---
    (18, EnemyKind::ReciprocatingSpikes, [1, 0, 0, 0, 0]), // ACT_SPIKES_FLOOR_RECIP
    (41, EnemyKind::ReciprocatingSpear, [0, 0, 0, 0, 0]),  // ACT_SPEAR_RECIP
    (50, EnemyKind::Pyramid, [0, 0, 0, 0, 1]),             // ACT_PYRAMID_FLOOR
    (83, EnemyKind::ClamPlant, [0, 0, 0, 0, 0]),           // ACT_CLAM_PLANT_FLOOR
    (84, EnemyKind::ClamPlant, [0, 0, 0, 0, 4]),           // ACT_CLAM_PLANT_CEIL
    (163, EnemyKind::FallingFloor, [0, 0, 0, 0, 0]),       // ACT_FALLING_FLOOR
];

const DIR2_WEST: i32 = 0;
const DIR2_EAST: i32 = 1;

/// `DRAW_MODE_FLIPPED` (def.h:77) - flipped vertically, not horizontally.
const DRAW_MODE_FLIPPED: i32 = 4;

/// Looks up the ported behavior for an ACT_* id, if there is one.
pub fn behavior_for(act_id: u16) -> Option<(EnemyKind, [i32; 5])> {
    ENEMY_TABLE
        .iter()
        .find(|(id, ..)| *id == act_id)
        .map(|(_, kind, data)| (*kind, *data))
}

/// A live actor running one of the ported behaviors.
#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub x: i32,
    pub y: i32,
    pub frame: usize,
    /// The original's generic per-actor scratch words.
    pub d1: i32,
    pub d2: i32,
    pub d3: i32,
    pub d4: i32,
    pub d5: i32,
    /// `Actor.falltime` - only the parachute ball reads it here.
    pub fall_time: i32,
    /// `Actor.dead`; a dead actor stops ticking and hides.
    pub dead: bool,
    pub width_tiles: i32,
    pub height_tiles: i32,
    /// Set by behaviors that walk into things, mirroring
    /// `Actor.westfree` / `Actor.eastfree` (game1.c:1813-1880).
    pub west_free: bool,
    pub east_free: bool,
    /// Actors flagged `acrophile` in `ConstructActor` happily walk off
    /// ledges; the rest turn around at one.
    pub acrophile: bool,
    pub frames: Vec<Handle<Image>>,
    /// Per-actor PRNG state - see `next_rand`.
    rng: u32,
}

impl Enemy {
    pub fn new(
        kind: EnemyKind,
        data: [i32; 5],
        x: i32,
        y: i32,
        width_tiles: i32,
        height_tiles: i32,
        frames: Vec<Handle<Image>>,
    ) -> Self {
        // ConstructActor marks the cabbage and parachute ball as acrophile
        // (game1.c:5691, 5843); everything else ported here is not.
        let acrophile = matches!(kind, EnemyKind::Cabbage | EnemyKind::ParachuteBall);
        Enemy {
            kind,
            x,
            y,
            frame: 0,
            d1: data[0],
            d2: data[1],
            d3: data[2],
            d4: data[3],
            d5: data[4],
            fall_time: 0,
            dead: false,
            width_tiles,
            height_tiles,
            west_free: true,
            east_free: true,
            acrophile,
            frames,
            // Seed from the spawn position so each actor desynchronises
            // from its neighbours but stays deterministic across runs.
            rng: (x as u32).wrapping_mul(1973).wrapping_add((y as u32).wrapping_mul(9277)) | 1,
        }
    }

    /// Stand-in for the original's `GameRand()`. The exact sequence is not
    /// reproduced - only the distribution matters for these behaviors, all
    /// of which use it for idle timing jitter.
    fn next_rand(&mut self, modulo: u32) -> u32 {
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        if modulo == 0 {
            0
        } else {
            self.rng % modulo
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum MoveResult {
    Free,
    Blocked,
    Sloped,
}

#[derive(Clone, Copy)]
enum Dir4 {
    North,
    South,
    West,
    East,
}

fn attr_at(level: &LevelJson, data: &GameData, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 {
        return 0;
    }
    data.tile_attr(level.tile_at(x as usize, y as usize))
}

/// Port of `TestSpriteMove()` (game1.c:940-1020): tests a sprite's whole
/// footprint against the tile attribute flags, generalised over the
/// sprite's width/height in tiles.
fn test_sprite_move(
    dir: Dir4,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    level: &LevelJson,
    data: &GameData,
) -> MoveResult {
    match dir {
        Dir4::North => {
            let row = y - height + 1;
            for i in 0..width {
                if attr_at(level, data, x + i, row) & TILE_ATTR_BLOCK_NORTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Dir4::South => {
            for i in 0..width {
                let a = attr_at(level, data, x + i, y);
                if a & TILE_ATTR_SLOPED != 0 {
                    return MoveResult::Sloped;
                }
                if a & TILE_ATTR_BLOCK_SOUTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Dir4::West => {
            for i in 0..height {
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
        Dir4::East => {
            let col = x + width - 1;
            for i in 0..height {
                let row = y - i;
                let a = attr_at(level, data, col, row);
                if a & TILE_ATTR_BLOCK_EAST != 0 {
                    return MoveResult::Blocked;
                }
                if i == 0
                    && a & TILE_ATTR_SLOPED != 0
                    && attr_at(level, data, col, row - 1) & TILE_ATTR_BLOCK_EAST == 0
                {
                    return MoveResult::Sloped;
                }
            }
            MoveResult::Free
        }
    }
}

/// Condensed port of `AdjustActorMove()` (game1.c:1813-1880). The caller
/// has already stepped `x`; this validates that step and reverts it if the
/// actor walked into a wall, or off a ledge it isn't allowed to leave.
/// Sets `west_free`/`east_free` the way the original does, since the
/// walking behaviors turn around on those.
///
/// The original's several slope-handling branches are collapsed into the
/// `Sloped` case here: it steps the actor up a row and treats the move as
/// free, which is the outcome those branches converge on.
fn adjust_actor_move(e: &mut Enemy, dir: Dir4, level: &LevelJson, data: &GameData) {
    let result = test_sprite_move(dir, e.x, e.y, e.width_tiles, e.height_tiles, level, data);
    let free = match result {
        MoveResult::Free => true,
        MoveResult::Sloped => {
            e.y -= 1;
            true
        }
        MoveResult::Blocked => {
            // Undo the caller's step.
            match dir {
                Dir4::West => e.x += 1,
                Dir4::East => e.x -= 1,
                _ => {}
            }
            false
        }
    };

    let free = if free && !e.acrophile {
        // Ground check: an actor that isn't an acrophile refuses to step
        // off a ledge, and reports the direction as blocked so it turns.
        let ground = test_sprite_move(
            Dir4::South,
            e.x,
            e.y + 1,
            e.width_tiles,
            e.height_tiles,
            level,
            data,
        );
        if ground == MoveResult::Free {
            match dir {
                Dir4::West => e.x += 1,
                Dir4::East => e.x -= 1,
                _ => {}
            }
            false
        } else {
            true
        }
    } else {
        free
    };

    match dir {
        Dir4::West => e.west_free = free,
        Dir4::East => e.east_free = free,
        _ => {}
    }
}

/// Runs one behavior tick for every live enemy, then syncs its transform
/// and sprite frame. Logic and presentation share a system so that
/// `hazard_damage`, which reads `Transform` in the same schedule, always
/// sees this tick's position.
pub fn tick_enemies(
    mut query: Query<(&mut Enemy, &mut Transform, &mut Sprite, &mut Visibility)>,
    player_q: Query<&Player>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    let Some(level) = data.load_level(&level_data.name) else {
        return;
    };

    for (mut e, mut transform, mut sprite, mut visibility) in &mut query {
        if e.dead {
            *visibility = Visibility::Hidden;
            continue;
        }

        match e.kind {
            EnemyKind::Prize => tick_prize(&mut e),
            EnemyKind::RoamerSlug => tick_roamer_slug(&mut e, &level, &data),
            EnemyKind::RedChomper => tick_red_chomper(&mut e, player, &level, &data),
            EnemyKind::PinkWorm => tick_pink_worm(&mut e, &level, &data),
            EnemyKind::Cabbage => tick_cabbage(&mut e, player, &level, &data),
            EnemyKind::ParachuteBall => tick_parachute_ball(&mut e, player, &level, &data),
            EnemyKind::Ghost => tick_ghost(&mut e, player),
            EnemyKind::Bird => tick_bird(&mut e, player),
            EnemyKind::ClamPlant => tick_clam_plant(&mut e),
            EnemyKind::ReciprocatingSpikes => tick_reciprocating_spikes(&mut e),
            EnemyKind::ReciprocatingSpear => tick_reciprocating_spear(&mut e),
            EnemyKind::SuctionWalker => tick_suction_walker(&mut e, &level, &data),
            EnemyKind::FallingFloor => tick_falling_floor(&mut e, player, &level, &data),
            EnemyKind::Pyramid => tick_pyramid(&mut e, player, &level, &data),
        }

        // Presentation. `frame` is clamped rather than trusted: a few
        // behaviors index frame tables whose sprites have fewer frames
        // decoded than the original's actor info claims.
        if let Some(handle) = e.frames.get(e.frame.min(e.frames.len().saturating_sub(1))) {
            sprite.image = handle.clone();
        }
        let top_row = e.y - (e.height_tiles - 1);
        let world = tile_topleft_to_center(
            e.x as f32,
            top_row as f32,
            e.width_tiles as f32 * 8.0,
            e.height_tiles as f32 * 8.0,
        );
        transform.translation.x = world.x;
        transform.translation.y = world.y;

        // `nextDrawMode = DRAW_MODE_FLIPPED` is a *vertical* flip.
        transform.scale.y = if flips_vertically(&e) { -1.0 } else { 1.0 };
        // Walking actors face their direction of travel. Sprite art faces
        // west by default, matching the player's PLAYER_BASE_WEST = 0.
        transform.scale.x = match facing(&e) {
            Some(DIR2_EAST) => -1.0,
            _ => 1.0,
        };
        *visibility = Visibility::Inherited;
    }
}

/// Whether this actor is drawn vertically flipped this tick.
fn flips_vertically(e: &Enemy) -> bool {
    match e.kind {
        // ActPrize (game1.c:4906-4908) flips whenever data1 is non-zero.
        EnemyKind::Prize => e.d1 != 0,
        // ActClamPlant (game1.c:3148) takes its draw mode straight from
        // data5, which the ceiling-mounted variant sets to FLIPPED.
        EnemyKind::ClamPlant => e.d5 == DRAW_MODE_FLIPPED,
        // ActPyramid (game1.c:2661-2662) flips the floor-mounted variant.
        EnemyKind::Pyramid => e.d5 != 0,
        _ => false,
    }
}

/// The DIR2_* this actor currently faces, for behaviors that walk.
fn facing(e: &Enemy) -> Option<i32> {
    match e.kind {
        EnemyKind::RedChomper | EnemyKind::PinkWorm | EnemyKind::SuctionWalker => Some(e.d1),
        // ActCabbage stores its facing in data4 (game1.c:2358-2363).
        EnemyKind::Cabbage => Some(if e.d4 != 0 { DIR2_EAST } else { DIR2_WEST }),
        _ => None,
    }
}

/// `ActPrize` (game1.c:4902-4928). Cycles `frame` up to `data5` and back
/// to zero; `data4` halves the rate via the `data3` toggle.
///
/// NOT PORTED: the sparkle decoration the original emits for single-frame
/// prizes (needs `NewDecoration`).
fn tick_prize(e: &mut Enemy) {
    if e.d4 == 0 {
        e.frame += 1;
    } else {
        e.d3 = if e.d3 == 0 { 1 } else { 0 };
        if e.d3 != 0 {
            e.frame += 1;
        }
    }
    if e.frame as i32 == e.d5 {
        e.frame = 0;
    }
}

/// `ActRoamerSlug` (game1.c:2966-3046). Crawls in one of four directions
/// until blocked, then picks a new direction at random. `data3` selects
/// the frame pair for the current heading and `data4` alternates within it.
fn tick_roamer_slug(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    const DIR4_NORTH: i32 = 0;
    const DIR4_SOUTH: i32 = 1;
    const DIR4_WEST: i32 = 2;
    const DIR4_EAST: i32 = 3;

    let (w, h) = (e.width_tiles, e.height_tiles);
    let free = |dir, x, y| test_sprite_move(dir, x, y, w, h, level, data) == MoveResult::Free;

    if e.d5 == 0 {
        match e.d1 {
            DIR4_NORTH => {
                if free(Dir4::North, e.x, e.y - 1) {
                    e.y -= 1;
                } else {
                    e.d5 = 1;
                }
                e.d3 = 0;
            }
            DIR4_SOUTH => {
                if free(Dir4::South, e.x, e.y + 1) {
                    e.y += 1;
                } else {
                    e.d5 = 1;
                }
                e.d3 = 4;
            }
            DIR4_WEST => {
                if free(Dir4::West, e.x - 1, e.y) {
                    e.x -= 1;
                } else {
                    e.d5 = 1;
                }
                e.d3 = 6;
            }
            _ => {
                if free(Dir4::East, e.x + 1, e.y) {
                    e.x += 1;
                } else {
                    e.d5 = 1;
                }
                e.d3 = 2;
            }
        }
    } else {
        let newdir = e.next_rand(4) as i32;
        let ok = match newdir {
            DIR4_NORTH => free(Dir4::North, e.x, e.y - 1),
            DIR4_SOUTH => free(Dir4::South, e.x, e.y + 1),
            DIR4_WEST => free(Dir4::West, e.x - 1, e.y),
            _ => free(Dir4::East, e.x + 1, e.y),
        };
        if ok {
            e.d5 = 0;
            e.d1 = newdir;
        }
    }

    e.d4 = if e.d4 == 0 { 1 } else { 0 };
    e.frame = (e.d3 + e.d4).max(0) as usize;
}

/// `ActRedChomper` (game1.c:4271-4340). Walks on alternate ticks, with two
/// random interruptions: a chomp (`data5` 1..10) and a look-around
/// (`data5` 11..16) that can end with it turning toward the player.
fn tick_red_chomper(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    e.d4 = if e.d4 == 0 { 1 } else { 0 };

    if e.next_rand(95) == 0 {
        e.d5 = 10;
    } else if e.next_rand(100) == 0 {
        e.d5 = 11;
    }

    if e.d5 < 11 && e.d5 != 0 {
        e.d5 -= 1;
        if e.d5 > 8 {
            e.frame = 6;
        } else if e.d5 == 8 {
            e.frame = 5;
        } else {
            e.d2 = if e.d2 == 0 { 1 } else { 0 };
            e.frame = (e.d2 + 6) as usize;
        }
        if e.d5 == 0 && e.next_rand(2) != 0 {
            e.d1 = if e.x >= player.x { DIR2_WEST } else { DIR2_EAST };
        }
    } else if e.d5 > 10 {
        const SEARCH_W: [usize; 6] = [8, 9, 10, 10, 9, 8];
        const SEARCH_E: [usize; 6] = [10, 9, 8, 8, 9, 10];
        let i = (e.d5 - 11).clamp(0, 5) as usize;
        e.frame = if e.d1 == DIR2_WEST {
            SEARCH_W[i]
        } else {
            SEARCH_E[i]
        };
        e.d5 += 1;
        if e.d5 == 17 {
            e.d5 = 0;
        }
    } else if e.d4 != 0 {
        if e.d1 == DIR2_WEST {
            e.frame = if e.frame == 0 { 1 } else { 0 };
            e.x -= 1;
            adjust_actor_move(e, Dir4::West, level, data);
            if !e.west_free {
                e.d1 = DIR2_EAST;
                e.frame = 4;
            }
        } else {
            e.d3 = if e.d3 == 0 { 1 } else { 0 };
            e.frame = (e.d3 + 2) as usize;
            e.x += 1;
            adjust_actor_move(e, Dir4::East, level, data);
            if !e.east_free {
                e.d1 = DIR2_WEST;
                e.frame = 4;
            }
        }
    }
}

/// `ActPinkWorm` (game1.c:4393-4443). Moves every other tick, pausing
/// occasionally to rear up (`data3`).
fn tick_pink_worm(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    e.d4 = if e.d4 == 0 { 1 } else { 0 };
    if e.d4 != 0 {
        return;
    }

    if e.next_rand(40) > 37 && e.d3 == 0 && e.d2 == 0 {
        e.d3 = 4;
    }

    if e.d3 != 0 {
        e.d3 -= 1;
        if e.d3 == 2 {
            if e.d1 == DIR2_WEST {
                e.frame = 2;
            } else if e.d2 == 0 {
                e.frame = 5;
            }
        } else if e.d1 == DIR2_WEST {
            e.frame = 0;
        } else {
            e.frame = 3;
        }
    } else if e.d1 == DIR2_WEST {
        e.frame = if e.frame == 0 { 1 } else { 0 };
        if e.frame != 0 {
            e.x -= 1;
            adjust_actor_move(e, Dir4::West, level, data);
            if !e.west_free {
                e.d1 = DIR2_EAST;
            }
        }
    } else {
        e.d2 = if e.d2 == 0 { 1 } else { 0 };
        if e.d2 == 0 {
            e.x += 1;
            adjust_actor_move(e, Dir4::East, level, data);
            if !e.east_free {
                e.d1 = DIR2_WEST;
            }
        }
        e.frame = (e.d2 + 3) as usize;
    }
}

/// `ActCabbage` (game1.c:2336-2389). Sits for ten ticks facing the player,
/// then hops three tiles toward them.
fn tick_cabbage(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);
    let grounded =
        test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data) != MoveResult::Free;

    if e.d2 == 10 && e.d3 == 3 && !grounded {
        e.frame = if e.d4 != 0 { 3 } else { 1 };
    } else if e.d2 < 10 && grounded {
        e.d2 += 1;
        e.d4 = if e.x > player.x { 0 } else { 2 };
        e.frame = e.d4 as usize;
    } else if e.d3 < 3 {
        const Y_JUMP: [i32; 3] = [-1, -1, 0];
        e.y += Y_JUMP[e.d3.clamp(0, 2) as usize];
        if e.d4 != 0 {
            e.x += 1;
            adjust_actor_move(e, Dir4::East, level, data);
        } else {
            e.x -= 1;
            adjust_actor_move(e, Dir4::West, level, data);
        }
        e.d3 += 1;
        e.frame = if e.d4 != 0 { 3 } else { 1 };
    } else {
        e.d2 = 0;
        e.d3 = 0;
        e.d4 = if e.x > player.x { 0 } else { 2 };
        e.frame = e.d4 as usize;
    }
}

/// `ActParachuteBall` (game1.c:3188-3272). Bobs through an idle frame
/// table, then charges horizontally at the player.
///
/// NOT PORTED: the falling/parachute-deploy branch, which depends on
/// `Actor.falltime` being driven by the shared actor gravity pass we
/// haven't ported. `fall_time` therefore stays 0 and the actor always
/// takes the grounded path.
fn tick_parachute_ball(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    const IDLE_FRAMES: [usize; 27] = [
        2, 2, 2, 0, 3, 3, 3, 0, 0, 2, 2, 0, 0, 1, 1, 0, 1, 3, 3, 3, 0, 1, 1, 0, 1, 1, 1,
    ];

    if e.d1 == 0 {
        e.d2 += 1;
        e.frame = IDLE_FRAMES[e.d2.clamp(0, 26) as usize];

        if e.d2 == 26 {
            e.d2 = 0;
            if e.y == player.y || e.next_rand(2) == 0 {
                if e.x >= player.x + 2 {
                    e.d1 = 1; // west
                    e.frame = 2;
                    e.d3 = 6;
                } else if e.x + 2 <= player.x {
                    e.d1 = 2; // east
                    e.frame = 3;
                    e.d3 = 6;
                }
            }
        }
    }

    if e.d3 != 0 {
        e.d3 -= 1;
    } else if e.d1 == 1 {
        e.x -= 1;
        adjust_actor_move(e, Dir4::West, level, data);
        if !e.west_free {
            e.d1 = 0;
            e.d2 = 0;
            e.frame = 0;
        } else {
            const FRAMES: [usize; 4] = [7, 6, 5, 4];
            e.frame = FRAMES[(e.d2.rem_euclid(4)) as usize];
            e.d2 += 1;
            if e.d2 == 16 {
                e.d1 = 0;
                e.d2 = 0;
            }
        }
    } else if e.d1 == 2 {
        e.x += 1;
        adjust_actor_move(e, Dir4::East, level, data);
        if !e.east_free {
            e.d1 = 0;
            e.d2 = 0;
            e.frame = 0;
        } else {
            const FRAMES: [usize; 4] = [4, 5, 6, 7];
            e.frame = FRAMES[(e.d2.rem_euclid(4)) as usize];
            e.d2 += 1;
            if e.d2 == 12 {
                e.d1 = 0;
                e.d2 = 0;
            }
        }
    }
}

/// `ActGhost` (game1.c:2699-2742). Drifts toward the player every fourth
/// tick, closing vertically as it goes.
///
/// The original picks its frame from the player's facing *and* whether the
/// player is clinging to a wall while pressing away from it; without the
/// cling-direction inputs ported here it uses the player's facing alone,
/// which drives the same frame set in every case but the cling one.
fn tick_ghost(e: &mut Enemy, player: &Player) {
    e.d4 += 1;
    if e.d4 % 3 == 0 {
        e.d1 += 1;
    }
    if e.d1 == 4 {
        e.d1 = 0;
    }

    let player_faces_west = matches!(player.face_dir, crate::player::FaceDir::West);
    if player_faces_west {
        if e.x > player.x {
            e.frame = (e.d1 % 2) as usize;
            if e.d1 == 0 {
                e.x -= 1;
                match e.y.cmp(&player.y) {
                    std::cmp::Ordering::Less => e.y += 1,
                    std::cmp::Ordering::Greater => e.y -= 1,
                    std::cmp::Ordering::Equal => {}
                }
            }
        } else {
            e.frame = 5;
        }
    } else if e.x < player.x {
        e.frame = ((e.d1 % 2) + 3) as usize;
        if e.d1 == 0 {
            e.x += 1;
            match e.y.cmp(&player.y) {
                std::cmp::Ordering::Less => e.y += 1,
                std::cmp::Ordering::Greater => e.y -= 1,
                std::cmp::Ordering::Equal => {}
            }
        }
    } else {
        e.frame = 2;
    }
}

/// `ActBird` (game1.c:5105-5173). Three phases: perch and watch, flap in
/// place, then swoop along a fixed vertical arc toward the player.
fn tick_bird(e: &mut Enemy, player: &Player) {
    if e.d1 == 0 {
        e.d2 = if e.x + 1 > player.x {
            if e.next_rand(10) == 0 {
                1
            } else {
                0
            }
        } else if e.next_rand(10) == 0 {
            5
        } else {
            4
        };
        e.frame = e.d2 as usize;

        e.d3 += 1;
        if e.d3 == 30 {
            e.d1 = 1;
            e.d3 = 0;
        }
    } else if e.d1 == 1 {
        e.d3 += 1;
        if e.d3 == 20 {
            e.d3 = 0;
            e.d1 = 2;
            e.d4 = if e.x + 1 > player.x {
                DIR2_WEST
            } else {
                DIR2_EAST
            };
        } else if e.d3 % 2 != 0 && e.d3 < 10 {
            e.y -= 1;
        }

        e.frame = if e.x + 1 > player.x {
            ((e.d3 % 2) + 2) as usize
        } else {
            ((e.d3 % 2) + 6) as usize
        };
    } else if e.d1 == 2 {
        const Y_JUMP: [i32; 15] = [2, 2, 2, 1, 1, 1, 0, 0, 0, -1, -1, -1, -2, -2, -2];

        e.d3 += 1;
        if e.d4 == DIR2_WEST {
            e.frame = ((e.d3 % 2) + 2) as usize;
            e.x -= 1;
        } else {
            e.frame = ((e.d3 % 2) + 6) as usize;
            e.x += 1;
        }
        e.y += Y_JUMP[(e.d3 - 1).clamp(0, 14) as usize];

        if e.d3 == 15 {
            e.d1 = 1;
            e.d3 = 10;
        }
    }
}

/// `ActClamPlant` (game1.c:3145-3181). Idles closed for sixteen ticks,
/// then opens and closes through frames 0..4.
fn tick_clam_plant(e: &mut Enemy) {
    if e.d2 == 1 {
        e.frame += 1;
        if e.frame == 4 {
            e.d2 = 2;
        }
    } else if e.d2 == 2 {
        e.frame = e.frame.saturating_sub(1);
        if e.frame == 1 {
            e.d2 = 0;
            e.d1 = 1;
        }
    } else {
        if e.d1 < 16 {
            e.d1 += 1;
        } else {
            e.d1 = 0;
        }
        if e.d1 == 0 {
            e.d2 = 1;
        } else {
            e.frame = 0;
        }
    }
}

/// `ActReciprocatingSpikes` (game1.c:2227-2253). Extends and retracts on a
/// twenty-tick cycle. Frame 2 is fully retracted, drawn hidden.
///
/// NOT PORTED: `SND_SPIKES_MOVE`.
fn tick_reciprocating_spikes(e: &mut Enemy) {
    e.d2 += 1;
    if e.d2 == 20 {
        e.d2 = 0;
    }

    if e.frame == 0 && e.d2 == 0 {
        e.d1 = 0;
    } else if e.frame == 2 && e.d2 == 0 {
        e.d1 = 1;
    } else if e.d1 != 0 {
        e.frame = e.frame.saturating_sub(1);
    } else if e.frame < 2 {
        e.frame += 1;
    }
}

/// `ActReciprocatingSpear` (game1.c:2394-2410). Rises and falls on a
/// thirty-tick cycle without changing frame.
fn tick_reciprocating_spear(e: &mut Enemy) {
    if e.d1 < 30 {
        e.d1 += 1;
    } else {
        e.d1 = 0;
    }

    if e.d1 > 22 {
        e.y -= 1;
    } else if e.d1 > 14 {
        e.y += 1;
    }
}

/// `ActSuctionWalker` (game1.c:3919-4065). Walks along floors and
/// ceilings, flipping between them when it runs out of surface.
///
/// Only the west-facing floor and ceiling states plus the two transitions
/// are ported; the original's east-facing half is the mirror image and is
/// reached by flipping `data1`. `CanSuctionWalkerFlip` is approximated by
/// testing whether the destination surface is reachable.
fn tick_suction_walker(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);
    let dir = if e.d1 == DIR2_WEST { -1 } else { 1 };
    let step_dir = if e.d1 == DIR2_WEST {
        Dir4::West
    } else {
        Dir4::East
    };

    e.d4 = if e.d4 == 0 { 1 } else { 0 };

    match e.d2 {
        // On the floor.
        0 => {
            if e.d4 != 0 {
                e.d3 = if e.d3 == 0 { 1 } else { 0 };
                e.frame = e.d3 as usize;
            }
            let blocked =
                test_sprite_move(step_dir, e.x + dir, e.y, w, h, level, data) != MoveResult::Free;
            let ledge =
                attr_at(level, data, e.x + dir, e.y + 1) & TILE_ATTR_BLOCK_SOUTH == 0;
            if blocked || ledge || e.next_rand(50) == 0 {
                // Flip to the ceiling if there is one to reach, else turn.
                if test_sprite_move(Dir4::North, e.x, e.y - 1, w, h, level, data)
                    == MoveResult::Free
                {
                    e.d2 = 2;
                    e.frame = 9;
                } else {
                    e.d1 = if e.d1 == DIR2_WEST { DIR2_EAST } else { DIR2_WEST };
                    e.d2 = 0;
                }
            } else if e.d4 != 0 {
                e.x += dir;
            }
        }
        // On the ceiling.
        1 => {
            if e.d4 != 0 {
                e.d3 = if e.d3 == 0 { 1 } else { 0 };
                e.frame = (e.d3 + 4) as usize;
            }
            let blocked =
                test_sprite_move(step_dir, e.x + dir, e.y, w, h, level, data) != MoveResult::Free;
            let ledge = attr_at(level, data, e.x + dir, e.y - h) & TILE_ATTR_BLOCK_WEST == 0;
            if blocked || ledge || e.next_rand(50) == 0 {
                if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data)
                    == MoveResult::Free
                {
                    e.d2 = 3;
                    e.frame = 9;
                } else {
                    e.d1 = if e.d1 == DIR2_WEST { DIR2_EAST } else { DIR2_WEST };
                    e.d2 = 1;
                }
            } else if e.d4 != 0 {
                e.x += dir;
            }
        }
        // Rising to the ceiling - two rows per tick.
        2 => {
            for _ in 0..2 {
                if test_sprite_move(Dir4::North, e.x, e.y - 1, w, h, level, data)
                    != MoveResult::Free
                {
                    e.d2 = 1;
                    break;
                }
                e.y -= 1;
            }
        }
        // Falling to the floor - two rows per tick.
        _ => {
            for _ in 0..2 {
                if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data)
                    != MoveResult::Free
                {
                    e.d2 = 0;
                    break;
                }
                e.y += 1;
            }
        }
    }
}

/// `ActFallingFloor` (game1.c:4977-5014). Waits until the player stands on
/// it, then drops after a seven-tick delay and shatters on impact.
///
/// NOT PORTED: the map-tile swap that makes it solid while intact (needs
/// `SetMapTile`), and the shards/sound on impact. Without the tile swap
/// the player cannot actually stand on one, so the trigger below is the
/// only thing that starts it falling.
fn tick_falling_floor(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);

    if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data) != MoveResult::Free {
        if e.d1 == 2 {
            e.dead = true; // landed after falling
        }
        return;
    }

    if e.y - 2 == player.y && e.x <= player.x + 2 && e.x + 1 >= player.x {
        e.d2 = 7;
    }

    if e.d2 != 0 {
        e.d2 -= 1;
        if e.d2 == 0 {
            e.d1 = 2; // now weighted - falls from here on
        }
    }

    if e.d1 == 2 {
        e.y += 1;
    }
}

/// `ActPyramid` (game1.c:2657-2693). The floor-mounted variant is inert
/// scenery; the ceiling-mounted one drops once the player walks beneath it.
///
/// NOT PORTED: explosion propagation and the score/shard award.
fn tick_pyramid(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    if e.d5 != 0 {
        return; // floor mounted - drawn flipped, no motion
    }

    let (w, h) = (e.width_tiles, e.height_tiles);
    if e.d1 == 0 {
        if e.y < player.y && e.x <= player.x + 6 && e.x + 5 > player.x {
            e.d1 = 1;
        }
    } else if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data) != MoveResult::Free {
        e.dead = true;
    } else {
        e.y += 1;
    }
}
