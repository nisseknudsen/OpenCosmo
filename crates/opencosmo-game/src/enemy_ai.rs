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
    /// `ActPipeCorner` (game1.c:3052-3057). The tick itself only hides the
    /// actor; the corner's real job - turning a rider onto the next leg -
    /// is `run_pipes`, because it acts on the player rather than itself.
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
    /// `ActWormCrate` (game1.c:4680-4726).
    WormCrate,
    /// `ActSplittingPlatform` (game1.c:3354-3410).
    SplittingPlatform,
    /// `ActDoor` (game1.c:2174-2188) - stamps itself into the map as solid
    /// until its switch is hit.
    Door,
    /// `ActRocket` (game1.c:5179-5262).
    Rocket,
    /// `ActJumpPadRobot` (game1.c:2194-2222).
    JumpPadRobot,
    /// `ActIvyPlant` (game1.c:4774-4812).
    IvyPlant,
    /// `ActHeadSwitch` (game1.c:2160-2170) - pounced to unlock its colour
    /// of door.
    HeadSwitch,
    /// `ActFootSwitch` (game1.c:1893-1977), for the four sprites where it
    /// is *not* a no-op: the knob that a bomb blast drives down.
    FootSwitch,
    /// `ActMysteryWall` (game1.c:3820-3854).
    MysteryWall,
    /// `ActForceField` (game1.c:4346-4392) - a beam rather than a body:
    /// it draws itself cell by cell until a wall stops it, and hurts the
    /// player anywhere along that line.
    ForceField,
    /// `ActPusherRobot` (game1.c:4488-4560) - shoves the player rather
    /// than hurting them.
    PusherRobot,
    /// `ActMonument` (game1.c:5338-5390) - a nine-tile pillar that two
    /// blasts bring down for a large score.
    Monument,
    /// `ActSatellite` (game1.c:4728-4770).
    Satellite,
    /// `ActTulipLauncher` (game1.c:5395-5450).
    TulipLauncher,
    /// `ActEpisode1End` (game1.c:5470-5482) and `ActExitLineHorizontal`
    /// (game1.c:5487-5498) - invisible trigger lines.
    TriggerLine,
    /// `ActScooter` (game1.c:5303-5330).
    Scooter,
    /// `ActBearTrap` (game1.c:4933-4990).
    BearTrap,
    /// `ActBeamRobot` (game1.c:3280-3350) - paces, with a vertical beam
    /// standing on its head.
    BeamRobot,
    /// `ActTransporter` (game1.c:4075-4130).
    Transporter,
    /// `ActBoss` (game1.c:5588-5838) - the five-phase fight that ends the
    /// episode.
    Boss,
    /// `ActFrozenDN` (game1.c:5454-5520).
    FrozenDN,
    /// `ActSpeechBubble` (game1.c:5290-5310) - the one-shot "WHOA!" and
    /// friends the player says the first time they meet something.
    SpeechBubble,
}

/// `ACT_SPEECH_*` (actor.h:271-282).
pub const ACT_SPEECH_OUCH: u16 = 235;
pub const ACT_SPEECH_WHOA: u16 = 244;
pub const ACT_SPEECH_UMPH: u16 = 245;
pub const ACT_SPEECH_WOW_50K: u16 = 246;

/// Pounces needed to finish the boss (game1.c:5595). The harder variant
/// the source can be built with wants 18; the shipped episodes use 12.
const BOSS_HITS: i32 = 12;

/// `TILE_SWITCH_BLOCK_1` (graphics.h:124) - the solid a monument stands as.
const TILE_SWITCH_BLOCK: u16 = 0x3d88;
/// `ACT_PARACHUTE_BALL` (actor.h:132). Note this is 86, not 22 - 22 is
/// `ACT_SAW_BLADE_HORIZ`, which is what this used to throw.
const ACT_PARACHUTE_BALL: u16 = 86;
/// `ACT_HAMBURGER` (actor.h) - what a destroyed satellite drops.
const ACT_HAMBURGER: u16 = 82;
/// `DIR8_*` as table indices, for the pipe corners' `d5`.
const DIR8_NORTH_I: i32 = 1;
const DIR8_EAST_I: i32 = 3;
const DIR8_SOUTH_I: i32 = 5;
const DIR8_WEST_I: i32 = 7;

/// Decoration sprites the behaviours ask for (sprite.h:37, 119-120).
const SPR_SPARKLE_SHORT: u16 = 15;
const SPR_SMOKE: u16 = 97;
const SPR_SMOKE_LARGE: u16 = 98;
/// Shard sprites (sprite.h:86, 153, 166, 185).
const SPR_MONUMENT: u16 = 64;
const SPR_WORM_CRATE_SHARDS: u16 = 131;
const SPR_SATELLITE_SHARDS: u16 = 144;
const SPR_FALLING_FLOOR: u16 = 163;
/// `SPR_BGHOST_EGG_SHARD_1`..`_4`. These are *not* consecutive - the first
/// two sit at 76-77 and the other two at 132-133 (sprite.h:98-99, 154-155).
const SPR_BGHOST_EGG_SHARDS: [u16; 4] = [76, 77, 132, 133];
/// `SPR_PYRAMID` (sprite.h) and `ACT_STAR_FLOAT` (actor.h:45).
const SPR_PYRAMID: u16 = 49;
/// `SPR_ROCKET` (sprite.h).
const SPR_ROCKET: u16 = 188;
const SPR_BOSS: u16 = 102;
const SPR_FROZEN_DN: u16 = 221;
const SPR_PARACHUTE_BALL_SPR: u16 = 86;
const ACT_STAR_FLOAT: u16 = 1;

/// `ACT_DOOR_*` (actor.h) sit four ids above their `ACT_HEAD_SWITCH_*`.
const ACT_DOOR_BLUE: u16 = 11;
const ACT_DOOR_YELLOW: u16 = 14;
/// The four `ACT_SWITCH_*` ids a foot switch can carry in data5.
const ACT_SWITCH_PLATFORMS: i32 = 59;
const ACT_SWITCH_MYSTERY_WALL: i32 = 61;
const ACT_SWITCH_LIGHTS: i32 = 120;
const ACT_SWITCH_FORCE_FIELD: i32 = 121;

/// `TILE_DOOR_BLOCK` (graphics.h:129) - what a locked door writes over
/// itself to become solid.
const TILE_DOOR_BLOCK: u16 = 0x3dc8;
/// `TILE_MYSTERY_BLOCK_*` (graphics.h) - the four cells a rising mystery
/// wall leaves behind. The port writes one value for all four; they differ
/// only in which edges are drawn.
const TILE_MYSTERY_BLOCK: u16 = 0x3d90;

/// Map tiles these behaviors write (graphics.h:120-130).
const TILE_EMPTY: u16 = 0x0000;
const TILE_STRIPED_PLATFORM: u16 = 0x0050;
const TILE_BLUE_PLATFORM: u16 = 0x3dd0;
/// `ACT_PINK_WORM` (actor.h) - what a broken crate lets out.
const ACT_PINK_WORM: u16 = 124;

/// `ACT_PROJECTILE_W` / `_E` (actor.h:151-152), the two the shipped
/// episodes actually fire.
pub const ACT_PROJECTILE_W: u16 = 109;
pub const ACT_PROJECTILE_E: u16 = 110;
/// The three the spitting turret adds (actor.h:115-117). All five draw the
/// same sprite and share `ActProjectile`; only `DIRP_*` differs.
pub const ACT_PROJECTILE_SW: u16 = 66;
pub const ACT_PROJECTILE_SE: u16 = 67;
pub const ACT_PROJECTILE_S: u16 = 68;
/// `ACT_BABY_GHOST` (actor.h:114) - what an egg hatches into.
const ACT_BABY_GHOST: u16 = 65;

/// `DIRP_*` (def.h:62-66) - the five directions a projectile can take.
const DIRP_WEST: i32 = 0;
const DIRP_SOUTHWEST: i32 = 1;
const DIRP_SOUTH: i32 = 2;
const DIRP_SOUTHEAST: i32 = 3;
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
            // A head switch is furniture that reacts: the pounce switch
            // sets its frame and returns *false*, so the player lands on
            // it rather than bouncing off, and it is never destroyed
            // (game1.c:7454-7462).
            EnemyKind::HeadSwitch => (0, i32::MAX),
            // The boss takes twelve pounces, counted in its own data5
            // rather than here, so the generic path must never kill it.
            EnemyKind::Boss => (7, i32::MAX),
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
    // d5 carries which way the corner turns the rider (game1.c:7617-7636).
    (70, EnemyKind::PipeCorner, [0, 0, 0, 0, DIR8_NORTH_I]), // ACT_PIPE_CORNER_N
    (71, EnemyKind::PipeCorner, [0, 0, 0, 0, DIR8_SOUTH_I]), // ACT_PIPE_CORNER_S
    (72, EnemyKind::PipeCorner, [0, 0, 0, 0, DIR8_WEST_I]),  // ACT_PIPE_CORNER_W
    (73, EnemyKind::PipeCorner, [0, 0, 0, 0, DIR8_EAST_I]),  // ACT_PIPE_CORNER_E
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
    // The turret's three diagonals/downward (game1.c:5792-5798). These are
    // `weighted` in the original, unlike the flat pair.
    (66, EnemyKind::Projectile, [0, 0, 0, 0, DIRP_SOUTHWEST]), // ACT_PROJECTILE_SW
    (67, EnemyKind::Projectile, [0, 0, 0, 0, DIRP_SOUTHEAST]), // ACT_PROJECTILE_SE
    (68, EnemyKind::Projectile, [0, 0, 0, 0, DIRP_SOUTH]),     // ACT_PROJECTILE_S
    (111, EnemyKind::SpittingWallPlant, [0, 0, 0, 0, 1]),  // ACT_SPIT_WALL_PLANT_E
    (112, EnemyKind::SpittingWallPlant, [0, 0, 0, 0, 0]),  // ACT_SPIT_WALL_PLANT_W
    (127, EnemyKind::SentryRobot, [DIR2_WEST, 0, 0, 0, 4]), // ACT_SENTRY_ROBOT
    (74, EnemyKind::BabyGhostEgg, [0, 0, 0, 0, 1]),        // ACT_BABY_GHOST_EGG_PROX
    (75, EnemyKind::BabyGhostEgg, [0, 0, 0, 0, 0]),        // ACT_BABY_GHOST_EGG
    (46, EnemyKind::JumpingBullet, [0, DIR2_WEST, 0, 0, 0]), // ACT_JUMPING_BULLET
    // --- behaviors that write to the map (game1.c:5858, 5954) ---
    (130, EnemyKind::WormCrate, [0, 0, 0, 0, 0]),          // ACT_WORM_CRATE
    (91, EnemyKind::SplittingPlatform, [0, 0, 0, 0, 0]),   // ACT_SPLITTING_PLATFORM
    // --- doors (game1.c:5652): all four colours share the behavior; the
    // colour only matters to the switch that opens them, unported ---
    (11, EnemyKind::Door, [0, 0, 0, 0, 0]),                // ACT_DOOR_BLUE
    (12, EnemyKind::Door, [0, 0, 0, 0, 0]),                // ACT_DOOR_RED
    (13, EnemyKind::Door, [0, 0, 0, 0, 0]),                // ACT_DOOR_GREEN
    (14, EnemyKind::Door, [0, 0, 0, 0, 0]),                // ACT_DOOR_YELLOW
    // --- game1.c:6124, 5673, 6005 ---
    (188, EnemyKind::Rocket, [60, 10, 0, 0, 0]),           // ACT_ROCKET
    (16, EnemyKind::JumpPadRobot, [0, DIR2_WEST, 0, 0, 0]), // ACT_JUMP_PAD_ROBOT
    (145, EnemyKind::IvyPlant, [5, 0, 0, 7, 0]),           // ACT_IVY_PLANT
    // --- head switches (game1.c:5649-5667): data5 names the door they
    // unlock, which sits four ids above the switch ---
    (7, EnemyKind::HeadSwitch, [0, 0, 0, 0, 11]),          // ACT_HEAD_SWITCH_BLUE
    (8, EnemyKind::HeadSwitch, [0, 0, 0, 0, 12]),          // ACT_HEAD_SWITCH_RED
    (9, EnemyKind::HeadSwitch, [0, 0, 0, 0, 13]),          // ACT_HEAD_SWITCH_GREEN
    (10, EnemyKind::HeadSwitch, [0, 0, 0, 0, 14]),         // ACT_HEAD_SWITCH_YELLOW
    // --- foot switches (game1.c:5778-5927): the four sprites where
    // ActFootSwitch is a real behavior rather than a no-op ---
    (59, EnemyKind::FootSwitch, [0, 0, 0, 0, ACT_SWITCH_PLATFORMS]),
    (61, EnemyKind::FootSwitch, [0, 0, 0, 0, ACT_SWITCH_MYSTERY_WALL]),
    (120, EnemyKind::FootSwitch, [0, 0, 0, 0, ACT_SWITCH_LIGHTS]),
    (121, EnemyKind::FootSwitch, [0, 0, 0, 0, ACT_SWITCH_FORCE_FIELD]),
    (62, EnemyKind::MysteryWall, [0, 0, 0, 0, 0]),         // ACT_MYSTERY_WALL
    // --- force fields (game1.c:5930-5933): data5 picks the axis ---
    (122, EnemyKind::ForceField, [0, 0, 0, 0, 0]),         // ACT_FORCE_FIELD_VERT
    (123, EnemyKind::ForceField, [0, 0, 0, 0, 1]),         // ACT_FORCE_FIELD_HORIZ
    (126, EnemyKind::PusherRobot, [DIR2_WEST, 0, 0, 0, 4]), // ACT_PUSHER_ROBOT
    (64, EnemyKind::Monument, [0, 0, 0, 0, 0]),            // ACT_MONUMENT
    (143, EnemyKind::Satellite, [0, 0, 0, 0, 0]),          // ACT_SATELLITE
    (152, EnemyKind::TulipLauncher, [0, 30, 0, 0, 0]),     // ACT_TULIP_LAUNCHER
    // Invisible trigger lines: the episode-end cliffhangers and the
    // horizontal exit line (game1.c:6061, 6361).
    (164, EnemyKind::TriggerLine, [0, 0, 0, 0, 0]),        // ACT_EP1_END_1
    (165, EnemyKind::TriggerLine, [0, 0, 0, 0, 0]),        // ACT_EP1_END_2
    (166, EnemyKind::TriggerLine, [0, 0, 0, 0, 0]),        // ACT_EP1_END_3
    (265, EnemyKind::TriggerLine, [1, 0, 0, 0, 0]),        // ACT_EP2_END_LINE
    (114, EnemyKind::Scooter, [0, 0, 0, 0, 0]),            // ACT_SCOOTER
    (162, EnemyKind::BearTrap, [0, 0, 0, 0, 0]),           // ACT_BEAR_TRAP
    (90, EnemyKind::BeamRobot, [0, 0, 0, 0, 0]),           // ACT_BEAM_ROBOT
    (107, EnemyKind::Transporter, [0, 0, 0, 0, 1]),        // ACT_TRANSPORTER_1
    (108, EnemyKind::Transporter, [0, 0, 0, 0, 2]),        // ACT_TRANSPORTER_2
    (102, EnemyKind::Boss, [0, 0, 0, 0, 0]),               // ACT_BOSS
    (221, EnemyKind::FrozenDN, [0, 0, 0, 0, 0]),           // ACT_FROZEN_DN
    // Speech bubbles. d5 marks the one that pays out (game1.c:5299).
    (235, EnemyKind::SpeechBubble, [0, 0, 0, 0, 0]),       // ACT_SPEECH_OUCH
    (244, EnemyKind::SpeechBubble, [0, 0, 0, 0, 0]),       // ACT_SPEECH_WHOA
    (245, EnemyKind::SpeechBubble, [0, 0, 0, 0, 0]),       // ACT_SPEECH_UMPH
    (246, EnemyKind::SpeechBubble, [0, 0, 0, 0, 1]),       // ACT_SPEECH_WOW_50K
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

/// The level-wide flags the switches throw (game1.c:1935-1957). Every one
/// starts *on* at level load (game1.c:10305, 10447, 10453); constructing
/// the switch that governs it is what turns it off, so a level with a
/// platform switch has its platforms dead until the switch is thrown
/// (game1.c:5779, 5923).
#[derive(Resource)]
pub struct SwitchState {
    pub platforms_active: bool,
    pub lights_active: bool,
    pub force_fields_active: bool,
    /// Counts down while the mystery wall rises; non-zero wakes it.
    pub mystery_wall_time: i32,
    /// `ACT_DOOR_*` ids whose head switch has been pounced.
    pub doors_opened: Vec<u16>,
}

impl SwitchState {
    /// Resets for a newly loaded level. Every flag starts on, and then the
    /// presence of a switch actor turns its flag off - which is what the
    /// original does at construction time (game1.c:5779, 5786, 5923).
    pub fn reset_for_level(&mut self, level: &LevelJson) {
        *self = SwitchState::default();
        for a in &level.actors {
            match a.map_type as i32 - 31 {
                ACT_SWITCH_PLATFORMS => self.platforms_active = false,
                ACT_SWITCH_MYSTERY_WALL => self.mystery_wall_time = 0,
                ACT_SWITCH_LIGHTS => self.lights_active = false,
                _ => {}
            }
        }
    }
}

impl Default for SwitchState {
    fn default() -> Self {
        SwitchState {
            platforms_active: true,
            lights_active: true,
            force_fields_active: true,
            mystery_wall_time: 0,
            doors_opened: Vec::new(),
        }
    }
}

/// The `saw*Bubble` one-shots (game1.c:10465-10480). Each speech bubble
/// fires the first time the player meets its thing and never again, and
/// the flags reset per *episode*, not per level - so saying "whoa" at a
/// transporter in level 1 keeps you quiet at every later one.
#[derive(Resource, Default)]
pub struct SeenBubbles {
    pub pusher_robot: bool,
    pub bear_trap: bool,
    pub mystery_wall: bool,
    pub boss: bool,
    /// The transporter and the pipes raise theirs from systems rather than
    /// behaviours, so they carry their own one-shots there.
    pub transporter: bool,
    pub pipe: bool,
}

/// `activeTransporter` / `transporterTimeLeft` (game1.c:4085-4122). A
/// transporter is inherently a pair, so the state lives outside the actors.
#[derive(Resource, Default)]
pub struct TransporterState {
    /// Which pad the player stepped into (`data5`), or 0 for none.
    pub active: i32,
    /// Counts down from 15 while the effect plays.
    pub time_left: i32,
}

/// A live actor running one of the ported behaviors.
#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    /// The `ACT_*` id this was built from. Needed by behaviors that have
    /// to recognise their own type at runtime - a door asking whether its
    /// colour has been unlocked.
    pub act_id: u16,
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
    /// `SetMapTile` writes raised this tick, as (x, y, raw tile). Queued
    /// for the same reason as `spawns`: the behaviors stay pure.
    pub tile_writes: Vec<(i32, i32, u16)>,
    /// A shove to apply to the player this tick, as
    /// (dx, dy, max_time, speed) - see `Player::set_push`.
    pub push_player: Option<(i32, i32, u32, u32)>,
    /// Ticks to hold the player still, raised by the bear trap.
    pub hold_player: u32,
    /// Set by the boss when its death sequence completes.
    pub won_level: bool,
    /// `NewDecoration` requests raised this tick, as
    /// (SPR_* id, frame count, x, y, DIR8, repeats). Queued for the same
    /// reason as `spawns`: the behaviours stay pure.
    pub decorations: Vec<(u16, usize, i32, i32, usize, u32)>,
    /// `StartSound` requests raised this tick. Queued like the rest so the
    /// behaviours stay pure; the priority gate in `sfx.rs` decides which
    /// of them is actually heard.
    pub sounds: Vec<u16>,
    /// `NewShard` requests raised this tick, as (SPR_* id, frame, x, y).
    pub shards: Vec<(u16, usize, i32, i32)>,
    /// Score this actor has earned and not yet handed over.
    pub score_award: u32,
    /// A speech bubble to raise, if its one-shot has not already fired.
    pub bubble: Option<u16>,
    /// `NewExplosion` requests raised this tick.
    pub explosions: Vec<(i32, i32)>,
    /// Set by the rocket while it is lifting a rider.
    pub carry_player: bool,
    /// Score pop-ups to raise, as (x, y).
    pub score_effects: Vec<(i32, i32)>,
}

impl Enemy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: EnemyKind,
        act_id: u16,
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
            act_id,
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
            tile_writes: Vec::new(),
            push_player: None,
            hold_player: 0,
            won_level: false,
            decorations: Vec::new(),
            sounds: Vec::new(),
            shards: Vec::new(),
            score_award: 0,
            bubble: None,
            explosions: Vec::new(),
            carry_player: false,
            score_effects: Vec::new(),
        }
    }

    /// A bare actor for unit tests: no sprite handles, no app, no window.
    /// The tick functions are pure over `Enemy` + the level, which is what
    /// makes the behaviors testable without standing up Bevy at all.
    #[cfg(test)]
    pub fn default_for_test(kind: EnemyKind) -> Self {
        Enemy {
            kind,
            act_id: 0,
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
            tile_writes: Vec::new(),
            push_player: None,
            hold_player: 0,
            won_level: false,
            decorations: Vec::new(),
            sounds: Vec::new(),
            shards: Vec::new(),
            score_award: 0,
            bubble: None,
            explosions: Vec::new(),
            carry_player: false,
            score_effects: Vec::new(),
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
/// Smoke rises off the flame as it peaks.
fn tick_flame_pulse(e: &mut Enemy) {
    if e.frame == 8 {
        e.decorations
            .push((SPR_SMOKE, 6, e.x, e.y - 2, crate::effects::DIR8_NORTH, 1));
    }
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
/// It fires on frames 2, 5, 8, 11 and 14 - west, south-west, south,
/// south-east and east - each from an offset that clears its own body.
fn tick_spitting_turret(e: &mut Enemy, player: &Player) {
    e.d2 -= 1;
    if e.d2 == 0 {
        e.d1 += 1;
        e.d2 = 3;
        if e.d1 != 3 {
            e.frame += 1;
            match e.frame {
                2 => e.spawns.push((ACT_PROJECTILE_W, e.x - 1, e.y - 1)),
                5 => e.spawns.push((ACT_PROJECTILE_SW, e.x - 1, e.y + 1)),
                8 => e.spawns.push((ACT_PROJECTILE_S, e.x + 1, e.y + 1)),
                11 => e.spawns.push((ACT_PROJECTILE_SE, e.x + 5, e.y + 1)),
                14 => e.spawns.push((ACT_PROJECTILE_E, e.x + 5, e.y - 1)),
                _ => {}
            }
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
/// The emitter is never drawn; the smoke is the whole actor. `data5`
/// selects the small plume over the large one.
///
/// The original never draws the emitter either, and
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
            e.sounds.push(crate::sfx::snd::DRIP);
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
fn tick_arrow_piston(e: &mut Enemy) {
    if e.d1 < 31 {
        e.d1 += 1;
    } else {
        e.d1 = 0;
    }

    // The two windows overlap in the source's `else if` chain: >28 wins
    // over >25, so 29..31 retract and 26..28 extend.
    // The two ticks it starts moving each way (game1.c:2062).
    if e.d1 == 29 || e.d1 == 26 {
        e.sounds.push(crate::sfx::snd::SPIKES_MOVE);
    }

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
fn tick_fireball(
    e: &mut Enemy,
    level: &LevelJson,
    data: &GameData,
    scroll: &crate::camera::Scroll,
) {
    if e.d1 == 29 {
        e.sounds.push(crate::sfx::snd::FIREBALL_LAUNCH);
    }
    if e.d1 < 30 {
        e.d1 += 1;
    } else {
        let dir = if e.d5 == DIR2_WEST { Dir4::West } else { Dir4::East };
        e.x += if e.d5 == DIR2_WEST { -1 } else { 1 };
        let blocked = test_sprite_move(dir, e.x, e.y, e.width_tiles, e.height_tiles, level, data)
            != MoveResult::Free;
        if blocked {
            // A puff where it struck, before it snaps back to its launcher
            // (game1.c:2103, 2114).
            let sx = if e.d5 == DIRP_WEST { e.x + 1 } else { e.x - 2 };
            e.decorations
                .push((SPR_SMOKE, 6, sx, e.y, crate::effects::DIR8_NORTH, 1));
            e.sounds.push(crate::sfx::snd::BIG_OBJECT_HIT);
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
fn tick_vertical_mover(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    e.frame = usize::from(e.frame == 0);
    // The original plays this every tick the blade is on screen; the
    // priority gate collapses the repeats (game1.c:2264).
    e.sounds.push(crate::sfx::snd::SAW_BLADE_MOVE);

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
        e.sounds.push(crate::sfx::snd::PLANT_MOUTH_OPEN);
    }
    if e.frame == 2 {
        e.x += 1;
    }
}

/// `ActTwoTonsCrusher` (game1.c:2496-2570). Drops on a twenty-tick timer
/// regardless of where the player is, accelerating 1/2/4 rows on the way
/// down and decelerating the same way back up.
///
/// NOT PORTED: the separate base sprite the original draws three rows
/// below itself.
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
            e.sounds.push(crate::sfx::snd::OBJECT_HIT);
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
        let mut landed = blocked(e, level, data);
        if landed {
            e.d1 = 2;
            e.y -= 1;
        } else {
            // It falls a second row in the same tick, testing again.
            e.y += 1;
            landed = blocked(e, level, data);
            if landed {
                e.d1 = 2;
                e.y -= 1;
            }
        }
        if landed {
            e.sounds.push(crate::sfx::snd::OBJECT_HIT);
            // Dust kicked out both ways on impact (game1.c:2620-2621).
            e.decorations
                .push((SPR_SMOKE, 6, e.x + 1, e.y, crate::effects::DIR8_NORTHEAST, 1));
            e.decorations
                .push((SPR_SMOKE, 6, e.x, e.y, crate::effects::DIR8_NORTHWEST, 1));
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
fn tick_projectile(e: &mut Enemy, scroll: &crate::camera::Scroll) {
    if !is_visible_at(e.x, e.y, e.width_tiles, e.height_tiles, scroll.x, scroll.y) {
        e.dead = true;
        return;
    }
    if e.d1 == 0 {
        e.d1 = 1;
        e.sounds.push(crate::sfx::snd::PROJECTILE_LAUNCH);
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
fn tick_sentry_robot(
    e: &mut Enemy,
    player: &Player,
    switches: &SwitchState,
    level: &LevelJson,
    data: &GameData,
) {
    e.d3 = i32::from(e.d3 == 0);
    if e.d3 != 0 {
        return;
    }

    // One outcome in fifty, and only while the lights are on
    // (game1.c:4575).
    if switches.lights_active && e.next_rand(50) > 48 && e.d4 == 0 {
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
        e.sounds.push(crate::sfx::snd::BGHOST_EGG_CRACK);
    }

    if e.d2 > 1 {
        e.d2 -= 1;
    } else if e.d2 == 1 {
        e.dead = true;
        e.spawns.push((ACT_BABY_GHOST, e.x, e.y));
        e.sounds.push(crate::sfx::snd::BGHOST_EGG_HATCH);
        // Four pieces of shell, thrown outward (game1.c:3093-3096).
        for (i, (dx, dy, dir)) in [
            (0, -1, crate::effects::DIR8_NORTHWEST),
            (1, -1, crate::effects::DIR8_NORTHEAST),
            (0, 0, crate::effects::DIR8_EAST),
            (1, 0, crate::effects::DIR8_WEST),
        ]
        .into_iter()
        .enumerate()
        {
            e.decorations
                .push((SPR_BGHOST_EGG_SHARDS[i], 2, e.x + dx, e.y + dy, dir, 5));
        }
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

/// `ActWormCrate` (game1.c:4680-4726). Lays a four-tile platform across
/// its own top, falls until something stops it, and breaks open when a
/// blast reaches it - letting out a pink worm.
///
/// NOT PORTED: the explosion delay
/// `data5` is seeded per-crate in the original from `GameRand`; here it
/// comes from the actor's own PRNG on first tick, for the same effect.
fn tick_worm_crate(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    if e.d4 == 0 {
        // First tick: stamp the platform the player can stand on.
        for i in 0..4 {
            e.tile_writes.push((e.x + i, e.y - 2, TILE_STRIPED_PLATFORM));
        }
        e.d4 = 1;
        return;
    }

    let free_below = test_sprite_move(
        Dir4::South,
        e.x,
        e.y + 1,
        e.width_tiles,
        e.height_tiles,
        level,
        data,
    ) == MoveResult::Free;

    if free_below {
        // Falling: the platform travels with it.
        for i in 0..4 {
            e.tile_writes.push((e.x + i, e.y - 2, TILE_EMPTY));
        }
        e.y += 1;
        let landed = test_sprite_move(
            Dir4::South,
            e.x,
            e.y + 1,
            e.width_tiles,
            e.height_tiles,
            level,
            data,
        ) != MoveResult::Free;
        if landed {
            for i in 0..4 {
                e.tile_writes.push((e.x + i, e.y - 2, TILE_STRIPED_PLATFORM));
            }
        }
    }
}

/// Breaks a worm crate open. Called by the blast code rather than by the
/// crate's own tick, which is where the original tests `IsNearExplosion`.
pub fn burst_worm_crate(e: &mut Enemy) {
    if e.dead {
        return;
    }
    e.dead = true;
    for i in 0..4 {
        e.tile_writes.push((e.x + i, e.y - 2, TILE_EMPTY));
    }
    e.spawns.push((ACT_PINK_WORM, e.x, e.y));
    e.sounds.push(crate::sfx::snd::DESTROY_SOLID);
    // Seven pieces (game1.c:4712-4718); two share an origin, as written.
    for (i, (dx, dy)) in [
        (-1, 3), (0, -1), (1, 0), (0, 0), (3, 2), (0, 0), (5, 5),
    ]
    .into_iter()
    .enumerate()
    {
        e.shards.push((SPR_WORM_CRATE_SHARDS, i, e.x + dx, e.y + dy));
    }
}

/// `ActSplittingPlatform` (game1.c:3354-3410). Holds a four-tile platform
/// until the player stands on it, then pulls apart and drops them, and
/// finally closes again.
///
/// The original keeps its half-speed counter in the `westfree` field,
/// which it borrows as an extra data word - noted because it looks like a
/// collision flag and is not one.
fn tick_splitting_platform(e: &mut Enemy, player: &Player) {
    e.d3 += 1; // the original's borrowed `westfree`

    if e.d1 == 0 {
        e.d1 = 1;
        for i in 0..4 {
            e.tile_writes.push((e.x + i, e.y - 1, TILE_BLUE_PLATFORM));
        }
    } else if e.d1 == 1 && e.y - 2 == player.y {
        // Either the player's left or right edge over the platform's span.
        let over = (e.x <= player.x && e.x + 3 >= player.x)
            || (e.x <= player.x + 2 && e.x + 3 >= player.x + 2);
        if over {
            e.d1 = 2;
            e.d2 = 0;
        }
    } else if e.d1 == 2 {
        if e.d3 % 2 != 0 {
            e.d2 += 1;
        }
        if e.d2 == 5 {
            // The halves have pulled apart far enough to drop through.
            for i in 0..4 {
                e.tile_writes.push((e.x + i, e.y - 1, TILE_EMPTY));
            }
        }
        if e.d2 == 7 {
            e.d1 = 3;
            e.d2 = 0;
        }
    }

    if e.d1 == 3 {
        if e.d3 % 2 != 0 {
            e.d2 += 1;
        }
        if e.d2 == 3 {
            // Closed again; the next tick re-lays the platform.
            e.d1 = 0;
        }
    }
}

/// `ActDoor` (game1.c:2174-2188). Writes five rows of solid door tile up
/// its own column on the first tick and then does nothing - the door *is*
/// the map change, which is why walking into one is stopped by ordinary
/// tile collision rather than by any actor test.
///
fn tick_door(e: &mut Enemy, switches: &SwitchState, level: &LevelJson) {
    if !e.west_free {
        // The original borrows `westfree` as its "already stamped" flag
        // (game1.c:2180) and saves the five tiles it is about to cover in
        // data1..data5, so the switch can put them back.
        e.west_free = true;
        let saved = [&mut e.d1, &mut e.d2, &mut e.d3, &mut e.d4, &mut e.d5];
        let (x, y0) = (e.x + 1, e.y);
        for (row, slot) in saved.into_iter().enumerate() {
            let y = y0 - row as i32;
            *slot = if x >= 0 && y >= 0 {
                level.tile_at(x as usize, y as usize) as i32
            } else {
                0
            };
        }
        for row in 0..5 {
            e.tile_writes.push((x, y0 - row, TILE_DOOR_BLOCK));
        }
        return;
    }

    if !switches.doors_opened.contains(&e.act_id) {
        return;
    }
    // Unlocked: put back what the door covered and remove it.
    let saved = [e.d1, e.d2, e.d3, e.d4, e.d5];
    for (row, raw) in saved.into_iter().enumerate() {
        e.tile_writes.push((e.x + 1, e.y - row as i32, raw as u16));
    }
    e.dead = true;
}

/// `ActRocket` (game1.c:5179-5262). Sits on a sixty-tick fuse, then climbs
/// until it hits a ceiling and destroys itself. Riding it is the point:
/// the original shoves the player up with it, which needs player state the
/// port does not have, so this flies without a passenger.
///
/// Riding it is the point: a player standing on the nose is pinned there
/// and carried up with it (game1.c:5237-5245).
///
/// NOT PORTED: the sounds, and the two explosions it leaves.
fn tick_rocket(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    if e.d1 != 0 {
        e.d1 -= 1;
        // Exhaust on the pad, alternating sides (game1.c:5185-5191).
        if e.d1 < 30 {
            let (dx, dir) = if e.d1 % 2 != 0 {
                (-1, crate::effects::DIR8_NORTHWEST)
            } else {
                (1, crate::effects::DIR8_NORTHEAST)
            };
            e.decorations
                .push((SPR_SMOKE, 6, e.x + dx, e.y + 1, dir, 1));
        }
        return;
    }

    if e.d2 != 0 {
        if e.d2 > 1 {
            e.d2 -= 1;
        }
        // The burn under it as it climbs (game1.c:5218-5220).
        if e.d2 > 4 && e.d2 % 2 != 0 {
            e.decorations
                .push((SPR_SMOKE, 6, e.x, e.y + 2, crate::effects::DIR8_SOUTH, 1));
        }
        // Below ten it climbs a row a tick, and below five a second row in
        // the same tick - which is the acceleration off the pad.
        for _ in 0..2 {
            if e.d2 >= 10 {
                break;
            }
            if e.d2 < 10 {
                if test_sprite_move(
                    Dir4::North, e.x, e.y - 1, e.width_tiles, e.height_tiles, level, data,
                ) == MoveResult::Free
                {
                    e.y -= 1;
                } else {
                    e.d5 = 1;
                }
            }
            if e.d2 >= 5 {
                break;
            }
        }
        e.d4 = i32::from(e.d4 == 0);
    }

    // A rider on the nose is held on and lifted (game1.c:5237-5245).
    if e.d2 != 0 && e.x == player.x && e.y - 7 <= player.y && e.y - 4 >= player.y {
        e.carry_player = true;
    }

    if e.d5 != 0 {
        e.dead = true;
        e.sounds.push(crate::sfx::snd::EXPLOSION);
        for (i, dx) in [0, 1, 2].into_iter().enumerate() {
            e.shards.push((SPR_ROCKET, i + 1, e.x + dx, e.y));
        }
    }
}

/// `ActJumpPadRobot` (game1.c:2194-2222). Paces, and holds a crouched
/// frame for `data1` ticks after it has been bounced on.
fn tick_jump_pad_robot(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    if e.d1 > 0 {
        e.frame = 2;
        e.d1 -= 1;
        e.sounds.push(crate::sfx::snd::JUMP_PAD_ROBOT);
        return;
    }

    e.frame = usize::from(e.frame == 0);
    if e.d2 != DIR2_WEST {
        e.x += 1;
        adjust_actor_move(e, Dir4::East, level, data);
        if !e.east_free {
            e.d2 = DIR2_WEST;
        }
    } else {
        e.x -= 1;
        adjust_actor_move(e, Dir4::West, level, data);
        if !e.west_free {
            e.d2 = DIR2_EAST;
        }
    }
}

/// `ActIvyPlant` (game1.c:4774-4812). Idles for `data1` ticks, then climbs
/// its seven rows one at a time, animating as it goes. A blast sends it
/// back down.
///
fn tick_ivy_plant(e: &mut Enemy) {
    if e.d2 != 0 {
        // Falling back down after a blast.
        e.y += 1;
        e.d4 += 1;
        if e.d4 == 7 {
            e.d2 = 0;
            e.d3 = 0;
            e.d1 = 12;
        }
        return;
    }

    if e.d3 < e.d1 {
        e.d3 += 1;
        return;
    }

    e.d5 = i32::from(e.d5 == 0);
    e.frame += 1;
    if e.frame == 4 {
        e.frame = 0;
    }

    if e.d4 != 0 {
        if e.d4 == 7 {
            e.sounds.push(crate::sfx::snd::IVY_PLANT_RISE);
        }
        e.d4 -= 1;
        e.y -= 1;
    }
}

/// `ActHeadSwitch` (game1.c:2160-2170) and `UpdateDoors` (game1.c:2143).
/// Pouncing the switch drives `frame` to 1; from there `data1` climbs to 3
/// and the doors of its colour open on the way.
fn tick_head_switch(e: &mut Enemy, switches: &mut SwitchState) {
    if e.frame != 1 {
        return;
    }
    if e.d1 < 3 {
        e.d1 += 1;
    }
    // data5 names the door colour. `UpdateDoors` restores the door's saved
    // tiles at step 1 and kills the door itself at step 2; recording the
    // colour once lets each door do both from its own tick.
    let door = e.d5 as u16;
    if (ACT_DOOR_BLUE..=ACT_DOOR_YELLOW).contains(&door) && !switches.doors_opened.contains(&door) {
        switches.doors_opened.push(door);
    }
}

/// `ActFootSwitch` (game1.c:1893-1977) for the knob sprite, where it is a
/// real behavior rather than the no-op it is for spikes and food.
///
/// A bomb blast drives the knob down one step; the fourth press throws
/// whatever `data5` names. `press` is called by the blast code.
///
/// NOT PORTED: the switch tiles it stamps into the map as it descends.
fn tick_foot_switch(e: &mut Enemy, switches: &mut SwitchState) {
    if e.d4 == 0 {
        return;
    }
    e.d4 = 0;
    e.y += 1;

    if e.d1 != 4 {
        e.sounds.push(crate::sfx::snd::FOOT_SWITCH_MOVE);
        return;
    }
    e.sounds.push(crate::sfx::snd::FOOT_SWITCH_ON);
    match e.d5 {
        ACT_SWITCH_PLATFORMS => switches.platforms_active = true,
        ACT_SWITCH_MYSTERY_WALL => {
            switches.mystery_wall_time = 4;
            e.bubble = Some(ACT_SPEECH_WHOA);
        }
        ACT_SWITCH_LIGHTS => switches.lights_active = true,
        ACT_SWITCH_FORCE_FIELD => switches.force_fields_active = false,
        _ => {}
    }
}

/// Drives a foot switch down one step. Called by the blast code, which is
/// where the original tests `IsNearExplosion` (game1.c:1966-1977).
pub fn press_foot_switch(e: &mut Enemy) {
    if e.d1 >= 4 || e.d4 != 0 {
        return;
    }
    e.d1 += 1;
    e.d4 = 1;
}

/// `ActMysteryWall` (game1.c:3820-3854). Sleeps until its switch is
/// thrown, then climbs until it meets a ceiling and becomes part of the
/// map.
///
fn tick_mystery_wall(
    e: &mut Enemy,
    switches: &mut SwitchState,
    level: &LevelJson,
    data: &GameData,
) {
    if switches.mystery_wall_time != 0 {
        e.d1 = 1;
        e.force_active = true;
    }
    if e.d1 == 0 {
        return;
    }

    if e.d1 % 2 != 0 {
        for (dx, dy) in [(0, -1), (1, -1), (0, 0), (1, 0)] {
            e.tile_writes.push((e.x + dx, e.y + dy, TILE_MYSTERY_BLOCK));
        }
    }

    if test_sprite_move(
        Dir4::North, e.x, e.y - 1, e.width_tiles, e.height_tiles, level, data,
    ) != MoveResult::Free
    {
        if e.d1 % 2 == 0 {
            for dx in 0..2 {
                e.tile_writes.push((e.x + dx, e.y - 1, TILE_MYSTERY_BLOCK));
            }
        }
        e.dead = true;
    } else {
        if e.d1 % 2 == 0 {
            e.decorations.push((
                SPR_SPARKLE_SHORT,
                4,
                e.x - 1,
                e.y - 1,
                crate::effects::DIR8_NONE,
                1,
            ));
        }
        e.d1 += 1;
        e.y -= 1;
    }
}

/// `ActForceField` (game1.c:4346-4392). The actor itself is never drawn;
/// each tick it walks out from its own cell until a wall blocks it, and
/// the beam is that run of cells. `data1` ends the tick holding its length,
/// which is what the damage and drawing pass reads.
///
/// Switching the fields off kills the actor outright (game1.c:4360), so a
/// thrown switch removes them permanently rather than merely hiding them.
fn tick_force_field(
    e: &mut Enemy,
    switches: &SwitchState,
    level: &LevelJson,
    data: &GameData,
) {
    e.d4 += 1;
    if e.d4 == 3 {
        e.d4 = 0;
    }

    if !switches.force_fields_active {
        e.dead = true;
        e.d1 = 0;
        return;
    }

    // Walk until the wall. The original's loop tests the *player* first
    // and stops there too, but stopping the beam at the player would let
    // them shield whatever is behind them; the damage pass handles that
    // separately, so here it only needs the wall.
    let vertical = e.d5 == 0;
    let mut len = 0;
    while len < FORCE_FIELD_MAX {
        let (x, y) = if vertical {
            (e.x, e.y - len)
        } else {
            (e.x + len, e.y)
        };
        let flag = if vertical {
            TILE_ATTR_BLOCK_NORTH
        } else {
            TILE_ATTR_BLOCK_EAST
        };
        if attr_at(level, data, x, y) & flag != 0 {
            break;
        }
        len += 1;
    }
    e.d1 = len;
}

/// A beam cannot run further than the map is tall; the cap only stops a
/// runaway loop if a level ever leaves one unterminated.
const FORCE_FIELD_MAX: i32 = 64;

/// The cells a beam currently occupies, as a rectangle in tile space:
/// (x, top row, width, height). Serves the force field and the beam robot,
/// which are the two actors that are partly a line rather than a body.
pub fn beam_rect(e: &Enemy) -> Option<(i32, i32, i32, i32)> {
    if e.dead {
        return None;
    }
    match e.kind {
        EnemyKind::ForceField if e.d1 > 0 => {
            if e.d5 == 0 {
                Some((e.x, e.y - e.d1 + 1, e.width_tiles, e.d1))
            } else {
                Some((e.x, e.y - e.height_tiles + 1, e.d1, e.height_tiles))
            }
        }
        // The robot's beam stands on its head, starting two rows up
        // (game1.c:3327).
        EnemyKind::BeamRobot if e.d2 > 0 => Some((e.x + 1, e.y - e.d2 - 1, 1, e.d2)),
        _ => None,
    }
}

fn has_beam(e: &Enemy) -> bool {
    matches!(e.kind, EnemyKind::ForceField | EnemyKind::BeamRobot)
}

/// `ActPusherRobot` (game1.c:4488-4560). Paces at half speed until the
/// player is level with it and exactly three columns ahead, then shoves
/// them for five ticks at two cells a tick and waits three ticks before it
/// can shove again.
///
/// NOT PORTED: the translucent draw mode it uses between shoves.
fn tick_pusher_robot(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    if e.d2 != 0 {
        // Holding the shove pose.
        e.d2 -= 1;
        return;
    }
    if e.d4 != 0 {
        e.d4 -= 1;
    }

    e.d3 = i32::from(e.d3 == 0);

    let west = e.d1 == DIR2_WEST;
    // The reach is asymmetric because the robot's origin is its left edge:
    // three columns west of it, four east (game1.c:4505, 4530).
    let in_reach = e.y == player.y
        && e.d4 == 0
        && if west { e.x - 3 == player.x } else { e.x + 4 == player.x };

    if in_reach {
        e.frame = if west { 2 } else { 5 };
        e.d2 = 8;
        e.d4 = 3;
        // Five ticks at two cells each, blockable so a wall stops it, and
        // not abortable - jumping does not get you out of it.
        e.push_player = Some((if west { -1 } else { 1 }, 0, 5, 2));
        e.sounds.push(crate::sfx::snd::PUSH_PLAYER);
        e.bubble = Some(ACT_SPEECH_UMPH);
        return;
    }

    if e.d3 == 0 {
        return;
    }
    if west {
        e.x -= 1;
        adjust_actor_move(e, Dir4::West, level, data);
        if !e.west_free {
            e.d1 = DIR2_EAST;
            e.frame = (e.x % 2) as usize + 3;
        } else {
            e.frame = usize::from(e.frame == 0);
        }
    } else {
        e.x += 1;
        adjust_actor_move(e, Dir4::East, level, data);
        if !e.east_free {
            e.d1 = DIR2_WEST;
            e.frame = usize::from(e.frame == 0);
        } else {
            e.frame = (e.x % 2) as usize + 3;
        }
    }
}

/// `ActMonument` (game1.c:5338-5390). Stands as nine tiles of solid until
/// *three* blasts bring it down, paying out 25600 - the largest single
/// score in the game. Three, not two: the frame climbs 0 -> 1 -> 2 -> 3
/// and only the step onto 3 topples it (game1.c:5381-5387).
///

fn tick_monument(e: &mut Enemy) {
    if e.d2 != 0 {
        e.dead = true;
        for i in 0..9 {
            e.tile_writes.push((e.x + 1, e.y - i, TILE_EMPTY));
        }
        e.sounds.push(crate::sfx::snd::DESTROY_SOLID);
        e.score_award += 25_600;
        // The pair of 12800 pop-ups either side of it (game1.c:5362-5363).
        e.score_effects.push((e.x - 2, e.y - 9));
        e.score_effects.push((e.x + 2, e.y - 9));
        // Six pieces of the pillar (game1.c:5346-5351).
        for (dx, dy) in [(0, -8), (0, -7), (0, -6), (0, 0), (1, 0), (2, 0)] {
            e.shards.push((SPR_MONUMENT, 3, e.x + dx, e.y + dy));
        }
        // The dust cloud it collapses into (game1.c:5352-5355).
        for (dx, dy, dir) in [
            (0, 0, crate::effects::DIR8_NORTH),
            (0, 0, crate::effects::DIR8_NORTHEAST),
            (0, 0, crate::effects::DIR8_NORTHWEST),
            (0, -4, crate::effects::DIR8_NORTH),
        ] {
            e.decorations
                .push((SPR_SMOKE, 6, e.x + dx, e.y + dy, dir, 2));
        }
        return;
    }
    if !e.west_free {
        e.west_free = true;
        for i in 0..9 {
            e.tile_writes.push((e.x + 1, e.y - i, TILE_SWITCH_BLOCK));
        }
    }
    if e.d1 != 0 {
        e.d1 -= 1;
    }
}

/// A blast landing on a monument. Returns whether this one toppled it; the
/// earlier ones only make it flash (game1.c:5378-5389).
pub fn blast_monument(e: &mut Enemy) -> bool {
    if e.d1 != 0 || e.d2 != 0 {
        return false;
    }
    e.d1 = 10;
    e.frame += 1;
    if e.frame == 3 {
        e.frame = 2;
        e.d2 = 1;
        return true; // this one brings it down
    }
    false
}

/// `ActSatellite` (game1.c:4728-4770). Two blasts destroy it and it drops
/// a hamburger.
fn tick_satellite(e: &mut Enemy) {
    if e.d2 != 0 {
        e.d2 -= 1;
    }
}

/// A blast landing on a tulip launcher. The second one finishes it.
pub fn blast_tulip_launcher(e: &mut Enemy) {
    if e.d3 != 0 {
        return;
    }
    e.d3 = 15;
    e.d5 += 1;
    if e.d5 != 2 {
        return;
    }
    e.dead = true;
    // The parachute ball it was holding, in pieces (game1.c:5422-5427).
    for frame in [0, 2, 4, 9, 3] {
        e.shards.push((SPR_PARACHUTE_BALL_SPR, frame, e.x + 2, e.y - 5));
    }
    e.sounds.push(crate::sfx::snd::DESTROY_SOLID);
}

/// A blast landing on a satellite.
pub fn blast_satellite(e: &mut Enemy) {
    if e.d2 != 0 {
        return;
    }
    if e.d1 == 0 {
        e.d1 = 1;
        e.d2 = 15;
        return;
    }
    e.dead = true;
    e.sounds.push(crate::sfx::snd::DESTROY_SATELLITE);
    // Eight pieces, each its own frame (game1.c:4756-4763).
    for (i, (dx, dy)) in [
        (0, -2), (1, -2), (7, 2), (3, -2), (-1, -8), (2, 3), (6, -2), (-4, 1),
    ]
    .into_iter()
    .enumerate()
    {
        e.shards.push((SPR_SATELLITE_SHARDS, i, e.x + dx, e.y + dy));
    }
    // A ring of smoke in every compass direction (game1.c:4750-4752).
    for dir in 1..9 {
        e.decorations
            .push((SPR_SMOKE, 6, e.x + 3, e.y - 3, dir, 3));
    }
    e.spawns.push((ACT_HAMBURGER, e.x + 4, e.y));
}

/// `ActTulipLauncher` (game1.c:5395-5450). Throws a parachute ball, then
/// sits for a hundred ticks before winding up again.
///
/// Two blasts destroy it, throwing the ball it was about to launch
/// (game1.c:5418-5430).
fn tick_tulip_launcher(e: &mut Enemy) {
    /// game1.c:5397 - the wind-up, as frame numbers.
    const LAUNCH_FRAMES: [usize; 5] = [0, 2, 1, 0, 1];

    if e.d2 != 0 {
        e.frame = 1;
        e.d2 -= 1;
        return;
    }

    e.frame = LAUNCH_FRAMES[e.d1 as usize % LAUNCH_FRAMES.len()];
    e.d1 += 1;
    if e.d1 == 2 {
        e.spawns.push((ACT_PARACHUTE_BALL, e.x + 2, e.y - 5));
        e.sounds.push(crate::sfx::snd::TULIP_LAUNCH);
    }
    if e.d1 == 5 {
        e.d2 = 100;
        e.d1 = 0;
    }
}

/// `ActEpisode1End` (game1.c:5470-5482) and `ActExitLineHorizontal`
/// (game1.c:5487-5498). Both are invisible lines that fire once when the
/// player crosses them; `data1` distinguishes the episode-2 exit line,
/// which is crossed from the other side.
///
/// The episode-1 lines show a cliffhanger message; the third wins the
/// game (`ShowE1CliffhangerMessage`, game2.c).
fn tick_trigger_line(e: &mut Enemy, player: &Player) -> bool {
    if e.d2 != 0 {
        return false;
    }
    let crossed = if e.d1 == 0 {
        e.y <= player.y && e.y >= player.y - 4
    } else {
        e.y >= player.y
    };
    if crossed {
        e.d2 = 1;
    }
    crossed
}

/// The cliffhanger text each `ACT_EP1_END_*` line shows (game2.c). The
/// third has no message - it ends the episode.
pub fn cliffhanger_lines(act_id: u16) -> Option<&'static [&'static str]> {
    match act_id {
        164 => Some(&[
            " What's happening?  Is",
            " Cosmo falling to his",
            " doom?",
        ]),
        165 => Some(&[
            " Is there no end to this",
            " pit?  And what danger",
            " awaits below?!",
        ]),
        _ => None,
    }
}

/// `ActScooter` (game1.c:5303-5330). Left alone it bobs on the spot,
/// settling onto whatever is below it every tenth tick.
///
/// `ActScooter` (game1.c:5303-5330). Left alone it bobs on the spot; once
/// mounted it simply follows the player, who is now flying it.
fn tick_scooter(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    e.frame = (e.frame + 1) & 3;

    if player.scooter != 0 {
        // Ridden: it sits under the player (game1.c:5310).
        e.x = player.x;
        e.y = player.y + 1;
        return;
    }

    e.d2 += 1;
    if e.d2 % 10 != 0 {
        return;
    }
    let grounded = |e: &Enemy| {
        test_sprite_move(
            Dir4::South, e.x, e.y + 1, e.width_tiles, e.height_tiles, level, data,
        ) != MoveResult::Free
    };
    if grounded(e) {
        e.y -= 1;
    } else {
        e.y += 1;
        if grounded(e) {
            e.y -= 1;
        }
    }
}

/// `ActBearTrap` (game1.c:4933-4990). Snaps shut on the player standing in
/// it and holds them for the length of its frame table.
///

fn tick_bear_trap(e: &mut Enemy, player: &Player) -> u32 {
    /// game1.c:4937 - open, then twenty-three ticks shut, then easing back
    /// open over the last three.
    const FRAMES: [usize; 27] = [
        0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 0,
    ];

    if e.d2 == 0 {
        // Waiting: it catches a player standing exactly on it
        // (game1.c:7751).
        if e.x == player.x && e.y == player.y {
            e.d2 = 1;
            e.sounds.push(crate::sfx::snd::BEAR_TRAP_CLOSE);
            e.bubble = Some(ACT_SPEECH_UMPH);
            return FRAMES.len() as u32;
        }
        e.frame = 0;
        return 0;
    }

    e.frame = FRAMES[e.d3 as usize % FRAMES.len()];
    e.d3 += 1;
    if e.d3 as usize == FRAMES.len() {
        e.d3 = 0;
        e.d2 = 0;
    }
    0
}

/// `ActBeamRobot` (game1.c:3280-3350). Paces at half speed under a beam
/// that stands on its head and reaches up to nineteen cells, stopping at
/// the ceiling. Both the robot and the beam hurt.
///
/// Destroyed, it leaves a column of explosions and a star every four
/// cells up its own beam (game1.c:3288-3293).
fn tick_beam_robot(e: &mut Enemy, level: &LevelJson, data: &GameData) {
    if e.d3 != 0 {
        // Blasted: an explosion and a star every four cells up the beam
        // it was casting (game1.c:3288-3293).
        let mut i = 0;
        while i < e.d3 {
            e.explosions.push((e.x, e.y - i));
            e.spawns.push((ACT_STAR_FLOAT, e.x, e.y - i));
            i += 4;
        }
        e.dead = true;
        return;
    }
    e.d5 = i32::from(e.d5 == 0);
    e.d4 += 1;

    if e.d1 != 0 {
        if e.d4 % 2 != 0 {
            e.x -= 1;
        }
        adjust_actor_move(e, Dir4::West, level, data);
        if !e.west_free {
            e.d1 = 0;
        }
    } else {
        if e.d4 % 2 != 0 {
            e.x += 1;
        }
        adjust_actor_move(e, Dir4::East, level, data);
        if !e.east_free {
            e.d1 = 1;
        }
    }

    // The beam: cells 2..21 above the robot, stopping at the first ceiling
    // (game1.c:3327-3336). Stored where the force field keeps its length,
    // so the same drawing and damage pass can serve both.
    let mut len = 0;
    for i in 2..21 {
        if test_sprite_move(
            Dir4::North, e.x + 1, e.y - i, e.width_tiles, e.height_tiles, level, data,
        ) != MoveResult::Free
        {
            break;
        }
        len = i - 1;
    }
    e.d2 = len;
}

/// `ActBoss` (game1.c:5588-5838). Five phases: it rises, pauses, then
/// bobs across the room for two hundred ticks, slams down chasing the
/// player, and climbs back to start again. Twelve pounces finish it.
///
/// `d1` is the phase, `d5` the hits taken, `d4` the walking direction.
/// The original borrows `westfree` as a hit-flash timer and `eastfree` as
/// the death sequence. The flash only picks a white draw mode, which is not
/// ported, so only the death timer is kept - held in `fall_time`, which
/// leaves the collision flags meaning what they say.
///
/// NOT PORTED: the boss music, and the parachute balls the harder build
/// throws. The damage sound is raised by `pounce_boss` rather than here.
fn tick_boss(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) -> bool {
    /// game1.c:5590 - the bob, as per-tick row offsets.
    const Y_JUMP: [i32; 14] = [2, 2, 1, 0, -1, -2, -2, -2, -2, -1, 0, 1, 2, 2];

    let free_below = |e: &Enemy, dy: i32| {
        test_sprite_move(
            Dir4::South, e.x, e.y + dy, e.width_tiles, e.height_tiles, level, data,
        ) == MoveResult::Free
    };

    // --- dying (game1.c:5612-5652) ---
    if e.fall_time > 0 {
        e.fall_time -= 1;
        if e.fall_time < 40 {
            e.y -= 1;
        }
        e.weighted = false;
        if e.fall_time < 40 && e.fall_time % 3 == 0 {
            e.decorations
                .push((SPR_SMOKE, 6, e.x, e.y, crate::effects::DIR8_NORTHWEST, 1));
            e.decorations
                .push((SPR_SMOKE, 6, e.x + 3, e.y, crate::effects::DIR8_NORTHEAST, 1));
        }
        if e.fall_time == 1 || e.y <= 0 {
            e.dead = true;
            return true; // won
        }
        return false;
    }

    // --- the fall that starts the death, once it has taken enough ---
    if e.d5 == BOSS_HITS {
        if free_below(e, 1) {
            e.y += 1;
        } else {
            e.fall_time = 80;
        }
        return false;
    }

    match e.d1 {
        // Rising into view.
        0 => {
            e.y -= 2;
            e.d2 += 1;
            if e.d2 == 6 {
                e.d1 = 1;
            }
        }
        // Hanging still.
        1 => {
            if e.d2 != 0 {
                e.d2 -= 1;
            } else {
                e.d1 = 2;
            }
        }
        // Bobbing across the room.
        2 => {
            let step = Y_JUMP[(e.d3.rem_euclid(14)) as usize];
            if !free_below(e, step) && step == 2 {
                e.y -= 2;
            } else if !free_below(e, step) && step == 1 {
                e.y -= 1;
            } else {
                e.y += step;
            }
            e.d3 += 1;
            e.d2 += 1;

            if e.d2 > 30 && e.d2 < 201 {
                if e.d4 != 0 {
                    if test_sprite_move(
                        Dir4::East, e.x + 1, e.y, e.width_tiles, e.height_tiles, level, data,
                    ) != MoveResult::Free
                    {
                        e.d4 = 0;
                    } else {
                        e.x += 1;
                    }
                } else if test_sprite_move(
                    Dir4::West, e.x - 1, e.y, e.width_tiles, e.height_tiles, level, data,
                ) == MoveResult::Free
                {
                    e.x -= 1;
                } else {
                    e.d4 = 1;
                }
            } else if e.d2 > 199 {
                e.d1 = 3;
                e.d2 = 0;
                e.d3 = 8;
            }
        }
        // Rising, then slamming down while chasing the player.
        3 => {
            e.d2 += 1;
            if e.d3 < 6 {
                e.d3 += 1;
                e.y -= 2;
            } else if e.d2 < 102 {
                e.weighted = true;
                if !free_below(e, 1) {
                    e.d3 = 0;
                    e.weighted = false;
                } else if e.x + 1 > player.x {
                    if test_sprite_move(
                        Dir4::West, e.x - 1, e.y, e.width_tiles, e.height_tiles, level, data,
                    ) == MoveResult::Free
                    {
                        e.x -= 1;
                    }
                } else if e.x + 3 < player.x
                    && test_sprite_move(
                        Dir4::East, e.x + 1, e.y, e.width_tiles, e.height_tiles, level, data,
                    ) == MoveResult::Free
                {
                    e.x += 1;
                }
            } else if !free_below(e, 1) || !free_below(e, 0) {
                e.d1 = 4;
                e.d2 = 0;
                e.d3 = 0;
                e.weighted = false;
            } else {
                e.y += 1;
            }
        }
        // Peeling back off the floor.
        _ => {
            e.weighted = false;
            e.y -= 1;
            e.d2 += 1;
            if e.d2 == 6 {
                e.d1 = 2;
                e.d3 = 0;
                e.d2 = 0;
            }
        }
    }
    false
}

/// A pounce landing on the boss (game1.c:7346-7375). Each hit knocks it
/// back into its bobbing phase; the twelfth starts the death sequence.
pub fn pounce_boss(e: &mut Enemy) {
    if e.fall_time > 0 || e.d5 >= BOSS_HITS {
        return;
    }
    e.d5 += 1;
    e.sounds.push(crate::sfx::snd::BOSS_DAMAGE);
    e.bubble = Some(ACT_SPEECH_WHOA);
    if e.d5 == 4 {
        // It sheds a piece on the fourth hit (game1.c:7373).
        e.shards.push((SPR_BOSS, 1, e.x, e.y - 4));
    }
    if e.d1 != 2 {
        e.d1 = 2;
        e.d2 = 31;
        e.d3 = 0;
        e.d4 = 1;
        e.weighted = false;
    }
}

/// `ActFrozenDN` (game1.c:5454-5520). Smashed out of its ice, then rises
/// through three timed phases.
///
/// NOT PORTED: the rescue message it ends on, which belongs with the
/// episode-2 ending rather than with the actor.
fn tick_frozen_dn(e: &mut Enemy) {
    match e.d1 {
        0 => {}
        1 => {
            e.d2 += 1;
            if e.d2 % 2 != 0 {
                e.y -= 1;
            }
            if e.d2 == 10 {
                e.d1 = 2;
                e.d2 = 0;
            }
        }
        2 => {
            e.d2 += 1;
            if e.d2 == 30 {
                e.d1 = 3;
                e.d2 = 0;
            }
        }
        _ => {}
    }
}

/// A blast freeing the frozen figure (game1.c:5459).
pub fn smash_frozen_dn(e: &mut Enemy) {
    if e.d1 == 0 {
        e.d1 = 1;
        e.x += 1;
        e.sounds.push(crate::sfx::snd::SMASH);
        // Six pieces of ice (game1.c:5461-5466).
        for (frame, dx, dy) in [
            (6, 0, -6), (7, 4, 0), (8, 0, -5),
            (9, 0, -4), (10, 5, -6), (11, 5, -4),
        ] {
            e.shards.push((SPR_FROZEN_DN, frame, e.x + dx, e.y + dy));
        }
    }
}

/// `ActSpeechBubble` (game1.c:5290-5310). Lives twenty ticks and follows
/// the player rather than staying where it was raised, so it reads as
/// something Cosmo is saying rather than an object in the world.
fn tick_speech_bubble(e: &mut Enemy, player: &Player) {
    if e.d1 == 0 {
        e.sounds.push(crate::sfx::snd::SPEECH_BUBBLE);
        // Only the "WOW! 50000 POINTS" bubble pays out, and it pays out
        // for existing rather than for anything the player then does.
        if e.d5 != 0 {
            e.score_award += 50_000;
        }
    }
    e.d1 += 1;
    if e.d1 == 20 {
        e.dead = true;
        return;
    }
    e.x = player.x - 1;
    e.y = player.y - 5;
}

fn tick_smoke_emitter(e: &mut Enemy) {
    e.d1 = e.next_rand(32) as i32;
    if e.d1 != 0 {
        return;
    }
    // One in thirty-two ticks, a puff (game1.c:5602-5609).
    if e.d5 != 0 {
        e.decorations
            .push((SPR_SMOKE, 6, e.x - 1, e.y, crate::effects::DIR8_NORTH, 1));
    } else {
        e.decorations
            .push((SPR_SMOKE_LARGE, 6, e.x - 2, e.y, crate::effects::DIR8_NORTH, 1));
    }
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
    mut switches: ResMut<SwitchState>,
    mut cliffhangers: EventWriter<crate::flow::ShowCliffhanger>,
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
            EnemyKind::SentryRobot => {
                tick_sentry_robot(&mut e, player, &switches, &level, &data)
            }
            EnemyKind::BabyGhostEgg => tick_baby_ghost_egg(&mut e, player),
            EnemyKind::JumpingBullet => tick_jumping_bullet(&mut e),
            EnemyKind::WormCrate => tick_worm_crate(&mut e, &level, &data),
            EnemyKind::SplittingPlatform => tick_splitting_platform(&mut e, player),
            EnemyKind::Door => tick_door(&mut e, &switches, &level),
            EnemyKind::Rocket => tick_rocket(&mut e, player, &level, &data),
            EnemyKind::JumpPadRobot => tick_jump_pad_robot(&mut e, &level, &data),
            EnemyKind::IvyPlant => tick_ivy_plant(&mut e),
            EnemyKind::HeadSwitch => tick_head_switch(&mut e, &mut switches),
            EnemyKind::FootSwitch => tick_foot_switch(&mut e, &mut switches),
            EnemyKind::MysteryWall => {
                tick_mystery_wall(&mut e, &mut switches, &level, &data)
            }
            EnemyKind::ForceField => {
                tick_force_field(&mut e, &switches, &level, &data)
            }
            EnemyKind::PusherRobot => tick_pusher_robot(&mut e, player, &level, &data),
            EnemyKind::Monument => tick_monument(&mut e),
            EnemyKind::Satellite => tick_satellite(&mut e),
            EnemyKind::TulipLauncher => tick_tulip_launcher(&mut e),
            EnemyKind::Scooter => tick_scooter(&mut e, player, &level, &data),
            EnemyKind::BearTrap => {
                let hold = tick_bear_trap(&mut e, player);
                if hold > 0 {
                    e.push_player = None;
                    e.hold_player = hold;
                }
            }
            EnemyKind::BeamRobot => tick_beam_robot(&mut e, &level, &data),
            EnemyKind::Transporter => {}
            EnemyKind::Boss => {
                if tick_boss(&mut e, player, &level, &data) {
                    e.won_level = true;
                }
            }
            EnemyKind::FrozenDN => tick_frozen_dn(&mut e),
            EnemyKind::SpeechBubble => tick_speech_bubble(&mut e, player),
            EnemyKind::TriggerLine => {
                if tick_trigger_line(&mut e, player) {
                    if let Some(lines) = cliffhanger_lines(e.act_id) {
                        cliffhangers.write(crate::flow::ShowCliffhanger(lines));
                    }
                }
            }
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
    effects: Res<crate::effects::EffectAssets>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
    mut xmode: ResMut<crate::effects::ShardXMode>,
    mut score: ResMut<crate::flow::Score>,
    mut seen: ResMut<SeenBubbles>,
    tileset: Option<Res<crate::tileset::TilesetAssets>>,
    mut tile_index: ResMut<crate::level::TileIndex>,
    mut current: ResMut<CurrentLevel>,
    mut player_q: Query<&mut Player>,
    mut query: Query<&mut Enemy>,
) {
    // Collected first so the borrow on the query ends before spawning,
    // which would otherwise conflict with the new entities.
    let mut requests = Vec::new();
    let mut writes = Vec::new();
    let mut decorations = Vec::new();
    let mut sounds: Vec<u16> = Vec::new();
    let mut shards: Vec<(u16, usize, i32, i32)> = Vec::new();
    let mut bubbles: Vec<(u16, i32, i32)> = Vec::new();
    let mut explosions: Vec<(i32, i32)> = Vec::new();
    let mut score_effects: Vec<(i32, i32)> = Vec::new();
    for mut e in &mut query {
        if !e.spawns.is_empty() {
            requests.append(&mut e.spawns);
        }
        if !e.tile_writes.is_empty() {
            writes.append(&mut e.tile_writes);
        }
        if !e.decorations.is_empty() {
            decorations.append(&mut e.decorations);
        }
        if !e.sounds.is_empty() {
            sounds.append(&mut e.sounds);
        }
        if !e.shards.is_empty() {
            shards.append(&mut e.shards);
        }
        if e.score_award != 0 {
            score.0 += e.score_award;
            e.score_award = 0;
        }
        if !e.explosions.is_empty() {
            explosions.append(&mut e.explosions);
        }
        if !e.score_effects.is_empty() {
            score_effects.append(&mut e.score_effects);
        }
        if e.carry_player {
            e.carry_player = false;
            if let Ok(mut player) = player_q.single_mut() {
                // Pinned to the nose: held out of a fall and pulled up one
                // row per tick, which is what riding one feels like.
                player.is_falling = false;
                player.fall_time = 0;
                player.recoil_left = 16;
                player.is_recoiling = true;
                player.clear_dizzy();
                player.long_jumping = false;
                player.y = e.y - 5;
            }
        }
        if let Some(act) = e.bubble.take() {
            // The one-shot gate: which flag depends on what raised it.
            let flag = match e.kind {
                EnemyKind::PusherRobot => Some(&mut seen.pusher_robot),
                EnemyKind::BearTrap => Some(&mut seen.bear_trap),
                EnemyKind::FootSwitch => Some(&mut seen.mystery_wall),
                EnemyKind::Boss => Some(&mut seen.boss),
                _ => None,
            };
            let fire = match flag {
                Some(f) if !*f => {
                    *f = true;
                    true
                }
                Some(_) => false,
                None => true,
            };
            if fire {
                bubbles.push((act, e.x, e.y));
            }
        }
        if e.hold_player > 0 {
            if let Ok(mut player) = player_q.single_mut() {
                player.held_ticks = e.hold_player;
            }
            e.hold_player = 0;
        }
        if let Some((dx, dy, max_time, speed)) = e.push_player.take() {
            if let Ok(mut player) = player_q.single_mut() {
                // Not abortable and blockable, as the pusher robot sets it
                // (game1.c:4510).
                player.set_push(dx, dy, max_time, speed, false, true);
            }
        }
    }
    if let Some(tileset) = tileset {
        for (x, y, raw) in writes {
            crate::level::set_map_tile(
                &mut commands,
                &tileset,
                &data,
                &mut tile_index,
                &mut current.level,
                x,
                y,
                raw,
            );
        }
    }
    for (act_type, x, y) in requests {
        crate::actors::spawn_one_actor(&mut commands, &asset_server, &data, act_type, x, y);
    }
    for (spr, frames, x, y, dir, times) in decorations {
        crate::effects::spawn_decoration(&mut commands, &effects, spr, frames, x, y, dir, times);
    }
    for number in sounds {
        sfx.write(crate::sfx::PlaySfx(number));
    }
    for (x, y) in score_effects {
        crate::effects::spawn_score_effect(&mut commands, &effects, 12_800, x, y);
    }
    for (x, y) in explosions {
        crate::effects::spawn_explosion(&mut commands, &effects, x, y);
    }
    for (act, x, y) in bubbles {
        crate::actors::spawn_one_actor(&mut commands, &asset_server, &data, act, x, y);
    }
    for (spr, frame, x, y) in shards {
        crate::effects::spawn_shard(&mut commands, &effects, &mut xmode, spr, frame, x, y);
    }
}

/// One drawn cell of a force field's beam. Rebuilt whenever the beam's
/// length changes, which in practice is once - the beam only shortens if
/// the map changes under it.
#[derive(Component)]
pub struct BeamSegment {
    owner: Entity,
}

/// Draws each live force field's beam and hurts the player standing in it.
///
/// The original redraws the beam cell by cell every tick inside the
/// behavior (game1.c:4370-4390). Here the behavior only measures it, and
/// this pass owns the entities, so the beam is rebuilt on change rather
/// than every tick.
pub fn draw_force_field_beams(
    mut commands: Commands,
    fields: Query<(Entity, &Enemy)>,
    segments: Query<(Entity, &BeamSegment)>,
    mut player_q: Query<&mut Player>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
    mut lengths: Local<bevy::platform::collections::HashMap<Entity, i32>>,
) {
    for (entity, e) in &fields {
        if !has_beam(e) {
            continue;
        }
        let want = beam_rect(e).map(|(_, _, w, h)| w.max(h)).unwrap_or(0);
        if lengths.get(&entity).copied() == Some(want) {
            continue;
        }
        lengths.insert(entity, want);

        for (seg, owned) in &segments {
            if owned.owner == entity {
                commands.entity(seg).despawn();
            }
        }
        let Some(frame) = e.frames.first() else {
            continue;
        };
        let Some((bx, by, bw, _)) = beam_rect(e) else {
            continue;
        };
        for i in 0..want {
            // Vertical beams grow upward from the bottom of the rect,
            // horizontal ones rightward from its left edge.
            let (x, y) = if bw == 1 { (bx, by + want - 1 - i) } else { (bx + i, by) };
            let pos = crate::level::tile_topleft_to_center(
                x as f32,
                (y - e.height_tiles + 1) as f32,
                e.width_tiles as f32 * crate::tileset::TILE_PX,
                e.height_tiles as f32 * crate::tileset::TILE_PX,
            );
            commands.spawn((
                Sprite {
                    image: frame.clone(),
                    ..default()
                },
                Transform::from_translation(pos.extend(6.0)),
                BeamSegment { owner: entity },
                crate::level::LevelScoped,
            ));
        }
    }

    // Damage: standing anywhere in the beam hurts, exactly as touching the
    // field itself would (game1.c:4373, 4384).
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.dead_timer != 0 || player.is_invincible() || player.hurt_cooldown > 0 {
        return;
    }
    for (_, e) in &fields {
        if !has_beam(e) {
            continue;
        }
        let Some((bx, by, bw, bh)) = beam_rect(e) else {
            continue;
        };
        let touching = crate::combat::rects_overlap(
            bx,
            by + bh - 1,
            bw,
            bh,
            player.x,
            player.y,
            crate::player::PLAYER_WIDTH,
            crate::player::PLAYER_HEIGHT,
        );
        if touching {
            player.cling_dir = None;
            player.health -= 1;
            if player.health <= 0 {
                player.dead_timer = 1;
            } else {
                sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::PLAYER_HURT));
                player.hurt_cooldown = 44;
            }
            return;
        }
    }
}

/// `ActTransporter` (game1.c:4075-4130). Stepping onto a pad starts a
/// fifteen-tick countdown; at the end the player is moved to the *other*
/// pad and the view re-centred on them. A pad numbered 3 wins the level
/// instead of moving anyone.
///
/// Cross-actor by nature - the destination is a different entity - so this
/// is a system rather than a behavior tick.
///
/// Sparkles and a first-time "whoa" go with the trip.
pub fn run_transporters(
    mut state: ResMut<TransporterState>,
    mut player_q: Query<&mut Player>,
    mut scroll: ResMut<crate::camera::Scroll>,
    level: Res<CurrentLevel>,
    pads: Query<&Enemy>,
    mut finished: EventWriter<crate::flow::LevelFinished>,
    stars: Res<crate::flow::Stars>,
    mut sequence: ResMut<crate::flow::LevelSequence>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };

    if state.active == 0 {
        // Look for a pad the player is standing on.
        for e in &pads {
            if e.kind != EnemyKind::Transporter || e.dead {
                continue;
            }
            if crate::hints::touching_player(&player, e.x, e.y, e.width_tiles, e.height_tiles) {
                state.active = e.d5;
                state.time_left = 15;
                sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::TRANSPORTER_ON));
                break;
            }
        }
        return;
    }

    if state.time_left > 1 {
        state.time_left -= 1;
        return;
    }

    if state.active == 3 {
        // Pad 3 is the exit: winning through it takes the same path as any
        // other level win (game1.c:4100).
        sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::WIN_LEVEL));
        let intermission = sequence.advance(stars.0);
        finished.write(crate::flow::LevelFinished {
            level: sequence.current().to_string(),
            intermission,
        });
        state.active = 0;
        state.time_left = 0;
        return;
    }

    // Move to the first pad that is neither the one stepped into nor an
    // exit pad (game1.c:4104).
    let dest = pads.iter().find(|e| {
        e.kind == EnemyKind::Transporter && !e.dead && e.d5 != state.active && e.d5 != 3
    });
    if let Some(dest) = dest {
        player.x = dest.x + 1;
        player.y = dest.y;
        player.is_recoiling = false;
        scroll.centre_on(&player, &level);
    }
    state.active = 0;
    state.time_left = 0;
}

/// Landing on a scooter while falling mounts it (game1.c:7682-7687).
pub fn mount_scooter(
    mut player_q: Query<&mut Player>,
    scooters: Query<&Enemy>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.scooter != 0 || player.dead_timer != 0 || !player.is_falling {
        return;
    }
    for e in &scooters {
        if e.kind != EnemyKind::Scooter || e.dead {
            continue;
        }
        if e.x == player.x && (e.y == player.y || e.y + 1 == player.y) {
            // Four ticks of forced lift, then free flight.
            player.scooter = 4;
            player.is_falling = false;
            player.fall_time = 0;
            player.push = None;
            sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::PLAYER_LAND));
            return;
        }
    }
}

/// The pipe network (game1.c:7613-7656).
///
/// A pipe end is an entrance while the player is outside and an exit while
/// they are being carried. Jumping at one puts them in the pipe; each
/// corner then shoves them along the next leg, hidden, for up to a hundred
/// ticks; and the far end drops them out dizzy.
///
/// Cross-actor and player-state by nature, so a system rather than a
/// behaviour tick.
///
/// A first ride raises a "whoa".
pub fn run_pipes(
    mut player_q: Query<&mut Player>,
    pipes: Query<&Enemy>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.dead_timer != 0 {
        return;
    }

    for e in &pipes {
        if e.dead {
            continue;
        }
        match e.kind {
            EnemyKind::PipeCorner => {
                // Corners only act on someone already inside the network.
                if !player.in_pipe
                    || !crate::hints::touching_player(
                        &player,
                        e.x,
                        e.y,
                        e.width_tiles,
                        e.height_tiles,
                    )
                {
                    continue;
                }
                let (dx, dy) = crate::effects::DIR8[e.d5 as usize % 9];
                // Not abortable and not blockable: once you are in the
                // pipe you go where it goes (game1.c:7620).
                player.set_push(dx, dy, 100, 2, false, false);
                sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::PIPE_CORNER_HIT));
            }
            EnemyKind::PipeEnd => {
                // The end's own row is three below its origin
                // (game1.c:7640).
                let at_mouth = e.x == player.x && (e.y + 3 == player.y || e.y + 2 == player.y);
                if !at_mouth {
                    continue;
                }
                if player.in_pipe && player.push.is_some() {
                    // Spat out: the ride ends here.
                    player.x = e.x;
                    player.in_pipe = false;
                    player.push = None;
                    player.queue_dizzy();
                } else if !player.in_pipe && !player.is_falling {
                    player.in_pipe = true;
                }
            }
            _ => {}
        }
    }
}

/// Ends the level once the boss's death sequence finishes, paying out the
/// 100000 the original awards (game1.c:5634).
pub fn finish_on_boss_defeat(
    mut bosses: Query<&mut Enemy>,
    mut score: ResMut<crate::flow::Score>,
    stars: Res<crate::flow::Stars>,
    mut sequence: ResMut<crate::flow::LevelSequence>,
    mut finished: EventWriter<crate::flow::LevelFinished>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
) {
    for mut e in &mut bosses {
        if !e.won_level {
            continue;
        }
        e.won_level = false;
        score.0 += 100_000;
        sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::WIN_LEVEL));
        let intermission = sequence.advance(stars.0);
        finished.write(crate::flow::LevelFinished {
            level: sequence.current().to_string(),
            intermission,
        });
    }
}

fn draws_hidden(e: &Enemy) -> bool {
    match e.kind {
        // Never drawn at all - it only spawns smoke (game1.c:5602).
        EnemyKind::SmokeEmitter => true,
        // The pad itself is never drawn; only its copy sprite is
        // (game1.c:4079).
        EnemyKind::Transporter => true,
        // Trigger lines are invisible markers (game1.c:5473, 5497).
        EnemyKind::TriggerLine => true,
        // The force field's own sprite is never drawn - only its beam is,
        // by `draw_force_field_beams` (game1.c:4356).
        EnemyKind::ForceField => true,
        // The corner is never drawn: the pipe artwork is already in the
        // map tiles, and the actor only marks the turn (game1.c:3052).
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
/// A single-frame prize sparkles instead of animating, which is what the
/// original does for the ones with nothing to cycle (game1.c:4920).
fn tick_prize(e: &mut Enemy) {
    // Nothing to cycle means it sparkles instead (game1.c:4920).
    if e.d5 <= 1 && e.next_rand(16) == 0 {
        e.decorations
            .push((SPR_SPARKLE_SHORT, 4, e.x, e.y, crate::effects::DIR8_NONE, 1));
    }
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
fn tick_reciprocating_spikes(e: &mut Enemy) {
    e.d2 += 1;
    e.sounds.push(crate::sfx::snd::SPIKES_MOVE);
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
/// `ActFallingFloor` (game1.c:2900-2940). Lays a two-tile platform across
/// its own top so the player can stand on it, drops it seven ticks after
/// they do, and breaks on landing.
///
/// The saved tiles go in `d3`/`d4`; the original borrows `westfree` and
/// `eastfree` for the same purpose, which look like collision flags and
/// are not.
///
fn tick_falling_floor(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    let (w, h) = (e.width_tiles, e.height_tiles);

    if test_sprite_move(Dir4::South, e.x, e.y + 1, w, h, level, data) != MoveResult::Free {
        // Transcribed as-is: the original kills it the moment anything is
        // below, with no guard for the first tick. No falling floor in any
        // of the three episodes is placed on solid ground, so this only
        // ever fires after a fall.
        e.dead = true;
        e.sounds.push(crate::sfx::snd::DESTROY_SOLID);
        e.shards.push((SPR_FALLING_FLOOR, 1, e.x, e.y));
        e.shards.push((SPR_FALLING_FLOOR, 2, e.x, e.y));
        return;
    }

    if e.d1 == 0 {
        e.d1 = 1;
        let read = |x: i32| {
            if x >= 0 && e.y - 1 >= 0 && (x as usize) < level.width {
                level.tile_at(x as usize, (e.y - 1) as usize) as i32
            } else {
                0
            }
        };
        e.d3 = read(e.x);
        e.d4 = read(e.x + 1);
        e.tile_writes.push((e.x, e.y - 1, TILE_STRIPED_PLATFORM));
        e.tile_writes.push((e.x + 1, e.y - 1, TILE_STRIPED_PLATFORM));
    }

    if e.y - 2 == player.y && e.x <= player.x + 2 && e.x + 1 >= player.x {
        e.d2 = 7;
    }

    if e.d2 != 0 {
        e.d2 -= 1;
        if e.d2 == 0 {
            // Handed to the shared gravity pass rather than moved by hand,
            // as the original does by setting `weighted`.
            e.weighted = true;
            e.tile_writes.push((e.x, e.y - 1, e.d3 as u16));
            e.tile_writes.push((e.x + 1, e.y - 1, e.d4 as u16));
        }
    }
}

/// `ActPyramid` (game1.c:2657-2693). The floor-mounted variant is inert
/// scenery; the ceiling-mounted one drops once the player walks beneath it.
///
/// A blast three ticks earlier destroys it for 200 and a shard. The
/// original notes that non-falling pyramids use a different function and
/// so do not propagate the explosion; that asymmetry is kept.
fn tick_pyramid(e: &mut Enemy, player: &Player, level: &LevelJson, data: &GameData) {
    if e.d2 != 0 {
        e.d2 -= 1;
        if e.d2 == 0 {
            e.explosions.push((e.x - 1, e.y + 1));
            e.dead = true;
            e.score_award += 200;
            e.shards.push((SPR_PYRAMID, 0, e.x, e.y));
            return;
        }
    }
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
    fn the_boss_takes_twelve_pounces_and_then_dies() {
        let (level, data) = world(&[
            "........",
            "........",
            "........",
            "........",
            "########",
        ]);
        let mut p = Player::spawn_at(4, 3);
        p.x = 4;
        p.y = 3;
        let mut e = Enemy::default_for_test(EnemyKind::Boss);
        e.x = 2;
        e.y = 3;
        e.width_tiles = 1;
        e.height_tiles = 1;

        // Pounced back to back so it stays on the floor; ticking between
        // hits would let it float up its bobbing arc, and with no map
        // below the test world it would then fall out of the bottom.
        for hit in 1..BOSS_HITS {
            pounce_boss(&mut e);
            assert_eq!(e.d5, hit, "hit {hit} should count");
            assert!(!e.dead, "it must survive hit {hit}");
        }
        pounce_boss(&mut e);
        assert_eq!(e.d5, BOSS_HITS);

        // The last hit starts a fall, then an eighty-tick death sequence.
        let mut won = false;
        for _ in 0..200 {
            won |= tick_boss(&mut e, &p, &level, &data);
            if e.dead {
                break;
            }
        }
        assert!(e.dead, "it should die after the final hit");
        assert!(won, "and report the win");
    }

    #[test]
    fn pouncing_a_dying_boss_does_nothing() {
        let mut e = Enemy::default_for_test(EnemyKind::Boss);
        e.d5 = BOSS_HITS;
        pounce_boss(&mut e);
        assert_eq!(e.d5, BOSS_HITS, "hits must not climb past the limit");
    }

    #[test]
    fn the_frozen_figure_rises_only_after_being_smashed() {
        let mut e = Enemy::default_for_test(EnemyKind::FrozenDN);
        let start = e.y;
        for _ in 0..50 {
            tick_frozen_dn(&mut e);
        }
        assert_eq!(e.y, start, "it stays put inside the ice");

        smash_frozen_dn(&mut e);
        for _ in 0..50 {
            tick_frozen_dn(&mut e);
        }
        assert!(e.y < start, "and rises once freed");
        assert_eq!(e.d1, 3, "ending in its final phase");
    }

    #[test]
    fn the_bear_trap_holds_the_player_it_catches() {
        let mut e = Enemy::default_for_test(EnemyKind::BearTrap);
        e.x = 10;
        e.y = 10;

        let mut beside = Player::spawn_at(12, 10);
        beside.x = 12;
        beside.y = 10;
        assert_eq!(tick_bear_trap(&mut e, &beside), 0, "it only catches a direct step");

        let mut on_it = Player::spawn_at(10, 10);
        on_it.x = 10;
        on_it.y = 10;
        let hold = tick_bear_trap(&mut e, &on_it);
        assert_eq!(hold, 27, "held for the length of the frame table");

        // It runs its animation out and reopens rather than staying shut.
        for _ in 0..27 {
            tick_bear_trap(&mut e, &on_it);
        }
        assert_eq!(e.d2, 0, "the trap reopens");
        assert_eq!(e.frame, 0, "and ends up drawn open");
    }

    #[test]
    fn the_beam_robot_carries_a_beam_that_stops_at_the_ceiling() {
        let (level, data) = world(&[
            "########",
            "........",
            "........",
            "........",
            "########",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::BeamRobot);
        e.x = 2;
        e.y = 3;
        e.width_tiles = 1;
        e.height_tiles = 1;
        tick_beam_robot(&mut e, &level, &data);
        let rect = beam_rect(&e).expect("it should have a beam");
        let (_, top, _, h) = rect;
        assert!(h > 0, "the beam should reach up the shaft");
        assert!(top >= 1, "but stop below the ceiling, got top row {top}");
    }

    #[test]
    fn a_scooter_settles_onto_the_ground_under_it() {
        let (level, data) = world(&[
            "....",
            "....",
            "....",
            "####",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::Scooter);
        e.x = 1;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;
        for _ in 0..200 {
            tick_scooter(&mut e, &Player::spawn_at(0, 0), &level, &data);
            assert!(e.y < 3, "it must not sink into the floor (y={})", e.y);
        }
        assert!(e.y >= 1, "and should have fallen toward it");
    }

    /// The pipe network is a system, not a tick, so these exercise the
    /// player-state transitions it drives rather than calling it.
    #[test]
    fn a_pipe_ride_hides_the_player_and_keeps_them_safe() {
        let mut p = Player::spawn_at(10, 10);
        assert!(!p.is_invincible(), "ordinarily vulnerable");
        p.in_pipe = true;
        assert!(
            p.is_invincible(),
            "riding a pipe is as safe as the bubble (game1.c:6905)"
        );
    }

    #[test]
    fn leaving_a_pipe_leaves_the_player_dizzy() {
        // The exit queues a head-shake, which then waits for the ground.
        let mut p = Player::spawn_at(10, 10);
        p.in_pipe = true;
        p.set_push(1, 0, 100, 2, false, false);

        // What run_pipes does at the far end.
        p.in_pipe = false;
        p.push = None;
        p.queue_dizzy();

        assert!(!p.is_invincible(), "and vulnerable again");
        p.process_dizzy(true);
        assert_ne!(p.dizzy_left, 0, "spat out disoriented");
    }

    #[test]
    fn a_pipe_corners_direction_comes_from_its_actor_id() {
        // Each corner turns the rider a different way; getting the DIR8
        // index wrong would send them through a wall.
        let dir_of = |act: u16| {
            let (_, data) = behavior_for(act).expect("corner should be in the table");
            crate::effects::DIR8[data[4] as usize % 9]
        };
        assert_eq!(dir_of(70), (0, -1), "ACT_PIPE_CORNER_N goes north");
        assert_eq!(dir_of(71), (0, 1), "ACT_PIPE_CORNER_S goes south");
        assert_eq!(dir_of(72), (-1, 0), "ACT_PIPE_CORNER_W goes west");
        assert_eq!(dir_of(73), (1, 0), "ACT_PIPE_CORNER_E goes east");
    }

    #[test]
    fn behaviours_ask_for_their_sounds() {
        // A spot check across the queue: each of these was silent before,
        // with a NOT PORTED note explaining why.
        let mut piston = Enemy::default_for_test(EnemyKind::ArrowPiston);
        let mut heard = false;
        for _ in 0..64 {
            tick_arrow_piston(&mut piston);
            heard |= piston.sounds.drain(..).any(|s| s == crate::sfx::snd::SPIKES_MOVE);
        }
        assert!(heard, "the piston should be audible when it moves");

        let mut plant = Enemy::default_for_test(EnemyKind::HeartPlant);
        plant.x = 10;
        plant.y = 10;
        let mut above = Player::spawn_at(10, 4);
        above.x = 10;
        above.y = 4;
        let mut opened = false;
        for _ in 0..20 {
            tick_heart_plant(&mut plant, &above);
            opened |= plant
                .sounds
                .drain(..)
                .any(|s| s == crate::sfx::snd::PLANT_MOUTH_OPEN);
        }
        assert!(opened, "the plant should be audible when it opens");
    }

    #[test]
    fn a_silent_behaviour_stays_silent() {
        // The eye plant has no sound in the original; the queue must not
        // become a place where every actor makes noise.
        let mut e = Enemy::default_for_test(EnemyKind::EyePlant);
        let p = Player::spawn_at(5, 10);
        for _ in 0..200 {
            tick_eye_plant(&mut e, &p);
        }
        assert!(e.sounds.is_empty());
    }

    #[test]
    fn the_actor_ids_spawned_at_runtime_are_the_right_ones() {
        // Straight from actor.h. These are easy to get wrong because they
        // are written once and never read back: ACT_PARACHUTE_BALL was 22
        // for a while, which is ACT_SAW_BLADE_HORIZ, so the tulip launcher
        // threw a saw blade that had no artwork and so drew nothing.
        assert_eq!(ACT_PARACHUTE_BALL, 86, "actor.h:132");
        assert_eq!(ACT_BABY_GHOST, 65, "actor.h:114");
        assert_eq!(ACT_HAMBURGER, 82, "actor.h:128");
        assert_eq!(ACT_PINK_WORM, 124, "actor.h:166");
        assert_eq!(ACT_PROJECTILE_W, 109, "actor.h:151");
        assert_eq!(ACT_PROJECTILE_E, 110, "actor.h:152");
        assert_eq!(ACT_PROJECTILE_SW, 66, "actor.h:115");
        assert_eq!(ACT_PROJECTILE_SE, 67, "actor.h:116");
        assert_eq!(ACT_PROJECTILE_S, 68, "actor.h:117");
    }

    #[test]
    fn everything_spawned_at_runtime_has_artwork_to_draw() {
        // A sprite is only converted if some level places it, so an actor
        // that exists only at runtime needs forcing into the conversion.
        // Without that the spawn silently produces nothing - which is how
        // the turrets came to fire invisible projectiles.
        use opencosmo_assets::actor_sprite_map::ACT_TO_SPRITE;
        let spr_of = |act: u16| {
            ACT_TO_SPRITE
                .iter()
                .find(|(id, ..)| *id == act)
                .map(|(_, s, ..)| *s)
                .unwrap_or(act)
        };
        let forced: Vec<u16> = opencosmo_assets::convert::EFFECT_SPRITES
            .iter()
            .chain(opencosmo_assets::convert::RUNTIME_SPAWNED_SPRITES)
            .copied()
            .collect();
        // Every actor a behaviour can spawn out of thin air.
        for act in [
            ACT_PROJECTILE_W,
            ACT_PROJECTILE_E,
            ACT_PROJECTILE_SW,
            ACT_PROJECTILE_SE,
            ACT_PROJECTILE_S,
            ACT_BABY_GHOST,
            ACT_HAMBURGER,
        ] {
            let spr = spr_of(act);
            assert!(
                forced.contains(&spr),
                "ACT {act} draws SPR {spr}, which no level is guaranteed to \
                 place - add it to RUNTIME_SPAWNED_SPRITES or it will spawn \
                 invisible"
            );
        }
    }

    #[test]
    fn only_the_first_two_end_lines_have_something_to_say() {
        // ACT_EP1_END_1 and _2 show a cliffhanger; _3 ends the episode
        // instead and has no message (game2.c).
        assert!(cliffhanger_lines(164).is_some());
        assert!(cliffhanger_lines(165).is_some());
        assert!(cliffhanger_lines(166).is_none(), "the third wins the game");
        assert!(cliffhanger_lines(999).is_none());
        for act in [164, 165] {
            let lines = cliffhanger_lines(act).unwrap();
            assert!(!lines.is_empty());
            assert!(
                lines.iter().all(|l| l.len() <= 26),
                "a line would run through the frame edge"
            );
        }
    }

    #[test]
    fn the_sentry_robot_holds_its_fire_in_the_dark() {
        // game1.c:4575 gates it on `areLightsActive`. Without that it
        // shot at the player through an unlit room.
        let (level, data) = world(&["................", "################"]);
        let fired_under = |lights_active: bool| {
            let mut switches = SwitchState::default();
            switches.lights_active = lights_active;
            let mut e = Enemy::default_for_test(EnemyKind::SentryRobot);
            e.x = 8;
            e.y = 0;
            e.width_tiles = 1;
            e.height_tiles = 1;
            let mut p = Player::spawn_at(1, 0);
            p.x = 1;
            p.y = 0;
            for _ in 0..4000 {
                tick_sentry_robot(&mut e, &p, &switches, &level, &data);
                if !e.spawns.is_empty() {
                    return true;
                }
            }
            false
        };
        assert!(fired_under(true), "it should fire with the lights on");
        assert!(!fired_under(false), "and hold fire with them off");
    }

    #[test]
    fn a_blasted_pyramid_pays_out_and_leaves_a_shard() {
        let (level, data) = world(&["....", "####"]);
        let p = Player::spawn_at(30, 0);
        let mut e = Enemy::default_for_test(EnemyKind::Pyramid);
        e.x = 1;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;
        // What the blast code sets.
        e.d2 = 3;
        for _ in 0..4 {
            if e.dead {
                break;
            }
            tick_pyramid(&mut e, &p, &level, &data);
        }
        assert!(e.dead, "three ticks after the blast it goes");
        assert_eq!(e.score_award, 200, "game1.c's own figure");
        assert_eq!(e.shards.len(), 1);
        assert_eq!(e.explosions.len(), 1);
    }

    #[test]
    fn a_blasted_beam_robot_goes_up_in_a_column() {
        let (level, data) = world(&["....", "####"]);
        let mut e = Enemy::default_for_test(EnemyKind::BeamRobot);
        e.x = 1;
        e.y = 20;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d3 = 12; // the beam length the blast recorded
        tick_beam_robot(&mut e, &level, &data);
        assert!(e.dead);
        // One explosion and one star every four cells (game1.c:3289).
        assert_eq!(e.explosions.len(), 3);
        assert_eq!(e.spawns.len(), 3);
        assert!(e.spawns.iter().all(|(a, ..)| *a == ACT_STAR_FLOAT));
        let ys: Vec<i32> = e.explosions.iter().map(|(_, y)| *y).collect();
        assert_eq!(ys, vec![20, 16, 12], "spaced up its own beam");
    }

    #[test]
    fn a_speech_bubble_follows_the_player_and_expires() {
        let mut e = Enemy::default_for_test(EnemyKind::SpeechBubble);
        let mut p = Player::spawn_at(40, 30);
        p.x = 40;
        p.y = 30;

        tick_speech_bubble(&mut e, &p);
        assert_eq!(
            e.sounds,
            vec![crate::sfx::snd::SPEECH_BUBBLE],
            "it speaks once, on its first tick"
        );
        assert_eq!((e.x, e.y), (39, 25), "and sits above the player's head");

        // It tracks them rather than staying put.
        p.x = 100;
        p.y = 12;
        tick_speech_bubble(&mut e, &p);
        assert_eq!((e.x, e.y), (99, 7));

        let mut ticks = 2;
        while !e.dead {
            tick_speech_bubble(&mut e, &p);
            ticks += 1;
            assert!(ticks < 100, "a bubble that never expired would follow forever");
        }
        assert_eq!(ticks, 20, "twenty ticks (game1.c:5305)");
    }

    #[test]
    fn only_the_jackpot_bubble_pays_out() {
        let p = Player::spawn_at(10, 10);
        let mut plain = Enemy::default_for_test(EnemyKind::SpeechBubble);
        tick_speech_bubble(&mut plain, &p);
        assert_eq!(plain.score_award, 0, "'whoa' is not worth anything");

        let mut jackpot = Enemy::default_for_test(EnemyKind::SpeechBubble);
        jackpot.d5 = 1;
        tick_speech_bubble(&mut jackpot, &p);
        assert_eq!(jackpot.score_award, 50_000);

        // ...and only once, however long it lives.
        for _ in 0..18 {
            tick_speech_bubble(&mut jackpot, &p);
        }
        assert_eq!(jackpot.score_award, 50_000);
    }

    #[test]
    fn the_bubble_actor_ids_are_right() {
        assert_eq!(ACT_SPEECH_OUCH, 235, "actor.h:271");
        assert_eq!(ACT_SPEECH_WHOA, 244, "actor.h:280");
        assert_eq!(ACT_SPEECH_UMPH, 245, "actor.h:281");
        assert_eq!(ACT_SPEECH_WOW_50K, 246, "actor.h:282");
    }

    #[test]
    fn the_smoke_emitter_puffs_occasionally_rather_than_constantly() {
        // One tick in thirty-two. Over 3200 ticks that is ~100; wide
        // bounds, but they catch an emitter that never fires or fires
        // every tick - the two ways this actor can be wrong.
        let mut e = Enemy::default_for_test(EnemyKind::SmokeEmitter);
        let mut puffs = 0;
        for _ in 0..3200 {
            tick_smoke_emitter(&mut e);
            puffs += e.decorations.len();
            e.decorations.clear();
        }
        assert!(
            (20..400).contains(&puffs),
            "puffed {puffs} times in 3200 ticks, expected roughly 100"
        );
    }

    #[test]
    fn the_smoke_emitter_picks_its_plume_from_data5() {
        let puff_for = |d5: i32| {
            let mut e = Enemy::default_for_test(EnemyKind::SmokeEmitter);
            e.d5 = d5;
            for _ in 0..3200 {
                tick_smoke_emitter(&mut e);
                if let Some(d) = e.decorations.first() {
                    return (d.0, d.2);
                }
            }
            panic!("never puffed");
        };
        assert_eq!(puff_for(1).0, SPR_SMOKE, "the small plume");
        assert_eq!(puff_for(0).0, SPR_SMOKE_LARGE, "the large one");
        assert!(
            puff_for(0).1 < puff_for(1).1,
            "the large plume is offset further left to stay centred"
        );
    }

    #[test]
    fn a_monument_needs_three_blasts_and_leaves_no_wall_behind() {
        let mut e = Enemy::default_for_test(EnemyKind::Monument);
        e.x = 5;
        e.y = 20;

        tick_monument(&mut e);
        assert_eq!(e.tile_writes.len(), 9, "it stands as nine tiles of solid");
        assert!(e.tile_writes.iter().all(|(_, _, r)| *r == TILE_SWITCH_BLOCK));
        e.tile_writes.clear();

        // Three blasts, and the flash has to run out before another counts.
        assert!(!blast_monument(&mut e), "one blast is not enough");
        assert!(!e.dead);
        for _ in 0..12 {
            tick_monument(&mut e);
        }
        assert!(!blast_monument(&mut e), "nor two");
        for _ in 0..12 {
            tick_monument(&mut e);
        }
        assert!(blast_monument(&mut e), "the third brings it down");
        tick_monument(&mut e);
        assert!(e.dead);
        assert_eq!(e.tile_writes.len(), 9, "and clears every tile it stood as");
        assert!(
            e.tile_writes.iter().all(|(_, _, r)| *r == TILE_EMPTY),
            "a monument that left its tiles behind would be an invisible wall"
        );
    }

    #[test]
    fn a_satellite_drops_a_hamburger_when_destroyed() {
        let mut e = Enemy::default_for_test(EnemyKind::Satellite);
        blast_satellite(&mut e);
        assert!(!e.dead, "the first blast only stuns it");
        for _ in 0..16 {
            tick_satellite(&mut e);
        }
        blast_satellite(&mut e);
        assert!(e.dead);
        assert_eq!(e.spawns.len(), 1);
        assert_eq!(e.spawns[0].0, ACT_HAMBURGER);
    }

    #[test]
    fn the_tulip_launcher_throws_one_ball_per_cycle() {
        let mut e = Enemy::default_for_test(EnemyKind::TulipLauncher);
        let mut at = Vec::new();
        for tick in 0..400 {
            tick_tulip_launcher(&mut e);
            if !e.spawns.is_empty() {
                assert_eq!(e.spawns.len(), 1, "one ball at a time");
                assert_eq!(e.spawns[0].0, ACT_PARACHUTE_BALL);
                at.push(tick);
                e.spawns.clear();
            }
        }
        // Five wind-up ticks then a hundred idle.
        let gaps: Vec<_> = at.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            gaps.iter().all(|g| *g == 105),
            "throws should be 105 ticks apart, got {gaps:?}"
        );
        assert!(at.len() >= 3, "and it should keep launching");
    }

    #[test]
    fn a_trigger_line_fires_once_and_only_once() {
        let mut e = Enemy::default_for_test(EnemyKind::TriggerLine);
        e.y = 10;
        let mut away = Player::spawn_at(0, 30);
        away.y = 30;
        assert!(!tick_trigger_line(&mut e, &away), "not crossed yet");

        let mut on_it = Player::spawn_at(0, 12);
        on_it.y = 12;
        assert!(tick_trigger_line(&mut e, &on_it), "crossing fires it");
        assert!(
            !tick_trigger_line(&mut e, &on_it),
            "but standing there must not fire it again"
        );
    }

    #[test]
    fn the_pusher_robot_shoves_only_when_the_player_is_in_reach() {
        let (level, data) = world(&[
            "................",
            "................",
            "################",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::PusherRobot);
        e.x = 8;
        e.y = 1;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d1 = DIR2_WEST;

        // Player on the wrong row: paces, never shoves.
        let mut wrong_row = Player::spawn_at(5, 0);
        wrong_row.x = 5;
        wrong_row.y = 0;
        for _ in 0..50 {
            tick_pusher_robot(&mut e, &wrong_row, &level, &data);
            assert!(e.push_player.is_none(), "it must not shove across rows");
        }

        // Exactly three columns west of it, same row.
        e.x = 8;
        e.d1 = DIR2_WEST;
        e.d2 = 0;
        e.d4 = 0;
        let mut in_reach = Player::spawn_at(5, 1);
        in_reach.x = 5;
        in_reach.y = 1;
        tick_pusher_robot(&mut e, &in_reach, &level, &data);
        assert_eq!(
            e.push_player,
            Some((-1, 0, 5, 2)),
            "it should shove the player west"
        );
    }

    #[test]
    fn a_shove_carries_the_player_and_then_ends() {
        use crate::player::Push;
        let mut p = Player::spawn_at(10, 5);
        p.x = 10;
        p.y = 5;
        p.set_push(1, 0, 5, 2, false, true);
        assert_eq!(
            p.push,
            Some(Push {
                dx: 1,
                dy: 0,
                speed: 2,
                time: 0,
                max_time: 5,
                abortable: false,
                blockable: true
            })
        );
    }

    #[test]
    fn a_falling_floor_is_solid_until_someone_stands_on_it() {
        // Spanning a gap, with air below - which is how every one of the
        // 63 placements in the shipped episodes is positioned.
        let (level, data) = world(&[
            "........",
            "........",
            "........",
            "........",
            "........",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::FallingFloor);
        e.x = 2;
        e.y = 2;
        e.width_tiles = 2;
        e.height_tiles = 1;

        let mut away = Player::spawn_at(30, 0);
        away.x = 30;
        away.y = 0;
        tick_falling_floor(&mut e, &away, &level, &data);
        assert_eq!(
            e.tile_writes,
            vec![(2, 1, TILE_STRIPED_PLATFORM), (3, 1, TILE_STRIPED_PLATFORM)],
            "it lays a platform you can stand on"
        );
        e.tile_writes.clear();
        for _ in 0..30 {
            tick_falling_floor(&mut e, &away, &level, &data);
        }
        assert!(!e.weighted, "it must not fall with nobody on it");
        assert!(e.tile_writes.is_empty(), "and must not restamp every tick");

        // Standing on it: the player's row is two above the actor's. The
        // trigger re-arms every tick they stay, so it does not give way
        // underneath them - it gives way a few ticks after they step off,
        // which is what makes it collapse just behind you.
        let mut on = Player::spawn_at(2, 0);
        on.x = 2;
        on.y = 0;
        for _ in 0..20 {
            tick_falling_floor(&mut e, &on, &level, &data);
        }
        assert!(!e.weighted, "it holds while it is being stood on");

        for _ in 0..8 {
            tick_falling_floor(&mut e, &away, &level, &data);
        }
        assert!(e.weighted, "and lets go seven ticks after they leave");
        assert_eq!(
            e.tile_writes,
            vec![(2, 1, 0), (3, 1, 0)],
            "and put back exactly the tiles it covered, or it would leave \
             an invisible ledge in mid-air"
        );
    }

    #[test]
    fn a_force_field_reaches_from_its_cell_to_the_wall() {
        // A vertical field in a four-high shaft should span the shaft and
        // stop at the ceiling, not run off the top of the map.
        let (level, data) = world(&[
            "####",
            "....",
            "....",
            "....",
            "####",
        ]);
        let switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::ForceField);
        e.x = 1;
        e.y = 3;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d5 = 0; // vertical

        tick_force_field(&mut e, &switches, &level, &data);
        assert_eq!(e.d1, 3, "three open rows between floor and ceiling");
        assert_eq!(beam_rect(&e), Some((1, 1, 1, 3)));
    }

    #[test]
    fn a_horizontal_force_field_runs_along_its_row() {
        let (level, data) = world(&[
            "#......#",
            "########",
        ]);
        let switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::ForceField);
        e.x = 1;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d5 = 1; // horizontal

        tick_force_field(&mut e, &switches, &level, &data);
        assert_eq!(e.d1, 6, "six open columns before the far wall");
    }

    #[test]
    fn throwing_the_switch_removes_the_force_fields_for_good() {
        let (level, data) = world(&["....", "####"]);
        let mut switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::ForceField);
        e.x = 1;
        e.y = 0;
        e.width_tiles = 1;
        e.height_tiles = 1;

        tick_force_field(&mut e, &switches, &level, &data);
        assert!(beam_rect(&e).is_some(), "on to begin with");

        switches.force_fields_active = false;
        tick_force_field(&mut e, &switches, &level, &data);
        assert!(e.dead, "the switch kills them outright (game1.c:4360)");
        assert_eq!(beam_rect(&e), None, "and the beam goes with it");
    }

    #[test]
    fn a_head_switch_unlocks_only_its_own_colour_of_door() {
        let mut switches = SwitchState::default();
        let mut sw = Enemy::default_for_test(EnemyKind::HeadSwitch);
        sw.d5 = 13; // ACT_DOOR_GREEN

        // Un-pounced, it does nothing.
        for _ in 0..10 {
            tick_head_switch(&mut sw, &mut switches);
        }
        assert!(switches.doors_opened.is_empty());

        sw.frame = 1; // what the pounce sets
        for _ in 0..10 {
            tick_head_switch(&mut sw, &mut switches);
        }
        assert_eq!(switches.doors_opened, vec![13], "only green, and only once");
    }

    #[test]
    fn a_door_reopens_to_exactly_the_tiles_it_covered() {
        // The door remembers what it painted over; a blue door that
        // reopened to the wrong tiles would leave a hole or a wall.
        let (level, data) = world(&[
            "..##....",
            "..##....",
            "..##....",
            "..##....",
            "..##....",
            "########",
        ]);
        let _ = data;
        let mut switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::Door);
        e.act_id = 11; // ACT_DOOR_BLUE
        e.x = 1;
        e.y = 4;

        tick_door(&mut e, &switches, &level);
        let stamped: Vec<_> = e.tile_writes.drain(..).collect();
        assert_eq!(stamped.len(), 5);
        assert!(
            stamped.iter().all(|(_, _, raw)| *raw == TILE_DOOR_BLOCK),
            "it should have made itself solid"
        );
        let covered: Vec<u16> = (0..5)
            .map(|r| level.tile_at(2, (4 - r) as usize))
            .collect();

        // Locked: nothing happens.
        for _ in 0..20 {
            tick_door(&mut e, &switches, &level);
        }
        assert!(e.tile_writes.is_empty() && !e.dead, "it stays shut");

        switches.doors_opened.push(11);
        tick_door(&mut e, &switches, &level);
        let restored: Vec<u16> = e.tile_writes.iter().map(|(_, _, raw)| *raw).collect();
        assert_eq!(restored, covered, "it must put back exactly what it covered");
        assert!(e.dead, "and remove itself");
    }

    #[test]
    fn a_foot_switch_throws_on_the_fourth_blast() {
        let mut switches = SwitchState::default();
        switches.force_fields_active = true;
        let mut e = Enemy::default_for_test(EnemyKind::FootSwitch);
        e.d5 = ACT_SWITCH_FORCE_FIELD;

        for press in 1..4 {
            press_foot_switch(&mut e);
            tick_foot_switch(&mut e, &mut switches);
            assert!(
                switches.force_fields_active,
                "press {press} should not have thrown it yet"
            );
        }
        press_foot_switch(&mut e);
        tick_foot_switch(&mut e, &mut switches);
        assert!(!switches.force_fields_active, "the fourth press throws it");

        // Further blasts do nothing.
        for _ in 0..5 {
            press_foot_switch(&mut e);
            tick_foot_switch(&mut e, &mut switches);
        }
        assert_eq!(e.d1, 4, "the knob bottoms out at four");
    }

    #[test]
    fn the_mystery_wall_sleeps_until_its_switch_is_thrown() {
        let (level, data) = world(&[
            "####",
            "....",
            "....",
            "....",
            "####",
        ]);
        let mut switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::MysteryWall);
        e.x = 1;
        e.y = 3;
        e.width_tiles = 1;
        e.height_tiles = 1;

        for _ in 0..20 {
            tick_mystery_wall(&mut e, &mut switches, &level, &data);
        }
        assert_eq!(e.y, 3, "it should not move before the switch");
        assert!(e.tile_writes.is_empty());

        switches.mystery_wall_time = 4;
        for _ in 0..20 {
            if e.dead {
                break;
            }
            tick_mystery_wall(&mut e, &mut switches, &level, &data);
        }
        assert!(e.dead, "it should stop on reaching the ceiling");
        assert!(e.y < 3, "having climbed from row 3 to {}", e.y);
        assert!(
            e.tile_writes.iter().all(|(_, _, raw)| *raw == TILE_MYSTERY_BLOCK),
            "it leaves solid block behind it"
        );
    }

    #[test]
    fn a_level_with_a_switch_starts_with_that_system_off() {
        let mut s = SwitchState::default();
        let mut level = LevelJson::default();
        assert!(s.platforms_active && s.lights_active, "on by default");

        level.actors = vec![crate::data::LevelActorJson {
            map_type: (ACT_SWITCH_PLATFORMS + 31) as u16,
            x: 0,
            y: 0,
        }];
        s.reset_for_level(&level);
        assert!(!s.platforms_active, "the switch's presence disables them");
        assert!(s.lights_active, "but not the unrelated ones");
    }

    #[test]
    fn a_door_makes_itself_solid_once_and_only_once() {
        let (level, _) = world(&["........"; 12]);
        let switches = SwitchState::default();
        let mut e = Enemy::default_for_test(EnemyKind::Door);
        e.x = 4;
        e.y = 9;
        tick_door(&mut e, &switches, &level);
        assert_eq!(
            e.tile_writes,
            (0..5).map(|y| (5, 9 - y, TILE_DOOR_BLOCK)).collect::<Vec<_>>(),
            "a door is five rows of solid tile in the column beside it"
        );
        e.tile_writes.clear();
        for _ in 0..50 {
            tick_door(&mut e, &switches, &level);
        }
        assert!(
            e.tile_writes.is_empty(),
            "it must not rewrite the map every tick"
        );
    }

    #[test]
    fn the_rocket_waits_out_its_fuse_then_climbs_until_it_hits_something() {
        let (level, data) = world(&[
            "####",
            "....",
            "....",
            "....",
            "....",
            "####",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::Rocket);
        e.x = 1;
        e.y = 4;
        e.width_tiles = 1;
        e.height_tiles = 1;
        e.d1 = 60;
        e.d2 = 10;

        for _ in 0..60 {
            tick_rocket(&mut e, &Player::spawn_at(0, 0), &level, &data);
        }
        assert_eq!(e.y, 4, "it should still be on the pad during the fuse");

        for _ in 0..40 {
            if e.dead {
                break;
            }
            tick_rocket(&mut e, &Player::spawn_at(0, 0), &level, &data);
        }
        assert!(e.dead, "it should destroy itself against the ceiling");
        assert!(e.y >= 1, "and not fly through it, ending at {}", e.y);
    }

    #[test]
    fn the_ivy_plant_climbs_then_returns_after_a_blast() {
        let mut e = Enemy::default_for_test(EnemyKind::IvyPlant);
        e.y = 20;
        e.d1 = 5;
        e.d4 = 7; // seven rows to climb, as ConstructActor seeds it
        for _ in 0..40 {
            tick_ivy_plant(&mut e);
        }
        assert_eq!(e.y, 13, "it should have climbed its seven rows");

        e.d2 = 1; // blasted
        for _ in 0..10 {
            tick_ivy_plant(&mut e);
        }
        assert_eq!(e.y, 20, "and dropped all the way back");
    }

    #[test]
    fn the_worm_crate_lays_a_platform_and_carries_it_down() {
        let (level, data) = world(&[
            "........",
            "........",
            "........",
            "........",
            "########",
        ]);
        let mut e = Enemy::default_for_test(EnemyKind::WormCrate);
        e.x = 1;
        e.y = 1;
        e.width_tiles = 1;
        e.height_tiles = 1;

        tick_worm_crate(&mut e, &level, &data);
        assert_eq!(
            e.tile_writes,
            (0..4)
                .map(|i| (1 + i, -1, TILE_STRIPED_PLATFORM))
                .collect::<Vec<_>>(),
            "first tick lays a four-tile platform across its top"
        );
        e.tile_writes.clear();

        // Falling: it clears the old row and re-lays on landing.
        for _ in 0..10 {
            tick_worm_crate(&mut e, &level, &data);
        }
        assert_eq!(e.y, 3, "it should come to rest on the floor");
        let last = e.tile_writes.last().expect("it should have written tiles");
        assert_eq!(last.2, TILE_STRIPED_PLATFORM, "and re-lay on landing");
    }

    #[test]
    fn bursting_a_crate_clears_its_platform_and_frees_a_worm() {
        let mut e = Enemy::default_for_test(EnemyKind::WormCrate);
        e.x = 5;
        e.y = 9;
        burst_worm_crate(&mut e);
        assert!(e.dead);
        assert_eq!(e.spawns, vec![(ACT_PINK_WORM, 5, 9)]);
        assert!(
            e.tile_writes.iter().all(|(_, _, raw)| *raw == TILE_EMPTY),
            "the platform must be cleared, or it would hang in mid-air"
        );
        // Bursting twice must not release two worms.
        let spawned = e.spawns.len();
        burst_worm_crate(&mut e);
        assert_eq!(e.spawns.len(), spawned);
    }

    #[test]
    fn the_splitting_platform_only_opens_under_the_player() {
        let mut e = Enemy::default_for_test(EnemyKind::SplittingPlatform);
        e.x = 10;
        e.y = 10;

        let mut away = Player::spawn_at(40, 8);
        away.x = 40;
        away.y = 8;
        for _ in 0..40 {
            tick_splitting_platform(&mut e, &away);
        }
        assert_eq!(e.d1, 1, "it should stay closed with nobody on it");

        // Standing on it: the player's row is two above the actor's.
        let mut on = Player::spawn_at(11, 8);
        on.x = 11;
        on.y = 8;
        let mut cleared = false;
        for _ in 0..40 {
            tick_splitting_platform(&mut e, &on);
            cleared |= e
                .tile_writes
                .iter()
                .any(|(_, _, raw)| *raw == TILE_EMPTY);
        }
        assert!(cleared, "it should drop the player through");
        // ...and eventually re-lay itself.
        let mut relaid = false;
        for _ in 0..40 {
            tick_splitting_platform(&mut e, &away);
            relaid |= e
                .tile_writes
                .iter()
                .any(|(_, _, raw)| *raw == TILE_BLUE_PLATFORM);
        }
        assert!(relaid, "the platform must come back");
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
    fn the_spitting_turret_fires_where_it_is_aimed() {
        // The turret picks a frame bank from where the player is, and each
        // bank fires one direction: level with it means west or east,
        // below it means one of the three downward shots.
        let aim = |px: i32, py: i32| -> Vec<u16> {
            let mut e = Enemy::default_for_test(EnemyKind::SpittingTurret);
            e.x = 10;
            e.y = 10;
            e.d3 = 10;
            // Start in the rest phase (d1 == 0), where the aiming happens;
            // dropping straight into a volley would fire the bank it
            // happened to be left on.
            e.d2 = 27;
            let mut p = Player::spawn_at(px, py);
            p.x = px;
            p.y = py;
            let mut fired = Vec::new();
            for _ in 0..200 {
                tick_spitting_turret(&mut e, &p);
                fired.extend(e.spawns.drain(..).map(|(a, ..)| a));
            }
            fired.sort_unstable();
            fired.dedup();
            fired
        };

        assert_eq!(aim(2, 10), vec![ACT_PROJECTILE_W], "player west -> west");
        assert_eq!(aim(40, 10), vec![ACT_PROJECTILE_E], "player east -> east");
        assert_eq!(aim(2, 20), vec![ACT_PROJECTILE_SW], "below and west -> south-west");
        assert_eq!(aim(40, 20), vec![ACT_PROJECTILE_SE], "below and east -> south-east");
        assert_eq!(aim(11, 20), vec![ACT_PROJECTILE_S], "directly below -> south");
    }

    #[test]
    fn the_turrets_shots_clear_its_own_body() {
        // Starting a projectile inside the turret would have it collide
        // with its own launcher.
        let mut e = Enemy::default_for_test(EnemyKind::SpittingTurret);
        e.x = 10;
        e.y = 10;
        e.d3 = 10;
        e.d2 = 27;
        let mut p = Player::spawn_at(40, 10);
        p.x = 40;
        p.y = 10;
        let mut fired = Vec::new();
        for _ in 0..200 {
            tick_spitting_turret(&mut e, &p);
            fired.append(&mut e.spawns);
        }
        assert!(!fired.is_empty());
        for (act, x, _) in &fired {
            if *act == ACT_PROJECTILE_E {
                assert!(*x > e.d3 + 3, "an east shot must clear its width");
            }
        }
    }

    #[test]
    fn a_diagonal_projectile_travels_on_both_axes() {
        let scroll = crate::camera::Scroll::default();
        let mut e = Enemy::default_for_test(EnemyKind::Projectile);
        e.d5 = DIRP_SOUTHEAST;
        let (x0, y0) = (e.x, e.y);
        for _ in 0..5 {
            tick_projectile(&mut e, &scroll);
        }
        assert_eq!((e.x - x0, e.y - y0), (5, 5), "south-east means both");

        let mut s = Enemy::default_for_test(EnemyKind::Projectile);
        s.d5 = DIRP_SOUTH;
        let (sx, sy) = (s.x, s.y);
        for _ in 0..5 {
            tick_projectile(&mut s, &scroll);
        }
        assert_eq!((s.x - sx, s.y - sy), (0, 5), "south means straight down");
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
                tick_sentry_robot(&mut e, &p, &SwitchState::default(), &level, &data);
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
