//! Static actor rendering: spawns a frame-0 sprite for every map actor with
//! `map_type >= 31` (the `ACT_*` range; `NewMapActorAtIndex()` in game1.c
//! confirms `ACT_* = map_type - 31`) at its authored tile position, using
//! the same bottom-row/left-column sprite origin convention as the player.
//! No AI/animation/collision yet - this establishes the visual cast so
//! levels don't read as empty, to be made interactive incrementally.

use crate::data::{FrameMetaJson, GameData, LevelJson};
use crate::level::{tile_topleft_to_center, LevelScoped};
use bevy::prelude::*;

#[derive(Component)]
pub struct StaticActor;

#[derive(Component)]
pub struct ExitTrigger {
    pub x: i32,
    pub y: i32,
}

#[derive(Component)]
pub struct Collectible {
    pub x: i32,
    pub y: i32,
    pub act_id: u16,
}

/// ACT_STAR_FLOAT=1, ACT_STAR=264 - the original tracks these in a
/// separate "Stars" status-bar counter (`gameStars`, game2.c:1249),
/// distinct from Score.
pub const STAR_ACT_IDS: [u16; 2] = [1, 264];

/// A BASKET_*/BARREL_* container - breaks when the player lands on top of
/// it, revealing/granting whatever `contents` (an ACT_* id) it holds. See
/// `actor_sprite_map::CONTAINER_CONTENTS`, extracted from every
/// `ConstructActor(..., ActBarrel, ACT_CONTENTS, ...)` call.
#[derive(Component)]
pub struct Container {
    pub x: i32,
    pub y: i32,
    pub contents: u16,
}

/// `ACT_*` ids for the plain food/gem pickups (excludes their BASKET_*/
/// BARREL_* container variants and animated slime hazards). From actor.h:
/// ACT_STAR_FLOAT=1, ACT_GRN_TOMATO=32, ACT_RED_TOMATO=34, ACT_GRN_GOURD=135,
/// ACT_POD=137, ACT_RED_BERRIES=141, ACT_BLU_CRYSTAL=154,
/// ACT_RED_CRYSTAL_FLOOR=155, ACT_GRN_TOMATO_FLOAT=159,
/// ACT_RED_TOMATO_FLOAT=160, ACT_REDGRN_BERRIES=170, ACT_RED_GOURD=172,
/// ACT_CLR_DIAMOND=176, ACT_CYA_DIAMOND=194, ACT_RED_DIAMOND=196,
/// ACT_CYA_DIAMOND_FLOAT=213, ACT_RED_DIAMOND_FLOAT=214,
/// ACT_RED_LEAFY_FLOAT=225, ACT_RED_LEAFY=226, ACT_RED_CRYSTAL_CEIL=252,
/// ACT_STAR=264.
pub const COLLECTIBLE_ACT_IDS: [u16; 21] = [
    1, 32, 34, 135, 137, 141, 154, 155, 159, 160, 170, 172, 176, 194, 196, 213, 214, 225, 226,
    252, 264,
];

/// `ACT_*` ids (map_type - 31) that end a level on player contact
/// (actor.h: ACT_EXIT_MONSTER_W=149, ACT_EXIT_MONSTER_N=247,
/// ACT_EXIT_PLANT=186, ACT_EXIT_TRANSPORTER=203).
pub const EXIT_ACT_IDS: [u16; 4] = [149, 247, 186, 203];

pub fn spawn_level_actors(
    commands: &mut Commands,
    asset_server: &AssetServer,
    level: &LevelJson,
    data: &GameData,
) {
    for a in &level.actors {
        if a.map_type < 31 {
            continue; // SPA_* special actor (player start, platform, light, fountain)
        }
        let act_type = a.map_type - 31;
        // ACT_* (map actor type) and SPR_* (the graphic actually drawn)
        // are different numbering spaces - see actor_sprite_map.rs. Sprite
        // folders on disk are keyed by SPR id, produced the same way by
        // cosmo-assets::convert::convert_episode1.
        let sprite_type = cosmo_assets::actor_sprite_map::ACT_TO_SPRITE
            .iter()
            .find(|(id, ..)| *id == act_type)
            .map(|(_, spr, ..)| *spr)
            .unwrap_or(act_type);
        let rel_dir = format!("sprites/actors/{sprite_type}");
        let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
            continue;
        };
        let Some(frame0) = manifest.frames.first() else {
            continue;
        };
        let FrameMetaJson {
            file,
            width_px,
            height_px,
        } = frame0;
        let height_tiles = (*height_px as f32 / 8.0).ceil();
        let top_row = a.y as f32 - height_tiles + 1.0;
        let pos = tile_topleft_to_center(a.x as f32, top_row, *width_px as f32, *height_px as f32);
        let mut entity = commands.spawn((
            Sprite {
                image: asset_server.load(format!("generated/{rel_dir}/{file}")),
                ..default()
            },
            Transform::from_translation(pos.extend(5.0)),
            StaticActor,
            LevelScoped,
        ));
        if EXIT_ACT_IDS.contains(&act_type) {
            entity.insert(ExitTrigger {
                x: a.x as i32,
                y: a.y as i32,
            });
        }
        if COLLECTIBLE_ACT_IDS.contains(&act_type) {
            entity.insert(Collectible {
                x: a.x as i32,
                y: a.y as i32,
                act_id: act_type,
            });
        }
        if crate::enemy::HAZARD_ACT_IDS.contains(&act_type) {
            entity.insert(crate::enemy::Hazard);
        }
        if let Some(contents) = cosmo_assets::actor_sprite_map::container_contents(act_type) {
            if contents != act_type {
                // ACT_BASKET_NULL "contains itself" - the game's encoding
                // for an empty basket; nothing to break out of it.
                entity.insert(Container {
                    x: a.x as i32,
                    y: a.y as i32,
                    contents,
                });
            }
        }
        if crate::enemy::WALKER_ACT_IDS.contains(&act_type) {
            let width_tiles = (*width_px as f32 / 8.0).ceil() as i32;
            entity.insert(crate::enemy::Walker {
                x: a.x as i32,
                y: a.y as i32,
                dir: 1,
                width_tiles,
                height_tiles: height_tiles as i32,
            });
        }
    }
}
