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
        let rel_dir = format!("sprites/actors/{act_type}");
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
    }
}
