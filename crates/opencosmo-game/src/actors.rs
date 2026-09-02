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

/// The exit sign, which ends the level when the player touches it
/// (game1.c:7386 gates the switch at 7551 on contact).
#[derive(Component)]
pub struct ExitSign {
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

/// `ACT_*` ids (map_type - 31) that end the level.
///
/// The important one is `ACT_EXIT_SIGN` (39), which this list used to be
/// missing entirely - and it is the *only* exit in episode 1's first level,
/// so there was no way to finish it. Its tick function is `ActFootSwitch`,
/// which does nothing at all; the level actually ends from the sprite-keyed
/// interaction switch (`case SPR_EXIT_SIGN: winLevel = true`,
/// game1.c:7551-7553), which is easy to miss when reading the actor table.
///
/// That case has no proximity test either - reaching it inside
/// `InteractPlayer` already means the actor passed `IsSpriteVisible`
/// (game1.c:7071), so the sign ends the level once it is on screen. In
/// practice that is the same thing: a1 puts it at x=508 of a 512-wide map,
/// which the view can only reach at the very end.
///
/// The others (ACT_EXIT_MONSTER_W=149, ACT_EXIT_PLANT=186,
/// ACT_EXIT_TRANSPORTER=203) genuinely need touching - they play a
/// swallow/transport animation first, which is not ported, so they are
/// treated as contact triggers. ACT_EXIT_MONSTER_N (247) is *not* here: it
/// is `ActFootSwitch` too, and its own exit case is compiled out of episode
/// 1 entirely (`#ifdef HAS_ACT_EXIT_MONSTER_N`, game1.c:7819).
pub const EXIT_ACT_IDS: [u16; 3] = [149, 186, 203];

/// `ACT_EXIT_SIGN` - ends the level on becoming visible, not on contact.
pub const ACT_EXIT_SIGN: u16 = 39;

const SPR_HINT_GLOBE: u16 = 125;
const SPR_PEDESTAL: u16 = 192;

/// `ACT_PEDESTAL_SMALL/MEDIUM/LARGE` and the column height each one passes
/// to `ActPedestal` as `data1` (game1.c:6129-6137).
const PEDESTALS: [(u16, i32); 3] = [(190, 13), (191, 19), (192, 25)];

/// The tile a pedestal's cap writes into the map: solid, and never drawn.
/// `TILE_INVISIBLE_PLATFORM` (graphics.h:121).
pub const TILE_INVISIBLE_PLATFORM: u16 = 0x0048;

/// Stamps every pedestal's cap into the map as solid floor.
///
/// `ActPedestal` does this each frame with `SetMapTileRepeat`
/// (game1.c:5279). Pedestals never move, so doing it once as the level
/// loads is equivalent and needs no per-tick map mutation - which the rest
/// of this port does not have.
pub fn apply_pedestal_platforms(level: &mut LevelJson) {
    for a in level.actors.clone() {
        if a.map_type < 31 {
            continue;
        }
        let Some((_, height)) = PEDESTALS.iter().find(|(id, _)| *id == a.map_type - 31) else {
            continue;
        };
        let cap_row = a.y as i32 - height;
        if cap_row < 0 {
            continue;
        }
        for i in 0..5 {
            let x = a.x as i32 - 2 + i;
            if x < 0 || x >= level.width as i32 {
                continue;
            }
            let idx = cap_row as usize * level.width + x as usize;
            if let Some(cell) = level.tiles.get_mut(idx) {
                *cell = TILE_INVISIBLE_PLATFORM;
            }
        }
    }
}

/// A standing pedestal, so it can be knocked down.
///
/// It is inert until something explodes against its base, and then it
/// sinks a tile at a time, shedding a shard each step, until it collapses
/// entirely (game1.c:5281-5303). That is the only thing it ever does, and
/// leaving it out made a pillar you could climb but never destroy.
#[derive(Component)]
pub struct Pedestal {
    pub x: i32,
    pub base_y: i32,
    pub height: i32,
    /// Counts 3, 2, 1 between steps, as `data2` does.
    pub collapse: u32,
    pub stalk: Vec<Entity>,
    pub cap: Entity,
}

/// A pedestal: a stalk `height` tiles tall with a five-wide cap on top.
///
/// The actor itself is never drawn (`nextDrawMode = DRAW_MODE_HIDDEN`,
/// game1.c:5272); it draws a column of frame 1 upward from its own tile and
/// then frame 0 across the top. Rendering it as a single frame-0 sprite at
/// the actor's own position - which is what this did - put the cap on the
/// ground with no column under it and nothing to stand on, which is the
/// "orange bar that does nothing".
fn spawn_pedestal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &GameData,
    x: i32,
    y: i32,
    height: i32,
) {
    let rel_dir = format!("sprites/actors/{SPR_PEDESTAL}");
    let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
        return;
    };
    if manifest.frames.len() < 2 {
        return;
    }
    let load = |i: usize| {
        asset_server.load(crate::data::asset_path(&format!(
            "{rel_dir}/{}",
            manifest.frames[i].file
        )))
    };
    let (cap, stalk) = (load(0), load(1));

    let mut segments = Vec::with_capacity(height as usize);
    for i in 0..height {
        let pos = tile_topleft_to_center(x as f32, (y - i) as f32, 8.0, 8.0);
        segments.push(
            commands
                .spawn((
                    Sprite {
                        image: stalk.clone(),
                        ..default()
                    },
                    Transform::from_translation(pos.extend(4.0)),
                    LevelScoped,
                ))
                .id(),
        );
    }
    let cap_meta = &manifest.frames[0];
    let pos = tile_topleft_to_center(
        (x - 2) as f32,
        (y - height) as f32,
        cap_meta.width_px as f32,
        cap_meta.height_px as f32,
    );
    let cap_entity = commands
        .spawn((
            Sprite {
                image: cap,
                ..default()
            },
            Transform::from_translation(pos.extend(4.0)),
            LevelScoped,
        ))
        .id();
    commands.spawn((
        Pedestal {
            x,
            base_y: y,
            height,
            collapse: 0,
            stalk: segments,
            cap: cap_entity,
        },
        LevelScoped,
    ));
}

/// Writes a pedestal's cap row into the map, or clears it.
fn set_cap_tiles(level: &mut LevelJson, x: i32, row: i32, value: u16) {
    if row < 0 || row >= level.height as i32 {
        return;
    }
    for i in 0..5 {
        let cx = x - 2 + i;
        if cx < 0 || cx >= level.width as i32 {
            continue;
        }
        let idx = row as usize * level.width + cx as usize;
        if let Some(cell) = level.tiles.get_mut(idx) {
            *cell = value;
        }
    }
}

/// `ActPedestal`'s collapse (game1.c:5281-5303): a blast against the base
/// starts it sinking, one tile every three ticks, until nothing is left.
pub fn collapse_pedestals(
    mut commands: Commands,
    mut level: ResMut<crate::level::CurrentLevel>,
    effects: Res<crate::effects::EffectAssets>,
    explosions: Query<&crate::effects::Explosion>,
    mut pedestals: Query<(Entity, &mut Pedestal)>,
) {
    let (blast_w, blast_h) = crate::combat::blast_size(&effects);
    for (entity, mut ped) in &mut pedestals {
        if ped.collapse == 0 {
            let hit = explosions.iter().any(|e| {
                crate::combat::rects_overlap(
                    e.x, e.y, blast_w, blast_h, ped.x, ped.base_y, 1, 1,
                )
            });
            if hit {
                ped.collapse = 3;
            }
            continue;
        }
        if ped.collapse > 1 {
            ped.collapse -= 1;
            continue;
        }
        // A step down: lose the top segment, drop the cap onto it.
        ped.collapse = 3;
        let old_cap_row = ped.base_y - ped.height;
        set_cap_tiles(&mut level.level, ped.x, old_cap_row, 0);
        if let Some(top) = ped.stalk.pop() {
            commands.entity(top).try_despawn();
        }
        ped.height -= 1;
        crate::effects::spawn_pounce_debris(&mut commands, &effects, ped.x, ped.base_y);

        if ped.height <= 1 {
            commands.entity(ped.cap).try_despawn();
            for e in ped.stalk.drain(..) {
                commands.entity(e).try_despawn();
            }
            commands.entity(entity).try_despawn();
            continue;
        }
        let row = ped.base_y - ped.height;
        set_cap_tiles(&mut level.level, ped.x, row, TILE_INVISIBLE_PLATFORM);
        if let Ok(mut t) = commands.get_entity(ped.cap) {
            let pos = tile_topleft_to_center((ped.x - 2) as f32, row as f32, 40.0, 8.0);
            t.insert(Transform::from_translation(pos.extend(4.0)));
        }
    }
}
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

/// Keeps the interaction markers on an actor in step with where it has
/// actually moved to.
///
/// `Collectible`, `Container` and `ExitTrigger` each carry their own copy of
/// a tile position, which was fine while every actor stayed where it was
/// placed. Actors that move - and now that the `weighted` flag is honoured,
/// that includes any prize that falls when you look up at it - would
/// otherwise still be picked up at the spot they were *authored*, and not
/// where they landed.
pub fn sync_actor_positions(
    mut collectibles: Query<(&crate::enemy_ai::Enemy, &mut Collectible)>,
    mut containers: Query<(&crate::enemy_ai::Enemy, &mut Container)>,
) {
    for (enemy, mut c) in &mut collectibles {
        c.x = enemy.x;
        c.y = enemy.y;
    }
    for (enemy, mut c) in &mut containers {
        c.x = enemy.x;
        c.y = enemy.y;
    }
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
        // opencosmo-assets::convert::convert_episode1.
        let sprite_type = opencosmo_assets::actor_sprite_map::ACT_TO_SPRITE
            .iter()
            .find(|(id, ..)| *id == act_type)
            .map(|(_, spr, ..)| *spr)
            .unwrap_or(act_type);

        // `ConstructActor` places 29 of the actor types at an offset from
        // the tile the map names - see `actor_flags::ACT_SPAWN_OFFSET`.
        let (dx, dy) = opencosmo_assets::actor_flags::spawn_offset(act_type);
        let (ax, ay) = (a.x as i32 + dx, a.y as i32 + dy);

        if sprite_type == SPR_HINT_GLOBE {
            // Every ACT_HINT_GLOBE_* draws the same sprite; which message
            // it holds is carried by the actor id alone.
            let hint = crate::hints::hint_number_for_actor(act_type).unwrap_or(0);
            spawn_hint_globe(commands, asset_server, data, ax, ay, hint);
            continue;
        }
        if let Some((_, height)) = PEDESTALS.iter().find(|(id, _)| *id == act_type) {
            spawn_pedestal(commands, asset_server, data, ax, ay, *height);
            continue;
        }
        if sprite_type == SPR_EYE_PLANT {
            spawn_eye_plant(commands, asset_server, data, ax, ay);
            continue;
        }

        spawn_one_actor(commands, asset_server, data, act_type, ax, ay);
    }
}

/// Spawns one actor entity: artwork, behavior, and whatever gameplay
/// markers its id earns. Shared by the level loader and by actors that
/// spawn other actors at runtime (`NewActor` in the original), so a
/// projectile fired mid-level is put together exactly like one placed by
/// the map.
pub fn spawn_one_actor(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &GameData,
    act_type: u16,
    ax: i32,
    ay: i32,
) {
    let sprite_type = opencosmo_assets::actor_sprite_map::ACT_TO_SPRITE
        .iter()
        .find(|(id, ..)| *id == act_type)
        .map(|(_, spr, ..)| *spr)
        .unwrap_or(act_type);
    let rel_dir = format!("sprites/actors/{sprite_type}");
    let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
        return;
    };
    let Some(frame0) = manifest.frames.first() else {
        return;
    };
    let FrameMetaJson {
        file,
        width_px,
        height_px,
    } = frame0;
    let height_tiles = (*height_px as f32 / 8.0).ceil();
    let top_row = ay as f32 - height_tiles + 1.0;
    let pos = tile_topleft_to_center(ax as f32, top_row, *width_px as f32, *height_px as f32);
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
    // An actor with no behavior of its own still falls if it is flagged
    // weighted: the original applies gravity in ProcessActor before it
    // reaches the tick function at all (game1.c:7868). Barrels, baskets,
    // loose bombs and dropped prizes have no behavior, so nothing ran that
    // pass on them and they hung wherever the map put them.
    let act_flags = opencosmo_assets::actor_flags::flags_for(act_type);
    let behavior = crate::enemy_ai::behavior_for(act_type).or_else(|| {
        act_flags
            .weighted
            .then_some((crate::enemy_ai::EnemyKind::Inert, [0; 5]))
    });
    if let Some((kind, init)) = behavior {
        let frames: Vec<Handle<Image>> = manifest
            .frames
            .iter()
            .map(|f| asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{}", f.file))))
            .collect();
        entity.insert((
            crate::enemy_ai::Enemy::new(
                kind,
                act_type,
                init,
                ax,
                ay,
                (*width_px as f32 / 8.0).ceil() as i32,
                height_tiles as i32,
                frames,
                opencosmo_assets::actor_flags::flags_for(act_type),
            ),
            crate::motion::PrevPos { x: ax, y: ay },
        ));
    }
    if act_type == ACT_EXIT_SIGN {
        entity.insert(ExitSign {
            x: ax,
            y: ay,
        });
    }
    if EXIT_ACT_IDS.contains(&act_type) {
        entity.insert(ExitTrigger {
            x: ax,
            y: ay,
        });
    }
    if crate::pickups::pickup_for_sprite(sprite_type).is_some() {
        entity.insert(Collectible {
            x: ax,
            y: ay,
            spr: sprite_type,
        });
    }
    if crate::enemy::HAZARD_ACT_IDS.contains(&act_type) {
        entity.insert(crate::enemy::Hazard);
    }
    if let Some(contents) = opencosmo_assets::actor_sprite_map::container_contents(act_type) {
        if contents != act_type {
            // ACT_BASKET_NULL "contains itself" - the game's encoding
            // for an empty basket; nothing to break out of it.
            entity.insert(Container {
                x: ax,
                y: ay,
                contents,
            });
        }
    }
    if behavior.is_none() && crate::enemy::WALKER_ACT_IDS.contains(&act_type) {
        let width_tiles = (*width_px as f32 / 8.0).ceil() as i32;
        entity.insert(crate::enemy::Walker {
            x: ax,
            y: ay,
            dir: 1,
            width_tiles,
            height_tiles: height_tiles as i32,
        });
    }
}
