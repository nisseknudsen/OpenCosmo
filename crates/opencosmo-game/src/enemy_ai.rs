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
    /// `ActEyePlant` (game1.c:3468-3488) - the most numerous unported
    /// actor in the game. It never moves and never hurts anyone; all it
    /// does is watch, which is done entirely through frame selection.
    EyePlant,
    /// `ActPipeCorner` (game1.c:3052-3057) - exists only to be invisible.
    PipeCorner,
    /// `ActRedGreenSlime` (game1.c:2417-2461), both colours and both the
    /// throb-only and throb-and-drip variants.
    Slime,
    /// `ActArrowPiston` (game1.c:2051-2083).
    ArrowPiston,
    /// `ActFireball` (game1.c:2089-2130).
    Fireball,
    /// `ActSpark` (game1.c:3418-3462) - crawls around the *outside* of a
    /// solid, following its edge.
    Spark,
    /// `ActVerticalMover` (game1.c:2258-2280) - the vertical saw blade.
    VerticalMover,
    /// `ActHorizontalMover` (game1.c:1983-2021) - the floor-mounted sharp
    /// robot, which pauses at each wall before turning.
    HorizontalMover,
    /// `ActPipeEnd` (game1.c:3860-3878) - only the inlet animates.
    PipeEnd,
    /// `ActHeartPlant` (game1.c:2768-2798).
    HeartPlant,
    /// `ActTwoTonsCrusher` (game1.c:2496-2570) - the weight that drops on
    /// a fixed cycle rather than in response to the player.
    TwoTonsCrusher,
    /// `ActStoneHeadCrusher` (game1.c:2598-2652) - the one that *does*
    /// wait for the player to walk underneath.
    StoneHeadCrusher,
    /// `ActSharpRobot` (game1.c:3106-3140) - runs along a ceiling.
    SharpRobot,
    /// `ActFlyingWisp` (game1.c:2461-2490).
    FlyingWisp,
    /// `ActProjectile` (game1.c:2136-2175) - what the turrets and wall
    /// plants fire.
    Projectile,
    /// `ActSpittingWallPlant` (game1.c:4135-4159).
    SpittingWallPlant,
    /// `ActSentryRobot` (game1.c:4566-4633).
    SentryRobot,
    /// `ActBabyGhostEgg` (game1.c:3062-3100).
    BabyGhostEgg,
    /// `ActJumpingBullet` (game1.c:2570-2592).
    JumpingBullet,
}

/// `ACT_PROJECTILE_W` / `_E` (actor.h:151-152), the two the shipped
/// episodes actually fire.
pub const ACT_PROJECTILE_W: u16 = 109;
pub const ACT_PROJECTILE_E: u16 = 110;
/// `ACT_BABY_GHOST` (actor.h:114) - what an egg hatches into.
const ACT_BABY_GHOST: u16 = 65;

/// `DIRP_*` (def.h:62-66) - the five directions a projectile can take.
const DIRP_WEST: i32 = 0;
const DIRP_EAST: i32 = 4;

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
    // --- ActEyePlant (game1.c:5870-5875) ---
    (95, EnemyKind::EyePlant, [0, 0, 0, 0, 0]),            // ACT_EYE_PLANT_FLOOR
    (96, EnemyKind::EyePlant, [0, 0, 0, 0, DRAW_MODE_FLIPPED]), // ACT_EYE_PLANT_CEIL
    // --- ActPipeCorner (game1.c:5804-5813) ---
    (70, EnemyKind::PipeCorner, [0, 0, 0, 0, 0]),          // ACT_PIPE_CORNER_N
    (71, EnemyKind::PipeCorner, [0, 0, 0, 0, 0]),          // ACT_PIPE_CORNER_S
    (72, EnemyKind::PipeCorner, [0, 0, 0, 0, 0]),          // ACT_PIPE_CORNER_W
    (73, EnemyKind::PipeCorner, [0, 0, 0, 0, 0]),          // ACT_PIPE_CORNER_E
    // --- ActRedGreenSlime (game1.c:5736-5739, 6271-6274) ---
    // The drip variants carry their home row in data2 and the drip flag in
    // data5; `Enemy::new` seeds data2 from y, as ConstructActor does.
    (42, EnemyKind::Slime, [0, 0, 0, 0, 0]),               // ACT_GRN_SLIME_THROB
    (43, EnemyKind::Slime, [0, 0, 0, 0, 1]),               // ACT_GRN_SLIME_DRIP
    (236, EnemyKind::Slime, [0, 0, 0, 0, 0]),              // ACT_RED_SLIME_THROB
    (237, EnemyKind::Slime, [0, 0, 0, 0, 1]),              // ACT_RED_SLIME_DRIP
    // --- ActArrowPiston (game1.c:5637-5640) ---
    (3, EnemyKind::ArrowPiston, [0, 0, 0, 0, DIR2_WEST]),  // ACT_ARROW_PISTON_W
    (4, EnemyKind::ArrowPiston, [0, 0, 0, 0, DIR2_EAST]),  // ACT_ARROW_PISTON_E
    // --- ActFireball (game1.c:5643-5646) ---
    // d2/d3 are the launch position it snaps back to; `Enemy::new` seeds
    // them from the spawn, as ConstructActor does.
    (5, EnemyKind::Fireball, [0, 0, 0, 0, DIR2_WEST]),     // ACT_FIREBALL_W
    (6, EnemyKind::Fireball, [0, 0, 0, 0, DIR2_EAST]),     // ACT_FIREBALL_E
    // --- ActSpark (game1.c:5861) ---
    (92, EnemyKind::Spark, [0, 0, 0, 0, 0]),               // ACT_SPARK
    // --- ActVerticalMover / ActHorizontalMover (game1.c:5682, 5822) ---
    (20, EnemyKind::VerticalMover, [0, 0, 0, 0, 0]),       // ACT_SAW_BLADE_VERT
    (78, EnemyKind::HorizontalMover, [8, 0, 0, 0, 1]),     // ACT_SHARP_ROBOT_FLOOR
    // --- ActPipeEnd (game1.c:5886-5889): only the inlet (d2=1) animates ---
    (104, EnemyKind::PipeEnd, [0, 0, 0, 0, 0]),            // ACT_PIPE_OUTLET
    (105, EnemyKind::PipeEnd, [0, 1, 0, 0, 0]),            // ACT_PIPE_INLET
    // --- ActReciprocatingSpikes: the east-facing variant was simply
    // missing from this table; it is the same behavior (game1.c:5848) ---
    (88, EnemyKind::ReciprocatingSpikes, [1, 0, 0, 0, 0]), // ACT_SPIKES_E_RECIP
    // --- crushers, plants and ceiling runners (game1.c:5742-5825) ---
    (55, EnemyKind::HeartPlant, [0, 0, 0, 0, 0]),          // ACT_HEART_PLANT
    (45, EnemyKind::TwoTonsCrusher, [0, 0, 0, 0, 0]),      // ACT_TWO_TONS_CRUSHER
    (47, EnemyKind::StoneHeadCrusher, [0, 0, 0, 0, 0]),    // ACT_STONE_HEAD_CRUSHER
    (80, EnemyKind::SharpRobot, [0, DIR2_WEST, 0, 0, 0]),  // ACT_SHARP_ROBOT_CEIL
    (44, EnemyKind::FlyingWisp, [0, 0, 0, 0, 0]),          // ACT_FLYING_WISP
    // --- projectiles and the things that fire them ---
    (109, EnemyKind::Projectile, [0, 0, 0, 0, DIRP_WEST]), // ACT_PROJECTILE_W
    (110, EnemyKind::Projectile, [0, 0, 0, 0, DIRP_EAST]), // ACT_PROJECTILE_E
    (111, EnemyKind::SpittingWallPlant, [0, 0, 0, 0, 1]),  // ACT_SPIT_WALL_PLANT_E
    (112, EnemyKind::SpittingWallPlant, [0, 0, 0, 0, 0]),  // ACT_SPIT_WALL_PLANT_W
    (127, EnemyKind::SentryRobot, [DIR2_WEST, 0, 0, 0, 4]), // ACT_SENTRY_ROBOT
    (74, EnemyKind::BabyGhostEgg, [0, 0, 0, 0, 1]),        // ACT_BABY_GHOST_EGG_PROX
    (75, EnemyKind::BabyGhostEgg, [0, 0, 0, 0, 0]),        // ACT_BABY_GHOST_EGG
    (46, EnemyKind::JumpingBullet, [0, DIR2_WEST, 0, 0, 0]), // ACT_JUMPING_BULLET
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
    /// `NewActor` requests raised by this actor's behavior this tick, as
    /// (ACT_* id, x, y). Behaviors stay pure over `Enemy` - they queue
    /// here, and `spawn_queued_actors` drains the queue afterwards with
    /// the `Commands` and assets it needs.
    pub spawns: Vec<(u16, i32, i32)>,
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
        // The dripping slime returns to where it started rather than dying
        // off the bottom of the map, so it carries its home row - which
        // `ConstructActor` passes as data2 (game1.c:5739, 6274).
        if kind == EnemyKind::Slime && data[4] != 0 {
            data[1] = y;
        }
        // The fireball returns to its launcher after every pass, so it
        // carries that position in data2/data3 (game1.c:5643-5646).
        if kind == EnemyKind::Fireball {
            data[1] = x;
            data[2] = y;
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
            spawns: Vec::new(),
        }
    }

    /// A bare actor for unit tests: no sprite handles, no app, no window.
    /// The tick functions are pure over `Enemy` + the level, which is what
    /// makes the behaviors testable without standing up Bevy at all.
    #[cfg(test)]
    pub fn default_for_test(kind: EnemyKind) -> Self {
        Enemy {
            kind,
            x: 10,
            y: 10,
            frame: 0,
            d1: 0,
            d2: 0,
            d3: 0,
            d4: 0,
            d5: 0,
            fall_time: 0,
            dead: false,
            width_tiles: 2,
            height_tiles: 2,
            west_free: false,
            east_free: false,
            acrophile: false,
            force_active: true,
            stay_active: false,
            pounce_hits: 0,
            pounce_recoil: 0,
            weighted: false,
            frames: Vec::new(),
            rng: 0x1234_5678,
            spawns: Vec::new(),
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

/// `ActEyePlant` (game1.c:3468-3488). The plant never moves, never damages
/// and never dies - the entire behavior is choosing which of six frames to
/// draw, so that the eye follows the player across the room.
///
/// Frames come in two banks of three (looking west / centre / east); the
/// second bank is the blink, picked on a 2-in-40 roll each tick. The
/// original re-rolls *every* tick, so the blink is a single-tick flicker
/// rather than a held pose - transcribed as-is.
fn tick_eye_plant(e: &mut Enemy, player: &Player) {
    // `random(40) > 37` - two of the forty outcomes, so about 5%.
    e.d2 = if e.next_rand(40) > 37 { 3 } else { 0 };

    // The dead zone is deliberately lopsided: two tiles of slack to the
    // west against one to the east (game1.c:3479-3485).
    e.frame = if e.x - 2 > player.x {
        e.d2 as usize
    } else if e.x + 1 < player.x {
        (e.d2 + 2) as usize
    } else {
        (e.d2 + 1) as usize
    };
}

/// `ActRedGreenSlime` (game1.c:2417-2461). Green and red are the same
/// behavior with different artwork; `data5` picks between throbbing in
/// place and throbbing then dripping down the screen.
///
/// NOT PORTED: the drip sound (`SND_DRIP`), which needs the sound number
/// wired through - the motion is complete without it.
fn tick_slime(e: &mut Enemy, scroll: &crate::camera::Scroll) {
    /// game1.c:2419 - note it holds seven entries but the throb-only path
    /// only ever indexes the first six.
    const THROB_FRAMES: [usize; 7] = [0, 1, 2, 3, 2, 1, 0];

    if e.d5 == 0 {
        // Throb in place forever.
        e.frame = THROB_FRAMES[e.d3 as usize % THROB_FRAMES.len()];
        e.d3 += 1;
        if e.d3 == 6 {
            e.d3 = 0;
        }
        return;
    }

    if e.d4 == 0 {
        // Gathering: throb until a drop is ready to fall.
        e.frame = THROB_FRAMES[(e.d3 as usize) % 6];
        e.d3 += 1;
        if e.d3 == 15 {
            e.d4 = 1;
            e.d3 = 0;
            e.frame = 4;
        }
    } else if e.frame < 6 {
        // Stretching away from the ceiling.
        e.frame += 1;
    } else {
        // Falling. It is not killed at the bottom of the map like other
        // actors - it returns to the ceiling row it started on and begins
        // again, which is why `data2` holds the home row.
        e.y += 1;
        if !is_visible_at(e.x, e.y, e.width_tiles, e.height_tiles, scroll.x, scroll.y) {
            e.y = e.d2;
            e.d4 = 0;
            e.frame = 0;
        }
    }
}

/// `ActArrowPiston` (game1.c:2051-2083). A 32-tick cycle that spends three
/// ticks punching out and three pulling back, and the remaining twenty-six
/// sitting still.
///
/// NOT PORTED: `SND_SPIKES_MOVE` on the two ticks it starts moving.
fn tick_arrow_piston(e: &mut Enemy) {
    if e.d1 < 31 {
        e.d1 += 1;
    } else {
        e.d1 = 0;
    }

    // The two windows overlap in the source's `else if` chain: >28 wins
    // over >25, so 29..31 retract and 26..28 extend.
    let out = if e.d5 == DIR2_WEST { -1 } else { 1 };
    if e.d1 > 28 {
        e.x -= out;
    } else if e.d1 > 25 {
        e.x += out;
    }
}

/// `ActFireball` (game1.c:2089-2130). Waits thirty ticks, then flies until
/// it hits something or leaves the screen, and snaps back to its launcher
/// to start again. `data2`/`data3` hold that launch position.
///
/// NOT PORTED: the smoke puff on impact (`NewDecoration`) and the launch
/// and impact sounds.
fn tick_fireball(
    e: &mut Enemy,
    level: &LevelJson,
    data: &GameData,
    scroll: &crate::camera::Scroll,
) {
    if e.d1 < 30 {
        e.d1 += 1;
    } else {
        let dir = if e.d5 == DIR2_WEST { Dir4::West } else { Dir4::East };
        e.x += if e.d5 == DIR2_WEST { -1 } else { 1 };
        let blocked = test_sprite_move(dir, e.x, e.y, e.width_tiles, e.height_tiles, level, data)
            != MoveResult::Free;
        if blocked {
            e.d1 = 0;
            e.x = e.d2;
            e.y = e.d3;
        }
    }

    // Leaving the view also resets it - this actor is force-active, so
    // without that it would keep flying forever off screen.
    if !is_visible_at(e.x, e.y, e.width_tiles, e.height_tiles, scroll.x, scroll.y) {
        e.d1 = 0;
        e.x = e.d2;
        e.y = e.d3;
    }

    e.frame = usize::from(e.frame == 0);
}

/// `ActSpark` (game1.c:3418-3462). Hugs the outside of a solid: it walks
/// in its current direction, turns *into* a wall it meets, and turns
/// *around* a corner it runs out of - which together trace the perimeter.
/// Moves every other tick.
fn tick_spark(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    const W: i32 = 0;
    const E: i32 = 1;
    const N: i32 = 2;
    const S: i32 = 3;

    e.d5 += 1;
    e.frame = usize::from(e.frame == 0);
    if e.d5 % 2 != 0 {
        return;
    }

    let free = |x: i32, y: i32, dir: Dir4| {
        test_sprite_move(dir, x, y, e.width_tiles, e.height_tiles, level, data) == MoveResult::Free
    };

    match e.d1 {
        x if x == W => {
            e.x -= 1;
            if !free(e.x - 1, e.y, Dir4::West) {
                e.d1 = N;
            } else if free(e.x, e.y + 1, Dir4::South) {
                e.d1 = S;
            }
        }
        x if x == E => {
            e.x += 1;
            if !free(e.x + 1, e.y, Dir4::East) {
                e.d1 = S;
            } else if free(e.x, e.y - 1, Dir4::North) {
                e.d1 = N;
            }
        }
        x if x == N => {
            e.y -= 1;
            if !free(e.x, e.y - 1, Dir4::North) {
                e.d1 = E;
            } else if free(e.x - 1, e.y, Dir4::West) {
                e.d1 = W;
            }
        }
        _ => {
            e.y += 1;
            if !free(e.x, e.y + 1, Dir4::South) {
                e.d1 = W;
            } else if free(e.x + 1, e.y, Dir4::East) {
                e.d1 = E;
            }
        }
    }
}

/// `ActVerticalMover` (game1.c:2258-2280). Rises until the ceiling stops
/// it, falls until the floor does.
///
/// NOT PORTED: `SND_SAW_BLADE_MOVE`, which the original plays every tick
/// the blade is on screen.
fn tick_vertical_mover(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    e.frame = usize::from(e.frame == 0);

    if e.d1 != DIR2_SOUTH {
        if test_sprite_move(Dir4::North, e.x, e.y - 1, e.width_tiles, e.height_tiles, level, data)
            != MoveResult::Free
        {
            e.d1 = DIR2_SOUTH;
        } else {
            e.y -= 1;
        }
    } else if test_sprite_move(Dir4::South, e.x, e.y + 1, e.width_tiles, e.height_tiles, level, data)
        != MoveResult::Free
    {
        e.d1 = DIR2_NORTH;
    } else {
        e.y += 1;
    }
}

/// `ActHorizontalMover` (game1.c:1983-2021). Paces left and right at half
/// speed, pausing `data1` ticks at each end before turning back.
fn tick_horizontal_mover(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    // data3 gates movement to every other tick (the saw blade forces it to
    // 1 and so moves every tick; no saw blade uses this behavior in the
    // shipped episodes).
    e.d3 = i32::from(e.d3 == 0);

    if e.d4 != 0 {
        e.d4 -= 1;
    }
    if e.d3 == 0 {
        return;
    }

    if e.d4 == 0 {
        if e.d2 != DIR2_WEST {
            e.x += 1;
            adjust_actor_move(e, Dir4::East, level, data);
            if !e.east_free {
                e.d2 = DIR2_WEST;
                e.d4 = e.d1;
            }
        } else {
            e.x -= 1;
            adjust_actor_move(e, Dir4::West, level, data);
            if !e.west_free {
                e.d2 = DIR2_EAST;
                e.d4 = e.d1;
            }
        }
    }

    e.frame += 1;
    if e.frame as i32 > e.d5 {
        e.frame = 0;
    }
}

/// `ActPipeEnd` (game1.c:3860-3878). The outlet (`data2` zero) is inert;
/// the inlet flickers between two frames.
///
/// NOT PORTED: the second sprite the original draws three rows below
/// itself, which needs a decoration rather than a behavior.
fn tick_pipe_end(e: &mut Enemy) {
    if e.d2 == 0 {
        return;
    }
    e.d1 += 1;
    e.d3 += 1;
    e.frame = if e.d3 % 2 != 0 { 4 } else { 0 };
    if e.d1 == 4 {
        e.d1 = 1;
    }
}

/// `ActHeartPlant` (game1.c:2768-2798). Snaps open when the player is
/// directly above it, then closes again. The lurch is done by nudging the
/// plant a column sideways on two of its three frames.
///
/// NOT PORTED: `SND_PLANT_MOUTH_OPEN`.
fn tick_heart_plant(e: &mut Enemy, player: &Player) {
    if e.d1 == 0 && e.y > player.y && e.x == player.x {
        e.d1 = 1;
    }

    if e.d1 != 1 {
        return;
    }

    e.d2 += 1;
    if e.d2 != 2 {
        return;
    }
    e.d2 = 0;
    e.frame += 1;

    if e.frame == 3 {
        e.d1 = 0;
        e.frame = 0;
    }
    if e.frame == 1 {
        e.x -= 1;
    }
    if e.frame == 2 {
        e.x += 1;
    }
}

/// `ActTwoTonsCrusher` (game1.c:2496-2570). Drops on a twenty-tick timer
/// regardless of where the player is, accelerating 1/2/4 rows on the way
/// down and decelerating the same way back up.
///
/// NOT PORTED: the impact sound, and the separate base sprite the original
/// draws three rows below itself.
fn tick_two_tons_crusher(e: &mut Enemy) {
    if e.d1 < 20 {
        e.d1 += 1;
    }
    if e.d1 == 19 {
        e.d2 = 1;
    }

    if e.d2 == 1 {
        if e.frame < 3 {
            e.frame += 1;
            e.d3 = match e.frame {
                1 => 1,
                2 => 2,
                _ => 4,
            };
            e.y += e.d3;
        } else {
            e.d2 = 2;
        }
    }

    if e.d2 == 2 {
        if e.frame > 0 {
            e.frame -= 1;
            e.d3 = match e.frame {
                0 => 1,
                1 => 2,
                _ => 4,
            };
            e.y -= e.d3;
        } else {
            e.d2 = 0;
            e.d1 = 0;
            e.d3 = 0;
        }
    }
}

/// `ActStoneHeadCrusher` (game1.c:2598-2652). Waits for the player to pass
/// underneath, drops two rows a tick until it lands, then climbs back to
/// where it started at half speed.
///
/// NOT PORTED: the impact sound and smoke puffs.
fn tick_stone_head_crusher(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    e.d4 = i32::from(e.d4 == 0);

    let blocked = |e: &Enemy, level: &LevelJson, data: &GameData| {
        test_sprite_move(Dir4::South, e.x, e.y, e.width_tiles, e.height_tiles, level, data)
            != MoveResult::Free
    };

    if e.d1 == 0 {
        // The trigger box is deliberately wide - seven columns, offset so
        // it leads the head slightly (game1.c:2605).
        if e.y < player.y && e.x <= player.x + 6 && e.x + 7 > player.x {
            e.d1 = 1;
            e.d2 = e.y; // the row to climb back to
            e.frame = 1;
        } else {
            e.frame = 0;
        }
    } else if e.d1 == 1 {
        e.frame = 1;
        e.y += 1;
        if blocked(e, level, data) {
            e.d1 = 2;
            e.y -= 1;
        } else {
            // It falls a second row in the same tick, testing again.
            e.y += 1;
            if blocked(e, level, data) {
                e.d1 = 2;
                e.y -= 1;
            }
        }
    } else if e.d1 == 2 {
        e.frame = 0;
        if e.y == e.d2 {
            e.d1 = 0;
        } else if e.d4 != 0 {
            e.y -= 1;
        }
    }
}

/// `ActSharpRobot` (game1.c:3106-3140). Runs along the underside of a
/// ceiling, turning both at a wall and at the end of the ceiling - the
/// second test is what stops it running off into open air.
fn tick_sharp_robot(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    e.d3 = i32::from(e.d3 == 0);
    if e.d3 == 0 {
        return;
    }

    if e.d4 != 0 {
        e.d4 -= 1;
    } else {
        let (step, dir) = if e.d2 == DIR2_EAST { (1, Dir4::East) } else { (-1, Dir4::West) };
        let ahead = test_sprite_move(
            dir, e.x + step, e.y, e.width_tiles, e.height_tiles, level, data,
        );
        let ceiling = test_sprite_move(
            dir, e.x + step, e.y - 1, e.width_tiles, e.height_tiles, level, data,
        );
        if ahead != MoveResult::Free || ceiling == MoveResult::Free {
            e.d4 = 4;
            e.d2 = if e.d2 == DIR2_EAST { DIR2_WEST } else { DIR2_EAST };
        } else {
            e.x += step;
        }
    }

    e.frame = usize::from(e.frame == 0);
}

/// `ActFlyingWisp` (game1.c:2461-2490). A 64-tick loop: still, then a slow
/// climb, then a faster drop back, flipped on the way down.
fn tick_flying_wisp(e: &mut Enemy) {
    e.frame = usize::from(e.frame == 0);

    if e.d1 < 63 {
        e.d1 += 1;
    } else {
        e.d1 = 0;
    }

    if e.d1 > 50 {
        e.y += 2;
        if e.d1 < 55 {
            e.y -= 1;
        }
    } else if e.d1 > 34 {
        if e.d1 < 47 {
            e.y -= 1;
        }
        if e.d1 < 45 {
            e.y -= 1;
        }
    }
}

/// `ActProjectile` (game1.c:2136-2175). Flies in a straight line until it
/// leaves the view, then dies. Only west and east are used by the shipped
/// episodes' actors, but the diagonal and downward cases are the same
/// behavior with a different step.
///
/// NOT PORTED: `SND_PROJECTILE_LAUNCH` on its first tick.
fn tick_projectile(e: &mut Enemy, scroll: &crate::camera::Scroll) {
    if !is_visible_at(e.x, e.y, e.width_tiles, e.height_tiles, scroll.x, scroll.y) {
        e.dead = true;
        return;
    }
    e.frame = usize::from(e.frame == 0);
    match e.d5 {
        DIRP_WEST => e.x -= 1,
        1 => {
            e.x -= 1;
            e.y += 1;
        }
        2 => e.y += 1,
        3 => {
            e.x += 1;
            e.y += 1;
        }
        _ => e.x += 1,
    }
}

/// `ActSpittingWallPlant` (game1.c:4135-4159). A fifty-tick cycle that
/// opens on tick 42 and spits on tick 45.
fn tick_spitting_wall_plant(e: &mut Enemy) {
    e.d4 += 1;

    if e.d4 == 50 {
        e.d4 = 0;
        e.frame = 0;
    }
    if e.d4 == 42 {
        e.frame = 1;
    }
    if e.d4 == 45 {
        e.frame = 2;
        // d5 carries the facing; the projectile appears clear of the
        // plant's own tiles on the side it faces (game1.c:4152-4156).
        if e.d5 == 0 {
            e.spawns.push((ACT_PROJECTILE_W, e.x - 1, e.y - 1));
        } else {
            e.spawns.push((ACT_PROJECTILE_E, e.x + 4, e.y - 1));
        }
    }
}

/// `ActSentryRobot` (game1.c:4566-4633). Paces at half speed and, while
/// the lights are on, occasionally stops to aim and fire at the player.
///
/// NOT PORTED: the `areLightsActive` gate, which belongs to the light
/// switch - unported, and its absence leaves the robot always willing to
/// fire, which is the lights-on behavior.
fn tick_sentry_robot(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    e.d3 = i32::from(e.d3 == 0);
    if e.d3 != 0 {
        return;
    }

    // `GameRand() % 50 > 48` - one outcome in fifty.
    if e.next_rand(50) > 48 && e.d4 == 0 {
        e.d4 = 10;
    }

    if e.d4 != 0 {
        e.d2 = i32::from(e.d2 == 0);
        e.d4 -= 1;

        if e.d4 == 1 {
            e.d1 = if e.x + 1 > player.x { DIR2_WEST } else { DIR2_EAST };
            if e.d1 != DIR2_WEST {
                e.spawns.push((ACT_PROJECTILE_E, e.x + 3, e.y - 1));
            } else {
                e.spawns.push((ACT_PROJECTILE_W, e.x - 1, e.y - 1));
            }
        }

        e.frame = match (e.d1 != DIR2_WEST, e.d2 != 0) {
            (true, true) => 5,
            (true, false) => 0,
            (false, true) => 6,
            (false, false) => 2,
        };
        return;
    }

    // Not firing: pace.
    if e.d1 == DIR2_WEST {
        e.x -= 1;
        adjust_actor_move(e, Dir4::West, level, data);
        if !e.west_free {
            e.d1 = DIR2_EAST;
        }
    } else {
        e.x += 1;
        adjust_actor_move(e, Dir4::East, level, data);
        if !e.east_free {
            e.d1 = DIR2_WEST;
        }
    }
    e.frame = if e.d1 == DIR2_WEST { 2 } else { 0 };
}

/// `ActBabyGhostEgg` (game1.c:3062-3100). Sits and jiggles until the
/// player comes near, then cracks for twenty ticks and hatches.
///
/// `data5` selects the trigger: the plain egg hatches when the player is
/// level with or below it and within its column range; the `_PROX` variant
/// never triggers on proximity at all (game1.c:3079).
///
/// NOT PORTED: the shell shards and the crack/hatch sounds.
fn tick_baby_ghost_egg(e: &mut Enemy, player: &Player) {
    if e.d2 != 0 {
        e.frame = 2;
    } else if e.next_rand(70) == 0 && e.d3 == 0 {
        e.d3 = 2;
    } else {
        e.frame = 0;
    }

    if e.d3 != 0 {
        e.d3 -= 1;
        e.frame = 1;
    }

    if e.d5 == 0 && e.d1 == 0 && e.y <= player.y && e.x - 6 < player.x && e.x + 4 > player.x {
        e.d1 = 1;
        e.d2 = 20;
    }

    if e.d2 > 1 {
        e.d2 -= 1;
    } else if e.d2 == 1 {
        e.dead = true;
        e.spawns.push((ACT_BABY_GHOST, e.x, e.y));
    }
}

/// `ActJumpingBullet` (game1.c:2570-2592). Bounces along a fixed sixteen
/// step arc, reversing direction at the end of each one.
fn tick_jumping_bullet(e: &mut Enemy) {
    /// game1.c:2572 - the arc, as per-tick row offsets.
    const Y_JUMP: [i32; 16] = [-2, -2, -2, -2, -1, -1, -1, 0, 0, 1, 1, 1, 2, 2, 2, 2];

    if e.d2 == DIR2_WEST {
        e.x -= 1;
    } else {
        e.x += 1;
    }

    e.y += Y_JUMP[e.d3 as usize % Y_JUMP.len()];
    e.d3 += 1;

    if e.d3 == 16 {
        e.d2 = i32::from(e.d2 == 0);
        e.d3 = 0;
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
            EnemyKind::EyePlant => tick_eye_plant(&mut e, player),
            EnemyKind::ArrowPiston => tick_arrow_piston(&mut e),
            EnemyKind::Fireball => tick_fireball(&mut e, &level, &data, &scroll),
            EnemyKind::Spark => tick_spark(&mut e, &level, &data),
            EnemyKind::VerticalMover => tick_vertical_mover(&mut e, &level, &data),
            EnemyKind::HorizontalMover => tick_horizontal_mover(&mut e, &level, &data),
            EnemyKind::PipeEnd => tick_pipe_end(&mut e),
            EnemyKind::HeartPlant => tick_heart_plant(&mut e, player),
            EnemyKind::TwoTonsCrusher => tick_two_tons_crusher(&mut e),
            EnemyKind::StoneHeadCrusher => {
                tick_stone_head_crusher(&mut e, player, &level, &data)
            }
            EnemyKind::SharpRobot => tick_sharp_robot(&mut e, &level, &data),
            EnemyKind::FlyingWisp => tick_flying_wisp(&mut e),
            EnemyKind::Projectile => tick_projectile(&mut e, &scroll),
            EnemyKind::SpittingWallPlant => tick_spitting_wall_plant(&mut e),
            EnemyKind::SentryRobot => tick_sentry_robot(&mut e, player, &level, &data),
            EnemyKind::BabyGhostEgg => tick_baby_ghost_egg(&mut e, player),
            EnemyKind::JumpingBullet => tick_jumping_bullet(&mut e),
            EnemyKind::Slime => tick_slime(&mut e, &scroll),
            // `nextDrawMode = DRAW_MODE_HIDDEN` and nothing else
            // (game1.c:3052-3057).
            EnemyKind::PipeCorner => {}
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
/// Drains the `NewActor` requests behaviors queued this tick and builds
/// the entities. Separate from `tick_enemies` so the behaviors themselves
/// stay pure functions over `Enemy` - which is what lets every one of them
/// be unit tested without an app, a window or an audio device.
pub fn spawn_queued_actors(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<GameData>,
    mut query: Query<&mut Enemy>,
) {
    // Collected first so the borrow on the query ends before spawning,
    // which would otherwise conflict with the new entities.
    let mut requests = Vec::new();
    for mut e in &mut query {
        if !e.spawns.is_empty() {
            requests.append(&mut e.spawns);
        }
    }
    for (act_type, x, y) in requests {
        crate::actors::spawn_one_actor(&mut commands, &asset_server, &data, act_type, x, y);
    }
}

fn draws_hidden(e: &Enemy) -> bool {
    match e.kind {
        // Never drawn at all - it only spawns smoke (game1.c:5602).
        EnemyKind::SmokeEmitter => true,
        // The pipe corners exist purely to be invisible: the artwork is
        // already in the map tiles, and the actor only marks the corner
        // (game1.c:3052-3057).
        EnemyKind::PipeCorner => true,
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
        // The ceiling-mounted eye plant is the floor one upside down
        // (game1.c:5874).
        EnemyKind::EyePlant => e.d5 == DRAW_MODE_FLIPPED,
        // The wisp flips for the falling half of its loop (game1.c:2478).
        EnemyKind::FlyingWisp => e.d1 > 50,
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

    /// Builds a level out of ASCII art: `#` is solid, anything else empty.
    /// Rows are equal length. This is the whole test rig - the behaviors
    /// are pure over `Enemy` + `LevelJson` + `GameData`, so nothing here
    /// touches Bevy, a window, or the audio device.
    fn world(rows: &[&str]) -> (LevelJson, GameData) {
        const SOLID: u16 = 8; // attr index 8/8 = 1
        let width = rows[0].len();
        let mut tiles = Vec::with_capacity(width * rows.len());
        for row in rows {
            assert_eq!(row.len(), width, "test world rows must be equal length");
            tiles.extend(row.chars().map(|c| if c == '#' { SOLID } else { 0 }));
        }
        let level = LevelJson {
            name: "test".into(),
            width,
            height: rows.len(),
            tiles,
            actors: Vec::new(),
            backdrop: None,
            music: None,
            has_h_scroll_backdrop: false,
            has_v_scroll_backdrop: false,
        };
        let mut tile_attrs = vec![0u8; 16];
        tile_attrs[1] = TILE_ATTR_BLOCK_SOUTH
            | TILE_ATTR_BLOCK_NORTH
            | TILE_ATTR_BLOCK_WEST
            | TILE_ATTR_BLOCK_EAST;
        let data = GameData {
            root: std::path::PathBuf::new(),
            asset_prefix: String::new(),
            episode: 1,
            tileset: crate::data::TilesetJson {
                tile_size: 8,
                atlas_cols: 1,
                solid_tile_count: 1,
                masked_tile_count: 0,
            },
            tile_attrs,
        };
        (level, data)
    }

    #[test]
    fn the_arrow_piston_returns_to_where_it_started() {
        // A 32-tick cycle that punches out and pulls back must be a closed
        // loop - a piston that drifted a tile per cycle would walk off.
        for dir in [DIR2_WEST, DIR2_EAST] {
            let mut e = Enemy::default_for_test(EnemyKind::ArrowPiston);
            e.d5 = dir;
            let start = e.x;
            let mut extreme = e.x;
            for _ in 0..32 {
                tick_arrow_piston(&mut e);
                if (e.x - start).abs() > (extreme - start).abs() {
                    extreme = e.x;
                }
            }
            assert_eq!(e.x, start, "dir {dir}: piston drifted over one cycle");
            assert_eq!(
                (extreme - start).abs(),
                3,
                "dir {dir}: it should reach three tiles out"
            );
        }
        // ...and it extends the way it is aimed.
        let mut w = Enemy::default_for_test(EnemyKind::ArrowPiston);
        w.d5 = DIR2_WEST;
        for _ in 0..28 {
            tick_arrow_piston(&mut w);
        }
        assert!(w.x < 10, "the west-facing piston must punch west");
    }

    #[test]
    fn the_fireball_waits_then_flies_and_resets_on_impact() {
        // Open corridor with a wall to the west.
        let (level, data) = world(&[
            "..........",
            "#....o....",
            "##########",
        ]);
        let scroll = crate::camera::Scroll::default();
        let mut e = Enemy::default_for_test(EnemyKind::Fireball);
        e.x = 5;
        e.y = 1;
        e.d2 = 5;
        e.d3 = 1;
        e.d5 = DIR2_WEST;
        e.width_tiles = 1;
        e.height_tiles = 1;

        for _ in 0..30 {
            tick_fireball(&mut e, &level, &data, &scroll);
        }
        assert_eq!(e.x, 5, "it should still be waiting on its launcher");

        let mut min_x = e.x;
        for _ in 0..40 {
            tick_fireball(&mut e, &level, &data, &scroll);
            min_x = min_x.min(e.x);
        }
        assert!(min_x < 5, "it should have flown west");
        assert_eq!((e.x, e.y), (5, 1), "and snapped back to its launcher");
    }

    #[test]
    fn the_saw_blade_bounces_between_floor_and_ceiling() {
        let (level, data) = world(&[
            "####",
            "....",
            "....",
            "....",
            "####",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::VerticalMover);
        e.x = 1;
        e.y = 2;
        e.width_tiles = 1;
        e.height_tiles = 1;
        let mut rows = std::collections::BTreeSet::new();
        for _ in 0..60 {
            tick_vertical_mover(&mut e, &level, &data);
            rows.insert(e.y);
        }
        assert_eq!(
            rows,
            [1, 2, 3].into_iter().collect(),
            "it should sweep the open rows and never enter a wall"
        );
    }

    #[test]
    fn the_spark_crawls_around_a_block_instead_of_through_it() {
        let (level, data) = world(&[
            "........",
            "..####..",
            "..####..",
            "........",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::Spark);
        e.x = 2;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;
        for _ in 0..200 {
            tick_spark(&mut e, &level, &data);
            let inside = (2..6).contains(&e.x) && (1..3).contains(&e.y);
            assert!(!inside, "the spark entered the solid at ({}, {})", e.x, e.y);
        }
    }

    #[test]
    fn only_the_pipe_inlet_animates() {
        let mut outlet = Enemy::default_for_test(EnemyKind::PipeEnd);
        outlet.d2 = 0;
        let mut inlet = Enemy::default_for_test(EnemyKind::PipeEnd);
        inlet.d2 = 1;
        let mut inlet_frames = std::collections::BTreeSet::new();
        for _ in 0..10 {
            tick_pipe_end(&mut outlet);
            tick_pipe_end(&mut inlet);
            inlet_frames.insert(inlet.frame);
        }
        assert_eq!(outlet.frame, 0, "the outlet is inert");
        assert_eq!(inlet_frames, [0, 4].into_iter().collect());
    }

    #[test]
    fn the_wall_plant_spits_once_per_cycle_on_the_side_it_faces() {
        for (facing, expect) in [(0u8, ACT_PROJECTILE_W), (1, ACT_PROJECTILE_E)] {
            let mut e = Enemy::default_for_test(EnemyKind::SpittingWallPlant);
            e.d5 = facing as i32;
            let mut fired = Vec::new();
            for _ in 0..150 {
                tick_spitting_wall_plant(&mut e);
                fired.append(&mut e.spawns);
            }
            assert_eq!(fired.len(), 3, "one shot per fifty-tick cycle");
            assert!(
                fired.iter().all(|(a, ..)| *a == expect),
                "facing {facing} fired the wrong way: {fired:?}"
            );
        }
    }

    #[test]
    fn a_projectile_flies_straight_and_dies_off_screen() {
        let scroll = crate::camera::Scroll::default();
        let mut e = Enemy::default_for_test(EnemyKind::Projectile);
        e.d5 = DIRP_EAST;
        e.x = 5;
        let start_y = e.y;
        for _ in 0..10 {
            tick_projectile(&mut e, &scroll);
        }
        assert_eq!(e.x, 15, "it should have flown ten columns east");
        assert_eq!(e.y, start_y, "and not drifted vertically");
        assert!(!e.dead);

        // Carried past the edge of the view, it dies rather than flying on.
        for _ in 0..200 {
            tick_projectile(&mut e, &scroll);
        }
        assert!(e.dead, "a projectile must not outlive the screen");
    }

    #[test]
    fn the_sentry_robot_fires_toward_the_player() {
        let (level, data) = world(&[
            "................",
            "................",
            "################",
        ]);
        let fire_dir = |player_x: i32| {
            let mut e = Enemy::default_for_test(EnemyKind::SentryRobot);
            e.x = 8;
            e.y = 1;
            e.width_tiles = 1;
            e.height_tiles = 1;
            let mut p = Player::spawn_at(player_x, 1);
            p.x = player_x;
            p.y = 1;
            for _ in 0..4000 {
                tick_sentry_robot(&mut e, &p, &level, &data);
                if let Some((act, ..)) = e.spawns.first() {
                    return *act;
                }
            }
            panic!("the sentry never fired in 4000 ticks");
        };
        assert_eq!(fire_dir(1), ACT_PROJECTILE_W, "player west -> fire west");
        assert_eq!(fire_dir(15), ACT_PROJECTILE_E, "player east -> fire east");
    }

    #[test]
    fn the_egg_hatches_into_a_baby_ghost_when_approached() {
        let mut e = Enemy::default_for_test(EnemyKind::BabyGhostEgg);
        e.x = 10;
        e.y = 10;

        // Nobody near: it sits there indefinitely.
        let mut away = Player::spawn_at(40, 10);
        away.x = 40;
        away.y = 10;
        for _ in 0..100 {
            tick_baby_ghost_egg(&mut e, &away);
        }
        assert!(!e.dead, "it should not hatch with nobody near");
        assert!(e.spawns.is_empty());

        let mut near = Player::spawn_at(8, 12);
        near.x = 8;
        near.y = 12;
        for _ in 0..40 {
            // `tick_enemies` skips dead actors, so stop when it dies -
            // otherwise this would hatch the same egg over and over.
            if e.dead {
                break;
            }
            tick_baby_ghost_egg(&mut e, &near);
        }
        assert!(e.dead, "it should have hatched");
        assert_eq!(e.spawns, vec![(ACT_BABY_GHOST, 10, 10)]);
    }

    #[test]
    fn the_prox_egg_ignores_a_passing_player() {
        // data5 = 1 disables the proximity trigger entirely
        // (game1.c:3079) - it is hatched by other means.
        let mut e = Enemy::default_for_test(EnemyKind::BabyGhostEgg);
        e.d5 = 1;
        let mut near = Player::spawn_at(8, 12);
        near.x = 8;
        near.y = 12;
        for _ in 0..200 {
            tick_baby_ghost_egg(&mut e, &near);
        }
        assert!(!e.dead, "the _PROX variant must not hatch on proximity");
    }

    #[test]
    fn the_jumping_bullet_arcs_and_reverses() {
        let mut e = Enemy::default_for_test(EnemyKind::JumpingBullet);
        e.d2 = DIR2_WEST;
        e.x = 50;
        let start_y = e.y;
        for _ in 0..16 {
            tick_jumping_bullet(&mut e);
        }
        // The arc sums to zero, so one bounce returns to the start row.
        assert_eq!(e.y, start_y, "the arc must close");
        assert_eq!(e.x, 34, "and it travels sixteen columns per bounce");
        assert_eq!(e.d2, DIR2_EAST, "then turns around");
    }

    #[test]
    fn the_two_tons_crusher_returns_to_its_rest_height() {
        // It accelerates 1/2/4 down and decelerates the same way up, so the
        // two halves must cancel exactly or it would walk down the screen.
        let mut e = Enemy::default_for_test(EnemyKind::TwoTonsCrusher);
        let rest = e.y;
        let mut lowest = e.y;
        for _ in 0..200 {
            tick_two_tons_crusher(&mut e);
            lowest = lowest.max(e.y);
        }
        assert_eq!(e.y % 7, rest % 7, "the cycle must be closed");
        assert_eq!(lowest - rest, 7, "it drops seven rows: 1 + 2 + 4");
    }

    #[test]
    fn the_stone_head_waits_for_the_player_then_drops() {
        let (level, data) = world(&[
            "........",
            "........",
            "........",
            "........",
            "########",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::StoneHeadCrusher);
        e.x = 2;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;

        // Player nowhere near: it stays put.
        let away = Player::spawn_at(40, 3);
        for _ in 0..20 {
            tick_stone_head_crusher(&mut e, &away, &level, &data);
        }
        assert_eq!(e.y, 0, "it should not drop with nobody underneath");

        // Player underneath: it falls to the floor, then climbs home.
        let mut under = Player::spawn_at(3, 3);
        under.x = 3;
        under.y = 3;
        let mut lowest = e.y;
        for _ in 0..10 {
            tick_stone_head_crusher(&mut e, &under, &level, &data);
            lowest = lowest.max(e.y);
        }
        assert!(lowest > 0, "it should have dropped");
        assert!(lowest < 4, "and stopped on the floor, not through it");
        for _ in 0..40 {
            tick_stone_head_crusher(&mut e, &away, &level, &data);
        }
        assert_eq!(e.y, 0, "it should climb back to where it started");
    }

    #[test]
    fn the_heart_plant_only_opens_for_a_player_overhead() {
        let mut e = Enemy::default_for_test(EnemyKind::HeartPlant);
        e.x = 10;
        e.y = 10;
        let mut beside = Player::spawn_at(4, 4);
        beside.x = 4;
        beside.y = 4;
        for _ in 0..20 {
            tick_heart_plant(&mut e, &beside);
        }
        assert_eq!(e.frame, 0, "it stays shut for a player off to the side");

        let mut above = Player::spawn_at(10, 4);
        above.x = 10;
        above.y = 4;
        let mut opened = false;
        for _ in 0..240 {
            tick_heart_plant(&mut e, &above);
            opened |= e.frame > 0;
            // The lurch is a column left on frame 1 and back on frame 2.
            // Over many open/close cycles that must not accumulate - a
            // plant that drifted would crawl across the level.
            assert!(
                (9..=10).contains(&e.x),
                "the plant drifted to column {}",
                e.x
            );
        }
        assert!(opened, "it should open for a player directly above");
    }

    #[test]
    fn the_sharp_robot_stays_under_its_ceiling() {
        // A ceiling that runs out: the robot must turn at the gap rather
        // than carry on into open air.
        let (level, data) = world(&[
            "#####...",
            "........",
            "........",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::SharpRobot);
        e.x = 1;
        e.y = 1;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d2 = DIR2_EAST;
        for _ in 0..120 {
            tick_sharp_robot(&mut e, &level, &data);
            assert!(
                (0..5).contains(&e.x),
                "it ran out from under the ceiling to x={}",
                e.x
            );
        }
    }

    #[test]
    fn the_flying_wisp_loops_back_to_its_start() {
        let mut e = Enemy::default_for_test(EnemyKind::FlyingWisp);
        let start = e.y;
        for _ in 0..64 {
            tick_flying_wisp(&mut e);
        }
        assert_eq!(e.y, start, "a 64-tick loop must close");
    }

    /// An eye plant far from any wall, so only the player's column matters.
    fn eye_plant(x: i32) -> Enemy {
        let mut e = Enemy::default_for_test(EnemyKind::EyePlant);
        e.x = x;
        e
    }

    fn player_at(x: i32) -> Player {
        let mut p = Player::spawn_at(x, 10);
        p.x = x;
        p
    }

    #[test]
    fn the_eye_plant_looks_toward_the_player() {
        // Frames are two banks of three: west / centre / east, then the
        // same again for the blink. Mask off the blink to test the aim.
        let aim = |plant_x: i32, player_x: i32| {
            let mut e = eye_plant(plant_x);
            tick_eye_plant(&mut e, &player_at(player_x));
            e.frame % 3
        };
        assert_eq!(aim(20, 5), 0, "player far west -> look west");
        assert_eq!(aim(20, 60), 2, "player far east -> look east");
        assert_eq!(aim(20, 20), 1, "player level with it -> look ahead");
    }

    #[test]
    fn the_eye_plants_dead_zone_is_lopsided() {
        // Two tiles of slack to the west against one to the east
        // (game1.c:3479-3485) - not symmetric, and worth pinning because
        // it would be very easy to "tidy" into a symmetric test.
        let aim = |player_x: i32| {
            let mut e = eye_plant(20);
            tick_eye_plant(&mut e, &player_at(player_x));
            e.frame % 3
        };
        assert_eq!(aim(17), 0, "three west is outside the dead zone");
        assert_eq!(aim(18), 1, "two west is still centre");
        assert_eq!(aim(21), 1, "one east is still centre");
        assert_eq!(aim(22), 2, "two east is outside it");
    }

    #[test]
    fn the_eye_plant_never_moves_or_dies() {
        let mut e = eye_plant(20);
        let (x, y) = (e.x, e.y);
        for _ in 0..200 {
            tick_eye_plant(&mut e, &player_at(5));
        }
        assert_eq!((e.x, e.y), (x, y), "it is rooted to the spot");
        assert!(!e.dead);
    }

    #[test]
    fn the_eye_plant_blinks_sometimes_but_rarely() {
        // 2-in-40 per tick. Over 4000 ticks that is ~200; the bounds are
        // wide enough not to be flaky but tight enough to catch a blink
        // that never fires or fires constantly.
        let mut e = eye_plant(20);
        let p = player_at(5);
        let blinks = (0..4000)
            .filter(|_| {
                tick_eye_plant(&mut e, &p);
                e.frame >= 3
            })
            .count();
        assert!(
            (40..800).contains(&blinks),
            "blinked {blinks} times in 4000 ticks, expected roughly 200"
        );
    }

    #[test]
    fn a_throbbing_slime_cycles_and_stays_put() {
        let mut e = Enemy::default_for_test(EnemyKind::Slime);
        e.d5 = 0;
        let scroll = crate::camera::Scroll::default();
        let home = e.y;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..60 {
            tick_slime(&mut e, &scroll);
            seen.insert(e.frame);
        }
        assert_eq!(e.y, home, "a throb-only slime never falls");
        assert_eq!(seen, [0, 1, 2, 3].into_iter().collect());
    }

    #[test]
    fn a_dripping_slime_falls_and_returns_home() {
        let mut e = Enemy::default_for_test(EnemyKind::Slime);
        e.d5 = 1;
        e.y = 8;
        e.d2 = 8; // home row, as ConstructActor seeds it
        let scroll = crate::camera::Scroll::default();
        let mut fell_below = false;
        for _ in 0..400 {
            tick_slime(&mut e, &scroll);
            if e.y > 8 {
                fell_below = true;
            }
        }
        assert!(fell_below, "it should detach and fall at some point");
        assert!(
            e.y >= 8,
            "it must never end up above the ceiling it hangs from"
        );
    }

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
