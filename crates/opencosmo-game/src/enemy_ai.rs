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
    /// `ActJumpPad` (game1.c:2026-2046).
    JumpPad,
    /// `ActMoon` (game1.c:2748-2763).
    Moon,
    /// `ActSmallFlame` (game1.c:4891-4897).
    SmallFlame,
    /// `ActFlamePulse` (game1.c:5545-5567).
    FlamePulse,
    /// `ActBabyGhost` (game1.c:2870-2915).
    BabyGhost,
    /// `ActSpittingTurret` (game1.c:4164-4236).
    SpittingTurret,
    /// `ActRedJumper` (game1.c:3493-3600). Episodes 2 and 3 only -
    /// `HAS_ACT_RED_JUMPER` is defined in episode{2,3}.h and commented out
    /// in episode1.h, so the whole body compiles away for episode 1.
    RedJumper,
    /// `ActSmokeEmitter` (game1.c:5598-5611).
    SmokeEmitter,
    /// `ActDragonfly` (game1.c:4655-4675).
    Dragonfly,
}

/// How an actor responds to being landed on: the recoil it kicks the
/// player back with, and how many pounces it survives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PounceSpec {
    pub recoil: i32,
    pub hits: i32,
}

impl EnemyKind {
    /// The per-type pounce response, from the `TryPounce(...)` call in each
    /// arm of the big sprite `switch` (game1.c:7094+).
    ///
    /// `None` means landing on it does nothing - either it's a collectible
    /// you pass through (prizes) or it hurts on contact rather than
    /// yielding to a stomp. The roamer slug is in that second group: it
    /// never calls `TryPounce`, and can only be destroyed by an explosion.
    ///
    /// Every creature ported so far happens to use recoil 7; the original's
    /// larger values belong to types not yet ported (40 for a jump pad, 20
    /// for a jump-pad robot, 15 for a red jumper or sentry robot).
    pub fn pounce_spec(self) -> Option<PounceSpec> {
        let (recoil, hits) = match self {
            // Dies to a single pounce (game1.c:7136, 7148, 7285, 7301).
            EnemyKind::Cabbage
            | EnemyKind::SuctionWalker
            | EnemyKind::Bird
            | EnemyKind::RedChomper
            | EnemyKind::PinkWorm => (7, 1),
            // Both decrement `data5` and only die at zero; their starting
            // values come from `ConstructActor` (4 and 2 respectively).
            EnemyKind::Ghost => (7, 4),
            EnemyKind::ParachuteBall => (7, 2),
            // A jump pad is furniture: it launches the player much harder
            // than any creature and is never destroyed (game1.c:7097-7106),
            // so it gets an effectively unlimited hit count.
            EnemyKind::JumpPad => (40, i32::MAX),
            // Grouped with the ghost in the pounce switch, and likewise
            // decrements data5 to zero; ConstructActor starts it at 4.
            EnemyKind::Moon => (7, 4),
            // Grouped with the suction walker and bird - dies outright.
            EnemyKind::BabyGhost => (7, 1),
            // The stiffest creature in the game: a harder kick back and
            // seven pounces to kill (game1.c:7226-7241, data5 starts at 7).
            EnemyKind::RedJumper => (15, 7),
            _ => return None,
        };
        Some(PounceSpec { recoil, hits })
    }
}

/// Recoil for pouncing a basket or barrel (game1.c:7154) - softer than a
/// creature, since you're bursting a container rather than stomping.
pub const CONTAINER_POUNCE_RECOIL: i32 = 5;

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
    (2, EnemyKind::JumpPad, [0, 0, 0, 0, 0]),              // ACT_JUMP_PAD_FLOOR
    // The ceiling variant also seeds d3/d4 from its own y - see Enemy::new.
    (217, EnemyKind::JumpPad, [0, 0, 0, 0, 1]),            // ACT_JUMP_PAD_CEIL
    (54, EnemyKind::Moon, [0, 0, 0, 0, 4]),                // ACT_MOON
    (151, EnemyKind::SmallFlame, [0, 0, 0, 0, 0]),         // ACT_SMALL_FLAME
    // The west variant also spawns one column left - see Enemy::new.
    (233, EnemyKind::FlamePulse, [0, 0, 0, 0, 1]),         // ACT_FLAME_PULSE_W
    (234, EnemyKind::FlamePulse, [0, 0, 0, 0, 0]),         // ACT_FLAME_PULSE_E
    (65, EnemyKind::BabyGhost, [DIR2_SOUTH, 0, 0, 0, 0]),  // ACT_BABY_GHOST
    // d3 remembers the spawn column, which the turret snaps back to as it
    // aims; seeded from x in Enemy::new.
    (113, EnemyKind::SpittingTurret, [0, 10, 0, 0, 3]),    // ACT_SPITTING_TURRET
    (101, EnemyKind::RedJumper, [0, 0, 0, 0, 7]),          // ACT_RED_JUMPER
    // Same kind as ACT_PYRAMID_FLOOR, told apart by d5: the floor variant
    // sets it and is inert scenery, this one leaves it clear and falls.
    // It also spawns one row lower - see Enemy::new.
    (49, EnemyKind::Pyramid, [0, 0, 0, 0, 0]),             // ACT_PYRAMID_FALLING
    // --- ActDragonfly ---
    (129, EnemyKind::Dragonfly, [DIR2_WEST, 0, 0, 0, 0]),  // ACT_DRAGONFLY

    (248, EnemyKind::SmokeEmitter, [0, 0, 0, 0, 1]),       // ACT_SMOKE_EMIT_SMALL
    (249, EnemyKind::SmokeEmitter, [1, 0, 0, 0, 0]),       // ACT_SMOKE_EMIT_LARGE
];

const DIR2_WEST: i32 = 0;
const DIR2_EAST: i32 = 1;
/// The same two values under the north/south naming the baby ghost uses
/// (def.h:31-34) - DIR2_* is one enum reused for both axes.
const DIR2_SOUTH: i32 = 0;
const DIR2_NORTH: i32 = 1;

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
    /// `Actor.forceactive` - ticks even while off screen. Set permanently
    /// once a `stay_active` actor has been seen (game1.c:7858-7864).
    pub force_active: bool,
    /// `Actor.stayactive` - dormant until first seen, then always active.
    pub stay_active: bool,
    /// Pounces still needed to kill this actor, and the recoil each one
    /// gives the player. Held per-instance rather than looked up per-kind
    /// because a ceiling-mounted jump pad is the same kind as a floor one
    /// but cannot be pounced at all (game1.c:7096).
    pub pounce_hits: i32,
    pub pounce_recoil: i32,
    /// `Actor.weighted` - ConstructActor's third bool. Weighted actors get
    /// the shared gravity pass before their own tick (game1.c:7868).
    pub weighted: bool,
    pub frames: Vec<Handle<Image>>,
    /// Per-actor PRNG state - see `next_rand`.
    rng: u32,
}

impl Enemy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: EnemyKind,
        data: [i32; 5],
        x: i32,
        y: i32,
        width_tiles: i32,
        height_tiles: i32,
        frames: Vec<Handle<Image>>,
        flags: opencosmo_assets::actor_flags::ActorFlags,
    ) -> Self {
        // All four now come from the actor's own ConstructActor call rather
        // than from a guess per kind - see `opencosmo_assets::actor_flags`. The
        // pair that matters most is stay_active + weighted, which is how a
        // prize perched out of view falls once you look up at it.
        let acrophile = flags.acrophile;
        let weighted = flags.weighted;
        let spec = kind.pounce_spec();
        let mut pounce_hits = spec.map(|s| s.hits).unwrap_or(0);
        let pounce_recoil = spec.map(|s| s.recoil).unwrap_or(0);
        // ActJumpPad's ceiling variant (data5 != 0) hangs from above,
        // alternating between two rows as it compresses, and the pounce
        // switch bails out on it immediately.
        let mut data = data;
        if kind == EnemyKind::JumpPad && data[4] != 0 {
            data[2] = y + 1;
            data[3] = y + 3;
            pounce_hits = 0;
        }
        // Spawn offsets are applied by the caller from
        // `actor_flags::ACT_SPAWN_OFFSET`, which covers all 29 of them -
        // this used to hardcode the two that had been noticed by eye.
        if kind == EnemyKind::SpittingTurret {
            data[2] = x; // d3 = spawn column, the turret's rest position
        }
        Enemy {
            kind,
            pounce_hits,
            pounce_recoil,
            weighted,
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
            force_active: flags.force_active,
            stay_active: flags.stay_active,
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
/// `ActJumpPad` (game1.c:2026-2046). Frame 1 is the compressed pad, held
/// for as many ticks as `d1` counts down; the pounce sets `d1 = 3`. The
/// ceiling-mounted variant additionally hops between the two rows it was
/// given at construction so the pad appears to depress upward.
fn tick_jump_pad(e: &mut Enemy) {
    if e.d1 > 0 {
        e.frame = 1;
        e.d1 -= 1;
    } else {
        e.frame = 0;
    }

    if e.d5 != 0 {
        e.y = if e.frame == 0 { e.d3 } else { e.d4 };
    }
}

/// Port of the `weighted` gravity block in `ProcessActor`
/// (game1.c:7868-7896), which runs *before* an actor's own tick function.
/// Actors flagged `weighted` in `ConstructActor` don't move themselves
/// downward - this does it for them, ramping the fall over five ticks.
fn apply_gravity(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);
    let south_free =
        |x: i32, y: i32| test_sprite_move(Dir4::South, x, y, w, h, level, data) == MoveResult::Free;

    // Embedded in the floor - lift back out.
    if !south_free(e.x, e.y) {
        e.y -= 1;
        e.fall_time = 0;
    }

    if south_free(e.x, e.y + 1) {
        if e.fall_time < 5 {
            e.fall_time += 1;
        }
        if e.fall_time > 1 && e.fall_time < 6 {
            e.y += 1;
        }
        if e.fall_time == 5 {
            if !south_free(e.x, e.y + 1) {
                e.fall_time = 0;
            } else {
                e.y += 1;
            }
        }
    } else {
        e.fall_time = 0;
    }
}

/// `ActMoon` (game1.c:2748-2763). Bobs between two frames on every second
/// tick, choosing the pair that faces the player.
fn tick_moon(e: &mut Enemy, player: &Player) {
    e.d3 = if e.d3 == 0 { 1 } else { 0 };
    if e.d3 != 0 {
        return;
    }

    e.d2 += 1;
    e.frame = if e.x < player.x {
        (e.d2.rem_euclid(2) + 2) as usize
    } else {
        e.d2.rem_euclid(2) as usize
    };
}

/// `ActSmallFlame` (game1.c:4891-4897). A six-frame loop, nothing else.
fn tick_small_flame(e: &mut Enemy) {
    e.frame += 1;
    if e.frame == 6 {
        e.frame = 0;
    }
}

/// `ActFlamePulse` (game1.c:5545-5567). Burns through a sixteen-step frame
/// table, then hides for thirty ticks before firing again.
///
/// NOT PORTED: the smoke decoration emitted as the flame peaks (needs
/// `NewDecoration`) and `SND_FLAME_PULSE`.
fn tick_flame_pulse(e: &mut Enemy) {
    const FRAMES: [usize; 16] = [0, 1, 0, 1, 0, 1, 0, 1, 2, 3, 2, 3, 2, 3, 1, 0];

    if e.d1 == 0 {
        e.frame = FRAMES[e.d2.clamp(0, 15) as usize];
        e.d2 += 1;
        if e.d2 == 16 {
            e.d1 = 30;
            e.d2 = 0;
        }
    } else {
        // Drawn hidden for the whole cooldown - see `draws_hidden`.
        e.d1 -= 1;
    }
}

/// `ActBabyGhost` (game1.c:2870-2915). Hops: falls under the shared gravity
/// pass until it lands, pauses, then rises four rows and falls again.
///
/// The original toggles `Actor.weighted` to switch between falling and
/// rising; `apply_gravity` reads the same flag here.
///
/// NOT PORTED: `SND_BABY_GHOST_LAND` and `SND_BABY_GHOST_JUMP`.
fn tick_baby_ghost(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);

    if e.d4 != 0 {
        e.d4 -= 1;
    } else if e.d1 == DIR2_SOUTH {
        if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data) != MoveResult::Free {
            // Landed: stop falling and wind up for the next hop.
            e.weighted = false;
            e.d1 = DIR2_NORTH;
            e.d4 = 3;
            e.d2 = 4;
            e.frame = 1;
            e.d3 = 1;
        } else if e.d5 == 0 {
            e.frame = 1;
            if e.d3 == 0 {
                e.d4 += 1;
            }
        } else {
            e.d5 -= 1;
        }
    } else if e.d1 == DIR2_NORTH {
        e.y -= 1;
        e.frame = 0;
        e.d2 -= 1;
        if e.d2 == 0 {
            e.d1 = DIR2_SOUTH;
            e.d5 = 3;
            e.weighted = true;
        }
    }
}

/// `ActSpittingTurret` (game1.c:4164-4236). Tracks the player through five
/// firing arcs, snapping between two columns as it turns, then rests.
///
/// NOT PORTED: the projectiles themselves. The original spawns an
/// `ACT_PROJECTILE_*` actor at frames 2/5/8/11/14; actor-spawns-actor isn't
/// available here, so the turret aims and animates but nothing leaves it.
fn tick_spitting_turret(e: &mut Enemy, player: &Player) {
    e.d2 -= 1;
    if e.d2 == 0 {
        e.d1 += 1;
        e.d2 = 3;
        if e.d1 != 3 {
            e.frame += 1;
            // NOT PORTED: frames 2, 5, 8, 11 and 14 each launch a
            // projectile west / south-west / south / south-east / east.
        }
    }

    if e.d1 == 0 {
        if e.y >= player.y - 2 {
            if e.x + 1 > player.x {
                e.frame = 0; // west
                e.x = e.d3;
            } else if e.x + 2 <= player.x {
                e.frame = 12; // east
                e.x = e.d3 + 1;
            }
        } else {
            if e.x - 2 > player.x {
                e.frame = 3; // south-west
                e.x = e.d3;
            } else if e.x + 3 < player.x {
                e.frame = 9; // south-east
                e.x = e.d3 + 1;
            } else if e.x - 2 < player.x && e.x + 3 >= player.x {
                e.frame = 6; // south
                e.x = e.d3 + 1;
            }
            // A separate check rather than folding into the chain above,
            // matching the original's own redundant branch.
            if e.x - 2 == player.x {
                e.frame = 6;
                e.x = e.d3 + 1;
            }
        }
    }

    if e.d1 == 3 {
        e.d2 = 27;
        e.d1 = 0;
    }

    if e.frame > 14 {
        e.frame = 14;
    }
}

/// `ActRedJumper` (game1.c:3493-3600). Crouches while facing the player,
/// then launches along a fixed arc, drifting horizontally as it flies.
///
/// `d2` indexes a table whose even entries are the per-tick vertical delta
/// and whose odd entries are the frame offset, so it advances two at a time.
/// `d1` is the facing (0 west, 3 east) and doubles as the frame base.
///
/// NOT PORTED: `SND_RED_JUMPER_JUMP` and `SND_RED_JUMPER_LAND`.
fn tick_red_jumper(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    const JUMP: [i32; 42] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, -2, 2, -2, 2, -2, 2, -2, 2, -1, 2,
        -1, 2, -1, 2, 0, 2, 0, 2, 1, 1, 1, 1, 1, 1,
    ];
    let (w, h) = (e.width_tiles, e.height_tiles);
    let free = |dir, x, y| test_sprite_move(dir, x, y, w, h, level, data) == MoveResult::Free;
    let frame_at = |d1: i32, idx: i32| (d1 + JUMP[idx.clamp(0, 41) as usize]).max(0) as usize;

    if e.d2 < 5 {
        e.d1 = if e.x > player.x { 0 } else { 3 };
    } else if e.d2 > 16 && e.d2 < 39 {
        if e.d1 == 0 && free(Dir4::West, e.x - 1, e.y) {
            e.x -= 1;
        } else if e.d1 == 3 && free(Dir4::East, e.x + 1, e.y) {
            e.x += 1;
        }
    }

    // Descending tail of the arc: two rows a tick while there's room.
    if e.d2 > 39 {
        // The original's second test pre-increments y, so the step happens
        // even when that test then fails.
        let first = free(Dir4::South, e.x, e.y + 1);
        let second = if first {
            e.y += 1;
            free(Dir4::South, e.x, e.y + 1)
        } else {
            false
        };
        if first && second {
            e.y += 1;
            e.frame = frame_at(e.d1, e.d2 + 1);
            if e.d2 < 39 {
                e.d2 += 2;
            }
        } else {
            e.d2 = 0;
        }
        return;
    }

    match JUMP[e.d2.clamp(0, 41) as usize] {
        -1 => {
            if free(Dir4::North, e.x, e.y - 1) {
                e.y -= 1;
            } else {
                e.d2 = 34;
            }
        }
        -2 => {
            for _ in 0..2 {
                if free(Dir4::North, e.x, e.y - 1) {
                    e.y -= 1;
                } else {
                    e.d2 = 34;
                }
            }
        }
        1 => {
            if free(Dir4::South, e.x, e.y + 1) {
                e.y += 1;
            }
        }
        2 => {
            let first = free(Dir4::South, e.x, e.y - 1);
            let second = if first {
                e.y += 1;
                free(Dir4::South, e.x, e.y - 1)
            } else {
                false
            };
            if first && second {
                e.y += 1;
            } else {
                e.d2 = 0;
                return;
            }
        }
        _ => {}
    }

    e.frame = frame_at(e.d1, e.d2 + 1);
    if e.d2 < 39 {
        e.d2 += 2;
    }
}

/// `ActSmokeEmitter` (game1.c:5598-5611). An invisible marker that puffs
/// smoke roughly one tick in thirty-two.
///
/// NOT PORTED: the smoke itself (needs `NewDecoration`), which is all this
/// actor does - so it is currently an invisible no-op. That is still the
/// right rendering: the original never draws the emitter either, and
/// leaving it out of the table would show a stray sprite instead.
/// `ActDragonfly` (game1.c:4655-4675). Flies straight along a row and
/// turns at walls.
///
/// It tests the move itself rather than going through `AdjustActorMove`,
/// so there is no ledge check and no gravity - it is airborne by
/// construction. That is why the generic walker fallback could not stand in
/// for it: that one refuses to step where there is no floor, and a
/// dragonfly in open sky has floor nowhere, so it reversed every tick and
/// hovered in place.
fn tick_dragonfly(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);
    let free = |dir, x: i32| test_sprite_move(dir, x, e.y, w, h, level, data) == MoveResult::Free;
    if e.d1 != DIR2_WEST {
        if !free(Dir4::East, e.x + 1) {
            e.d1 = DIR2_WEST;
        } else {
            e.x += 1;
            e.d2 = if e.d2 == 0 { 1 } else { 0 };
            e.frame = (e.d2 + 2) as usize;
        }
    } else if !free(Dir4::West, e.x - 1) {
        e.d1 = DIR2_EAST;
    } else {
        e.x -= 1;
        e.frame = usize::from(e.frame == 0);
    }
}

fn tick_smoke_emitter(e: &mut Enemy) {
    e.d1 = e.next_rand(32) as i32;
}

/// `IsSpriteVisible` (game1.c:916-931): does this actor's box overlap the
/// scroll window?
pub fn is_visible_at(x: i32, y: i32, w: i32, h: i32, sx: i32, sy: i32) -> bool {
    use crate::camera::{SCROLL_H, SCROLL_W};
    let horizontal = (sx <= x && sx + SCROLL_W > x) || (sx >= x && x + w > sx);
    let vertical = (sy + SCROLL_H > (y - h) + 1 && sy + SCROLL_H <= y)
        || (y >= sy && sy + SCROLL_H > y);
    horizontal && vertical
}

fn is_visible(e: &Enemy, scroll: &crate::camera::Scroll) -> bool {
    is_visible_at(e.x, e.y, e.width_tiles, e.height_tiles, scroll.x, scroll.y)
}

pub fn tick_enemies(
    mut query: Query<(&mut Enemy, &mut Transform, &mut Sprite, &mut Visibility)>,
    player_q: Query<&Player>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
    scroll: Res<crate::camera::Scroll>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    let level = &level_data.level;
    // Falling off the bottom of the map kills an actor rather than letting
    // it fall forever (game1.c:7849-7852).
    let floor = crate::camera::max_scroll_y(level.width) + crate::camera::SCROLL_H + 3;

    for (mut e, mut transform, mut sprite, mut visibility) in &mut query {
        if e.dead {
            *visibility = Visibility::Hidden;
            continue;
        }
        if e.y > floor {
            e.dead = true;
            *visibility = Visibility::Hidden;
            continue;
        }

        // `ProcessActor`'s activation gate (game1.c:7858-7864). An actor
        // that is off screen and not force-active does not tick at all -
        // which is what leaves a prize perched on a tall structure sitting
        // there until the view reaches it. Seeing a `stay_active` actor
        // wakes it permanently, and if it is also `weighted` it starts
        // falling: that pair is the whole "look up and the bonus drops"
        // mechanic.
        let visible = is_visible(&e, &scroll);
        if visible {
            if e.stay_active {
                e.force_active = true;
            }
        } else if !e.force_active {
            *visibility = Visibility::Hidden;
            continue;
        }

        // The shared gravity pass runs before the actor's own tick, as it
        // does in ProcessActor (game1.c:7868, ahead of `act->tickfunc`).
        if e.weighted {
            apply_gravity(&mut e, &level, &data);
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
            EnemyKind::JumpPad => tick_jump_pad(&mut e),
            EnemyKind::Moon => tick_moon(&mut e, player),
            EnemyKind::SmallFlame => tick_small_flame(&mut e),
            EnemyKind::FlamePulse => tick_flame_pulse(&mut e),
            EnemyKind::BabyGhost => tick_baby_ghost(&mut e, &level, &data),
            EnemyKind::SpittingTurret => tick_spitting_turret(&mut e, player),
            EnemyKind::RedJumper => tick_red_jumper(&mut e, player, &level, &data),
            EnemyKind::SmokeEmitter => tick_smoke_emitter(&mut e),
            EnemyKind::Dragonfly => tick_dragonfly(&mut e, &level, &data),
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

        // `nextDrawMode = DRAW_MODE_FLIPPED` is a *vertical* flip, and it
        // is the only flip the original has. Actors are never mirrored
        // horizontally: `DrawSprite` (game1.c:1210-1264) picks between
        // `DrawSpriteTile` and `DrawSpriteTileFlipped`, and the latter
        // walks rows bottom-to-top - there is no column-reversing variant.
        // Facing is baked into the *artwork* instead, each walker having
        // separate west and east frame runs that its tick function selects
        // between (e.g. `ActRedChomper` game1.c:4327-4340: frames 0/1 walk
        // west, 2/3 walk east). Mirroring on top of that therefore turns
        // an already-correct sprite backwards, which is exactly what was
        // reported.
        transform.scale.y = if flips_vertically(&e) { -1.0 } else { 1.0 };
        *visibility = if draws_hidden(&e) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// Actors the original marks `DRAW_MODE_HIDDEN` for this tick.
fn draws_hidden(e: &Enemy) -> bool {
    match e.kind {
        // Never drawn at all - it only spawns smoke (game1.c:5602).
        EnemyKind::SmokeEmitter => true,
        // Hidden for the whole cooldown between pulses (game1.c:5565).
        EnemyKind::FlamePulse => e.d1 != 0,
        _ => false,
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
        // ActJumpPad (game1.c:2038) flips the ceiling-mounted variant.
        EnemyKind::JumpPad => e.d5 != 0,
        _ => false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{SCROLL_H, SCROLL_W};

    #[test]
    fn an_actor_inside_the_window_is_visible() {
        assert!(is_visible_at(10, 10, 2, 2, 0, 0));
    }

    #[test]
    fn an_actor_beyond_either_edge_is_not() {
        assert!(!is_visible_at(SCROLL_W + 5, 10, 2, 2, 0, 0));
        assert!(!is_visible_at(10, SCROLL_H + 5, 2, 2, 0, 0));
        assert!(!is_visible_at(-10, 10, 2, 2, 0, 0));
    }

    #[test]
    fn an_actor_straddling_an_edge_still_counts() {
        // Half on screen is on screen - otherwise something would wake only
        // once fully inside, a tile late.
        assert!(is_visible_at(-1, 10, 3, 2, 0, 0));
        assert!(is_visible_at(SCROLL_W - 1, 10, 3, 2, 0, 0));
    }

    #[test]
    fn scrolling_up_brings_a_perched_actor_into_view() {
        // The mechanic: a star at row 20 with the player's view starting at
        // row 26 is out of sight, and looking up to row 19 reveals it.
        let star = |sy| is_visible_at(19, 20, 2, 2, 4, sy);
        assert!(!star(26), "should be above the window");
        assert!(star(19), "looking up should reveal it");
    }
}
