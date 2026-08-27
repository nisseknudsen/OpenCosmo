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
    /// The `SPR_*` this actor draws as - what `pickups::pickup_for_sprite`
    /// keys on, since the original's pickup switch is sprite-keyed too.
    pub spr: u16,
}

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

/// `ACT_*` ids (map_type - 31) that end a level on player contact
/// (actor.h: ACT_EXIT_MONSTER_W=149, ACT_EXIT_MONSTER_N=247,
/// ACT_EXIT_PLANT=186, ACT_EXIT_TRANSPORTER=203).
pub const EXIT_ACT_IDS: [u16; 4] = [149, 247, 186, 203];

const SPR_HINT_GLOBE: u16 = 125;
const SPR_EYE_PLANT: u16 = 95;

/// Cycles through `frames[sequence[step]]` on a timer - drives the hint
/// globe's two independently-animated parts (game1.c ActHintGlobe,
/// 4448-4483): a 3-frame flickering base and a slower 6-step glow pulse on
/// the orb floating above it. Two entities per globe, not a parent/child
/// pair, since each needs its own frame sequence/timing.
#[derive(Component)]
pub struct AnimatedSprite {
    pub frames: Vec<Handle<Image>>,
    pub sequence: &'static [usize],
    pub step: usize,
    pub timer: f32,
    pub interval: f32,
}

pub fn animate_sprites(time: Res<Time>, mut query: Query<(&mut AnimatedSprite, &mut Sprite)>) {
    for (mut anim, mut sprite) in &mut query {
        anim.timer += time.delta_secs();
        if anim.timer >= anim.interval {
            anim.timer = 0.0;
            anim.step = (anim.step + 1) % anim.sequence.len();
            if let Some(&frame) = anim.sequence.get(anim.step) {
                if let Some(handle) = anim.frames.get(frame) {
                    sprite.image = handle.clone();
                }
            }
        }
    }
}

const HINT_GLOBE_BASE_FRAMES: [usize; 3] = [1, 2, 3];
const HINT_GLOBE_ORB_FRAMES: [usize; 6] = [0, 4, 5, 6, 5, 4];

fn spawn_hint_globe(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &GameData,
    x: i32,
    y: i32,
    hint: u16,
) {
    let rel_dir = format!("sprites/actors/{SPR_HINT_GLOBE}");
    let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
        return;
    };
    if manifest.frames.len() < 7 {
        return; // unexpected - fall back to no render rather than a wrong one
    }
    let handles: Vec<Handle<Image>> = manifest
        .frames
        .iter()
        .map(|f| asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{}", f.file))))
        .collect();

    let base_meta = &manifest.frames[1];
    let base_h_tiles = (base_meta.height_px as f32 / 8.0).ceil();
    let base_pos = tile_topleft_to_center(
        x as f32,
        y as f32 - base_h_tiles + 1.0,
        base_meta.width_px as f32,
        base_meta.height_px as f32,
    );
    commands.spawn((
        Sprite {
            image: handles[1].clone(),
            ..default()
        },
        Transform::from_translation(base_pos.extend(5.0)),
        AnimatedSprite {
            frames: handles.clone(),
            sequence: &HINT_GLOBE_BASE_FRAMES,
            step: 0,
            timer: 0.0,
            interval: 0.15,
        },
        LevelScoped,
    ));

    let orb_meta = &manifest.frames[0];
    let orb_h_tiles = (orb_meta.height_px as f32 / 8.0).ceil();
    let orb_row = y - 2; // floats 2 tiles above the base, game1.c:4458
    let orb_pos = tile_topleft_to_center(
        x as f32,
        orb_row as f32 - orb_h_tiles + 1.0,
        orb_meta.width_px as f32,
        orb_meta.height_px as f32,
    );
    commands.spawn((
        Sprite {
            image: handles[0].clone(),
            ..default()
        },
        Transform::from_translation(orb_pos.extend(6.0)),
        AnimatedSprite {
            frames: handles,
            sequence: &HINT_GLOBE_ORB_FRAMES,
            step: 0,
            timer: 0.0,
            interval: 0.3,
        },
        // The touch test is against the *orb*, not the pedestal
        // (game1.c:4469), so the trigger lives on this entity.
        crate::hints::HintGlobe {
            x,
            y: orb_row,
            width_tiles: (orb_meta.width_px as f32 / 8.0).ceil() as i32,
            height_tiles: orb_h_tiles as i32,
            hint,
        },
        LevelScoped,
    ));
}

/// Faces left/center/right based on the player's horizontal position
/// (game1.c ActEyePlant, 3468-3488: frame = base + 0/1/2 depending on
/// whether the player is well to the left, well to the right, or roughly
/// beneath it). We skip the original's occasional random "blink" frame
/// variant (data2 == 3) for simplicity.
#[derive(Component)]
pub struct EyeTracker {
    pub x: i32,
    pub frames: Vec<Handle<Image>>,
}

pub fn track_player(
    player_q: Query<&crate::player::Player>,
    mut query: Query<(&EyeTracker, &mut Sprite)>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    for (eye, mut sprite) in &mut query {
        let frame = if eye.x - 2 > player.x {
            0 // looking left
        } else if eye.x + 1 < player.x {
            2 // looking right
        } else {
            1 // looking at the player
        };
        if let Some(handle) = eye.frames.get(frame) {
            sprite.image = handle.clone();
        }
    }
}

fn spawn_eye_plant(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &GameData,
    x: i32,
    y: i32,
) {
    let rel_dir = format!("sprites/actors/{SPR_EYE_PLANT}");
    let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
        return;
    };
    let Some(frame0) = manifest.frames.first() else {
        return;
    };
    let handles: Vec<Handle<Image>> = manifest
        .frames
        .iter()
        .map(|f| asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{}", f.file))))
        .collect();
    let h_tiles = (frame0.height_px as f32 / 8.0).ceil();
    let pos = tile_topleft_to_center(
        x as f32,
        y as f32 - h_tiles + 1.0,
        frame0.width_px as f32,
        frame0.height_px as f32,
    );
    commands.spawn((
        Sprite {
            image: handles[0].clone(),
            ..default()
        },
        Transform::from_translation(pos.extend(5.0)),
        EyeTracker { x, frames: handles },
        LevelScoped,
    ));
}

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

        if sprite_type == SPR_HINT_GLOBE {
            // Every ACT_HINT_GLOBE_* draws the same sprite; which message
            // it holds is carried by the actor id alone.
            let hint = crate::hints::hint_number_for_actor(act_type).unwrap_or(0);
            spawn_hint_globe(commands, asset_server, data, a.x as i32, a.y as i32, hint);
            continue;
        }
        if sprite_type == SPR_EYE_PLANT {
            spawn_eye_plant(commands, asset_server, data, a.x as i32, a.y as i32);
            continue;
        }

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
                image: asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{file}"))),
                ..default()
            },
            Transform::from_translation(pos.extend(5.0)),
            StaticActor,
            LevelScoped,
        ));

        // Actors with a ported ActXxx() behavior drive their own position
        // and frame from here on (crate::enemy_ai). They keep whatever
        // Hazard/Collectible markers apply, but must not also be given the
        // generic `Walker`, or two systems would fight over their position.
        let behavior = crate::enemy_ai::behavior_for(act_type);
        if let Some((kind, init)) = behavior {
            let frames: Vec<Handle<Image>> = manifest
                .frames
                .iter()
                .map(|f| asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{}", f.file))))
                .collect();
            entity.insert((
                crate::enemy_ai::Enemy::new(
                    kind,
                    init,
                    a.x as i32,
                    a.y as i32,
                    (*width_px as f32 / 8.0).ceil() as i32,
                    height_tiles as i32,
                    frames,
                ),
                crate::motion::PrevPos { x: a.x as i32, y: a.y as i32 },
            ));
        }
        if EXIT_ACT_IDS.contains(&act_type) {
            entity.insert(ExitTrigger {
                x: a.x as i32,
                y: a.y as i32,
            });
        }
        if crate::pickups::pickup_for_sprite(sprite_type).is_some() {
            entity.insert(Collectible {
                x: a.x as i32,
                y: a.y as i32,
                spr: sprite_type,
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
        if behavior.is_none() && crate::enemy::WALKER_ACT_IDS.contains(&act_type) {
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
