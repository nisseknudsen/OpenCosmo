//! Per-actor activation and gravity flags, extracted from every
//! `ConstructActor(...)` call in `NewActorAtIndex` (game1.c:5618-6371).
//!
//! `ConstructActor(sprite, x, y, force_active, stay_active, weighted,
//! acrophile, tickfunc, data1..data5)`. The four booleans are not
//! incidental bookkeeping - between them they define a mechanic:
//!
//! - **weighted** actors get a shared gravity pass before their own tick
//!   (game1.c:7868-7897), so they fall.
//! - **force_active** actors tick even while off screen.
//! - **stay_active** actors are dormant until seen once, then tick forever
//!   (`ProcessActor`, game1.c:7858-7864).
//! - **acrophile** actors will walk off a ledge; the rest turn around.
//!
//! `stay_active + weighted` is what makes prizes perched out of view fall
//! once you look up at them: `ACT_STAR` and `ACT_POWER_UP` are both
//! `force=false, stay=true, weighted=true`, while their `*_FLOAT` twins are
//! all-false and hang in the air forever. Two variants of the same prize,
//! distinguished only by these flags.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActorFlags {
    pub force_active: bool,
    pub stay_active: bool,
    pub weighted: bool,
    pub acrophile: bool,
}

/// Indexed by `ACT_*` id (`map_type - 31`).
pub const ACT_FLAGS: &[(u16, ActorFlags)] = &[
    (0, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_NULL
    (1, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_STAR_FLOAT
    (2, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_JUMP_PAD_FLOOR
    (3, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_ARROW_PISTON_W
    (4, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_ARROW_PISTON_E
    (5, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_FIREBALL_W
    (6, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_FIREBALL_E
    (7, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEAD_SWITCH_BLUE
    (8, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEAD_SWITCH_RED
    (9, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEAD_SWITCH_GREEN
    (10, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEAD_SWITCH_YELLOW
    (11, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_DOOR_BLUE
    (12, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_DOOR_RED
    (13, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_DOOR_GREEN
    (14, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_DOOR_YELLOW
    (16, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_JUMP_PAD_ROBOT
    (17, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_FLOOR
    (18, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_FLOOR_RECIP
    (20, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: true }), // ACT_SAW_BLADE_VERT
    (22, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: true }), // ACT_SAW_BLADE_HORIZ
    (24, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: true }), // ACT_BOMB_ARMED
    (25, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: true }), // ACT_CABBAGE
    (28, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_POWER_UP_FLOAT
    (29, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_POWER_UP
    (31, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_GRN_TOMATO
    (32, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_GRN_TOMATO
    (33, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_RED_TOMATO
    (34, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_RED_TOMATO
    (35, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_YEL_PEAR
    (36, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_YEL_PEAR
    (37, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_ONION
    (38, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_ONION
    (39, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_EXIT_SIGN
    (40, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEAR
    (41, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEAR_RECIP
    (42, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_GRN_SLIME_THROB
    (43, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_GRN_SLIME_DRIP
    (44, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_FLYING_WISP
    (45, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_TWO_TONS_CRUSHER
    (46, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_JUMPING_BULLET
    (47, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_STONE_HEAD_CRUSHER
    (48, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_PYRAMID_CEIL
    (49, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: true }), // ACT_PYRAMID_FALLING
    (50, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_PYRAMID_FLOOR
    (51, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_GHOST
    (52, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_GRN_GOURD
    (53, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_BLU_SPHERES
    (54, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: true }), // ACT_MOON
    (55, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEART_PLANT
    (56, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_BOMB
    (57, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BOMB_IDLE
    (58, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: true }), // ACT_BARREL_JUMP_PAD_FL
    (59, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SWITCH_PLATFORMS
    (61, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SWITCH_MYSTERY_WALL
    (62, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_MYSTERY_WALL
    (63, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_FLOOR_BENT
    (64, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_MONUMENT
    (65, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_BABY_GHOST
    (66, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: true }), // ACT_PROJECTILE_SW
    (67, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: true }), // ACT_PROJECTILE_SE
    (68, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: true }), // ACT_PROJECTILE_S
    (69, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_ROAMER_SLUG
    (70, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_PIPE_CORNER_N
    (71, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_PIPE_CORNER_S
    (72, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_PIPE_CORNER_W
    (73, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_PIPE_CORNER_E
    (74, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BABY_GHOST_EGG_PROX
    (75, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BABY_GHOST_EGG
    (78, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SHARP_ROBOT_FLOOR
    (80, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SHARP_ROBOT_CEIL
    (81, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_HAMBURGER
    (82, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_HAMBURGER
    (83, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_CLAM_PLANT_FLOOR
    (84, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_CLAM_PLANT_CEIL
    (85, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_GRAPES
    (86, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: true }), // ACT_PARACHUTE_BALL
    (87, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_E
    (88, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_E_RECIP
    (89, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIKES_W
    (90, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_BEAM_ROBOT
    (91, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_SPLITTING_PLATFORM
    (92, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SPARK
    (93, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_DANCE_MUSH
    (94, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_DANCING_MUSHROOM
    (95, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_EYE_PLANT_FLOOR
    (96, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_EYE_PLANT_CEIL
    (100, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_CABB_HARDER
    (101, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_RED_JUMPER
    (102, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_BOSS
    (104, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PIPE_OUTLET
    (105, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_PIPE_INLET
    (106, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SUCTION_WALKER
    (107, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_TRANSPORTER_1
    (108, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_TRANSPORTER_2
    (109, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PROJECTILE_W
    (110, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PROJECTILE_E
    (111, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIT_WALL_PLANT_E
    (112, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SPIT_WALL_PLANT_W
    (113, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SPITTING_TURRET
    (114, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCOOTER
    (115, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_PEA_PILE
    (116, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_BASKET_LUMPY_FRUIT
    (117, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_HORN
    (118, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_RED_CHOMPER
    (119, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_POD
    (120, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SWITCH_LIGHTS
    (121, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SWITCH_FORCE_FIELD
    (122, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_FORCE_FIELD_VERT
    (123, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_FORCE_FIELD_HORIZ
    (124, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_PINK_WORM
    (125, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_0
    (126, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_PUSHER_ROBOT
    (127, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SENTRY_ROBOT
    (128, ActorFlags { force_active: false, stay_active: false, weighted: true, acrophile: false }), // ACT_PINK_WORM_SLIME
    (129, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_DRAGONFLY
    (130, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_WORM_CRATE
    (134, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BOTTLE_DRINK
    (135, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_GRN_GOURD
    (136, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BLU_SPHERES
    (137, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_POD
    (138, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_PEA_PILE
    (139, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_LUMPY_FRUIT
    (140, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_HORN
    (141, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_BERRIES
    (142, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_BOTL_DRINK
    (143, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SATELLITE
    (145, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_IVY_PLANT
    (146, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_YEL_FRUIT_VINE
    (147, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_HEADDRESS
    (148, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_HEADDRESS
    (149, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_EXIT_MONSTER_W
    (150, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_EXIT_LINE_VERT
    (151, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SMALL_FLAME
    (152, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_TULIP_LAUNCHER
    (153, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_ROTATING_ORNAMENT
    (154, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BLU_CRYSTAL
    (155, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_RED_CRYSTAL_FLOOR
    (156, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_RT_ORNAMENT
    (157, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_BLU_CRYSTAL
    (158, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_RED_CRYSTAL
    (159, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_GRN_TOMATO_FLOAT
    (160, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_TOMATO_FLOAT
    (161, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_YEL_PEAR_FLOAT
    (162, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BEAR_TRAP
    (163, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_FALLING_FLOOR
    (164, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_EP1_END_1
    (167, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_ROOT
    (168, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_ROOT
    (169, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_RG_BERRIES
    (170, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_REDGRN_BERRIES
    (171, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_RED_GOURD
    (172, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_RED_GOURD
    (173, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_GRN_EMERALD
    (174, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_GRN_EMERALD
    (175, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_CLR_DIAMOND
    (176, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_CLR_DIAMOND
    (177, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_100
    (178, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_200
    (179, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_400
    (180, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_800
    (181, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_1600
    (182, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_3200
    (183, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_6400
    (184, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_SCORE_EFFECT_12800
    (186, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_EXIT_PLANT
    (187, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_BIRD
    (188, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_ROCKET
    (189, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_INVINCIBILITY_CUBE
    (190, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PEDESTAL_SMALL
    (191, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PEDESTAL_MEDIUM
    (192, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_PEDESTAL_LARGE
    (193, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_CYA_DIAMOND
    (194, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_CYA_DIAMOND
    (195, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_RED_DIAMOND
    (196, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_RED_DIAMOND
    (197, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_GRY_OCTAHED
    (198, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_GRY_OCTAHEDRON
    (199, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_BLU_EMERALD
    (200, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BLU_EMERALD
    (201, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_INVINCIBILITY_BUBB
    (202, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_THRUSTER_JET
    (203, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_EXIT_TRANSPORTER
    (204, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_1
    (205, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_2
    (206, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_3
    (207, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_4
    (208, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_5
    (209, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_6
    (210, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_7
    (211, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_8
    (212, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_9
    (213, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_CYA_DIAMOND_FLOAT
    (214, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_DIAMOND_FLOAT
    (215, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_GRY_OCTAHED_FLOAT
    (216, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BLU_EMERALD_FLOAT
    (217, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_JUMP_PAD_CEIL
    (218, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BARREL_HEADPHONES
    (219, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HEADPHONES_FLOAT
    (220, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_HEADPHONES
    (221, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_FROZEN_DN
    (223, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BANANAS
    (224, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_RED_LEAFY
    (225, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_LEAFY_FLOAT
    (226, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_RED_LEAFY
    (227, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_BRN_PEAR
    (228, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_BRN_PEAR_FLOAT
    (229, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BRN_PEAR
    (230, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_BASKET_CANDY_CORN
    (231, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_CANDY_CORN_FLOAT
    (232, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: false }), // ACT_CANDY_CORN
    (233, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_FLAME_PULSE_W
    (234, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_FLAME_PULSE_E
    (235, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEECH_OUCH
    (236, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_SLIME_THROB
    (237, ActorFlags { force_active: false, stay_active: true, weighted: false, acrophile: false }), // ACT_RED_SLIME_DRIP
    (238, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_10
    (239, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_11
    (240, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_12
    (241, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_13
    (242, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_14
    (243, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_15
    (244, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEECH_WHOA
    (245, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEECH_UMPH
    (246, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_SPEECH_WOW_50K
    (247, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_EXIT_MONSTER_N
    (248, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SMOKE_EMIT_SMALL
    (249, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_SMOKE_EMIT_LARGE
    (250, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_EXIT_LINE_HORIZ
    (251, ActorFlags { force_active: true, stay_active: false, weighted: true, acrophile: true }), // ACT_CABBAGE_HARDER
    (252, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_RED_CRYSTAL_CEIL
    (253, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_16
    (254, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_17
    (255, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_18
    (256, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_19
    (257, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_20
    (258, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_21
    (259, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_22
    (260, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_23
    (261, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_24
    (262, ActorFlags { force_active: false, stay_active: false, weighted: false, acrophile: false }), // ACT_HINT_GLOBE_25
    (263, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_POWER_UP
    (264, ActorFlags { force_active: false, stay_active: true, weighted: true, acrophile: false }), // ACT_STAR
    (265, ActorFlags { force_active: true, stay_active: false, weighted: false, acrophile: false }), // ACT_EP2_END_LINE
];

/// Where an actor is placed relative to the tile the map names, extracted
/// from the `x`/`y` arguments of the same `ConstructActor` calls.
///
/// 29 of them are offset, and honouring only the couple that were noticed
/// by eye left the rest sitting one to seven tiles away from where the
/// level author put them - grapes, vines, bananas and berries hanging in
/// the wrong place, and an ivy plant off by seven.
pub const ACT_SPAWN_OFFSET: &[(u16, i32, i32)] = &[
    (4, -4, 0), // ACT_ARROW_PISTON_E
    (6, -1, 0), // ACT_FIREBALL_E
    (7, 0, 1), // ACT_HEAD_SWITCH_BLUE
    (8, 0, 1), // ACT_HEAD_SWITCH_RED
    (9, 0, 1), // ACT_HEAD_SWITCH_GREEN
    (10, 0, 1), // ACT_HEAD_SWITCH_YELLOW
    (42, 0, 1), // ACT_GRN_SLIME_THROB
    (43, 0, 1), // ACT_GRN_SLIME_DRIP
    (48, 0, 1), // ACT_PYRAMID_CEIL
    (49, 0, 1), // ACT_PYRAMID_FALLING
    (80, 0, 2), // ACT_SHARP_ROBOT_CEIL
    (84, 0, 2), // ACT_CLAM_PLANT_CEIL
    (85, 0, 2), // ACT_GRAPES
    (89, -3, 0), // ACT_SPIKES_W
    (96, 0, 1), // ACT_EYE_PLANT_CEIL
    (104, -1, 2), // ACT_PIPE_OUTLET
    (105, -1, 2), // ACT_PIPE_INLET
    (112, -3, 0), // ACT_SPIT_WALL_PLANT_W
    (141, 0, 2), // ACT_RED_BERRIES
    (145, 0, 7), // ACT_IVY_PLANT
    (146, 0, 2), // ACT_YEL_FRUIT_VINE
    (149, -4, 0), // ACT_EXIT_MONSTER_W
    (202, 0, 2), // ACT_THRUSTER_JET
    (223, 0, 1), // ACT_BANANAS
    (233, -1, 0), // ACT_FLAME_PULSE_W
    (236, 0, 1), // ACT_RED_SLIME_THROB
    (237, 0, 1), // ACT_RED_SLIME_DRIP
    (252, 0, 1), // ACT_RED_CRYSTAL_CEIL
    (265, 0, 3), // ACT_EP2_END_LINE
];

/// `(dx, dy)` in tiles for an actor id; `(0, 0)` for anything unlisted.
pub fn spawn_offset(act_id: u16) -> (i32, i32) {
    ACT_SPAWN_OFFSET
        .iter()
        .find(|(id, ..)| *id == act_id)
        .map(|(_, dx, dy)| (*dx, *dy))
        .unwrap_or((0, 0))
}

/// Flags for an actor id. Anything absent from the table is inert scenery
/// as far as these four are concerned.
pub fn flags_for(act_id: u16) -> ActorFlags {
    ACT_FLAGS
        .iter()
        .find(|(id, _)| *id == act_id)
        .map(|(_, f)| *f)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_covers_the_actors_the_game_actually_builds() {
        assert!(ACT_FLAGS.len() > 200, "only {} entries", ACT_FLAGS.len());
    }

    #[test]
    fn ids_are_unique_and_sorted() {
        let mut last = None;
        for (id, _) in ACT_FLAGS {
            assert!(Some(*id) > last, "id {id} out of order or duplicated");
            last = Some(*id);
        }
    }

    #[test]
    fn a_perched_prize_wakes_and_falls_but_its_floating_twin_does_not() {
        // The mechanic this table exists for.
        for act in [264u16 /* ACT_STAR */, 263 /* ACT_POWER_UP */] {
            let f = flags_for(act);
            assert!(!f.force_active, "{act} should start dormant");
            assert!(f.stay_active, "{act} should wake permanently once seen");
            assert!(f.weighted, "{act} should fall once awake");
        }
        // ACT_STAR_FLOAT hangs where it was placed.
        assert_eq!(flags_for(1), ActorFlags::default());
    }

    #[test]
    fn barrels_are_weighted_so_they_sit_on_the_ground() {
        assert!(flags_for(29).weighted, "ACT_BARREL_POWER_UP");
        assert!(flags_for(56).weighted, "ACT_BARREL_BOMB");
    }

    #[test]
    fn spawn_offsets_match_the_source() {
        // A few spot checks against the ConstructActor calls they came from.
        assert_eq!(spawn_offset(202), (0, 2), "ACT_THRUSTER_JET, game1.c:6166");
        assert_eq!(spawn_offset(145), (0, 7), "ACT_IVY_PLANT");
        assert_eq!(spawn_offset(149), (-4, 0), "ACT_EXIT_MONSTER_W, game1.c:6017");
        assert_eq!(spawn_offset(233), (-1, 0), "ACT_FLAME_PULSE_W");
        assert_eq!(spawn_offset(49), (0, 1), "ACT_PYRAMID_FALLING");
    }

    #[test]
    fn an_actor_with_no_offset_sits_where_the_map_says() {
        assert_eq!(spawn_offset(264), (0, 0), "ACT_STAR");
        assert_eq!(spawn_offset(118), (0, 0), "ACT_RED_CHOMPER");
    }

    #[test]
    fn an_unknown_id_is_inert_rather_than_a_panic() {
        assert_eq!(flags_for(9999), ActorFlags::default());
    }
}
