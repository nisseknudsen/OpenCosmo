//! Transient visual effects: explosions, pounce debris, and score pop-ups.
//!
//! These sprites are never placed in a level - the game creates them at
//! runtime - so `cosmo_assets::convert::EFFECT_SPRITES` force-converts them
//! even though no map actor references them.
//!
//! Explosion timing follows `NewExplosion`/`DrawExplosions`
//! (game1.c:6592-6650): an explosion starts at age 1 two tiles below the
//! requested origin, shows frame `(age - 1) % 4`, hurts the player on
//! contact, and expires once age reaches 9.

use crate::data::GameData;
use crate::level::{tile_topleft_to_center, LevelScoped};
use crate::player::Player;
use crate::sfx::{snd, PlaySfx};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

pub const SPR_POUNCE_DEBRIS: u16 = 21;
pub const SPR_BOMB_ARMED: u16 = 24;
pub const SPR_EXPLOSION: u16 = 26;
/// SPR_SCORE_EFFECT_100..12800 are consecutive (sprite.h:199-206), so the
/// pop-up for a score is `BASE + log2(points / 100)`.
const SPR_SCORE_EFFECT_BASE: u16 = 177;

const EXPLOSION_MAX_AGE: u32 = 9;

/// Frame handles plus the first frame's pixel size, which is all the
/// positioning maths needs.
pub struct EffectSprite {
    pub frames: Vec<Handle<Image>>,
    pub width_px: f32,
    pub height_px: f32,
}

#[derive(Resource, Default)]
pub struct EffectAssets {
    sprites: HashMap<u16, EffectSprite>,
}

impl EffectAssets {
    pub fn load(asset_server: &AssetServer, data: &GameData) -> Self {
        let mut sprites = HashMap::new();
        for &spr in cosmo_assets::convert::EFFECT_SPRITES {
            let rel_dir = format!("sprites/actors/{spr}");
            let Some(manifest) = data.load_sprite_manifest(&rel_dir) else {
                continue;
            };
            let Some(first) = manifest.frames.first() else {
                continue;
            };
            sprites.insert(
                spr,
                EffectSprite {
                    width_px: first.width_px as f32,
                    height_px: first.height_px as f32,
                    frames: manifest
                        .frames
                        .iter()
                        .map(|f| asset_server.load(format!("generated/{rel_dir}/{}", f.file)))
                        .collect(),
                },
            );
        }
        Self { sprites }
    }

    pub fn get(&self, spr: u16) -> Option<&EffectSprite> {
        self.sprites.get(&spr)
    }
}

fn place(sprite: &EffectSprite, x: i32, y: i32) -> Vec3 {
    let h_tiles = (sprite.height_px / 8.0).ceil();
    tile_topleft_to_center(
        x as f32,
        y as f32 - h_tiles + 1.0,
        sprite.width_px,
        sprite.height_px,
    )
    .extend(8.0)
}

#[derive(Component)]
pub struct Explosion {
    pub x: i32,
    pub y: i32,
    pub age: u32,
}

/// A short-lived animated sprite that optionally drifts, covering the
/// original's decorations and shards (pounce debris, sparkles, smoke).
#[derive(Component)]
pub struct Decoration {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    frame: usize,
    frames: Vec<Handle<Image>>,
    width_px: f32,
    height_px: f32,
}

#[derive(Component)]
pub struct ScoreEffect {
    x: i32,
    y: i32,
    age: u32,
}

pub fn spawn_explosion(commands: &mut Commands, effects: &EffectAssets, x: i32, y: i32) {
    let Some(sprite) = effects.get(SPR_EXPLOSION) else {
        return;
    };
    // NewExplosion offsets two tiles down from the requested origin.
    let (x, y) = (x, y + 2);
    commands.spawn((
        Sprite {
            image: sprite.frames[0].clone(),
            ..default()
        },
        Transform::from_translation(place(sprite, x, y)),
        Explosion { x, y, age: 1 },
        LevelScoped,
    ));
}

/// Six pieces radiating outward, mirroring `NewPounceDecoration`
/// (game1.c:6935-6941).
pub fn spawn_pounce_debris(commands: &mut Commands, effects: &EffectAssets, x: i32, y: i32) {
    let Some(sprite) = effects.get(SPR_POUNCE_DEBRIS) else {
        return;
    };
    const PIECES: [(i32, i32, i32, i32); 6] = [
        (1, 0, -1, 1),
        (3, 0, 1, 1),
        (4, -2, 1, 0),
        (3, -4, 1, -1),
        (1, -4, -1, -1),
        (0, -2, -1, 0),
    ];
    for (ox, oy, dx, dy) in PIECES {
        let (px, py) = (x + ox, y + oy);
        commands.spawn((
            Sprite {
                image: sprite.frames[0].clone(),
                ..default()
            },
            Transform::from_translation(place(sprite, px, py)),
            Decoration {
                x: px,
                y: py,
                dx,
                dy,
                frame: 0,
                frames: sprite.frames.clone(),
                width_px: sprite.width_px,
                height_px: sprite.height_px,
            },
            LevelScoped,
        ));
    }
}

pub fn spawn_score_effect(
    commands: &mut Commands,
    effects: &EffectAssets,
    points: u32,
    x: i32,
    y: i32,
) {
    let steps = (points / 100).max(1).ilog2() as u16;
    let Some(sprite) = effects.get(SPR_SCORE_EFFECT_BASE + steps) else {
        return;
    };
    commands.spawn((
        Sprite {
            image: sprite.frames[0].clone(),
            ..default()
        },
        Transform::from_translation(place(sprite, x, y)),
        ScoreEffect { x, y, age: 0 },
        LevelScoped,
    ));
}

pub fn tick_explosions(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut query: Query<(Entity, &mut Explosion, &mut Sprite, &mut Transform)>,
    mut player_q: Query<&mut Player>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Some(sprite) = effects.get(SPR_EXPLOSION) else {
        return;
    };
    let player = player_q.single_mut().ok();
    let mut player = player;

    for (entity, mut ex, mut spr, mut transform) in &mut query {
        ex.age += 1;
        if ex.age >= EXPLOSION_MAX_AGE {
            commands.entity(entity).despawn();
            continue;
        }
        let frame = ((ex.age - 1) % 4) as usize;
        if let Some(handle) = sprite.frames.get(frame) {
            spr.image = handle.clone();
        }
        transform.translation = place(sprite, ex.x, ex.y);

        if let Some(p) = player.as_mut() {
            // Same sprite-rect test the enemy side uses, so standing right
            // next to your own bomb is as dangerous as it should be.
            let blast_w = (sprite.width_px / 8.0).ceil() as i32;
            let blast_h = (sprite.height_px / 8.0).ceil() as i32;
            if p.dead_timer == 0
                && p.hurt_cooldown == 0
                && crate::combat::rects_overlap(
                    ex.x,
                    ex.y,
                    blast_w,
                    blast_h,
                    p.x,
                    p.y,
                    crate::player::PLAYER_WIDTH,
                    crate::player::PLAYER_HEIGHT,
                )
            {
                p.health -= 1;
                sfx.write(PlaySfx(snd::PLAYER_HURT));
                p.hurt_cooldown = 44;
                if p.health <= 0 {
                    p.dead_timer = 1;
                }
            }
        }
    }
}

pub fn tick_decorations(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Decoration, &mut Sprite, &mut Transform)>,
) {
    for (entity, mut dec, mut spr, mut transform) in &mut query {
        dec.frame += 1;
        if dec.frame >= dec.frames.len() {
            commands.entity(entity).despawn();
            continue;
        }
        dec.x += dec.dx;
        dec.y += dec.dy;
        spr.image = dec.frames[dec.frame].clone();
        let h_tiles = (dec.height_px / 8.0).ceil();
        transform.translation = tile_topleft_to_center(
            dec.x as f32,
            dec.y as f32 - h_tiles + 1.0,
            dec.width_px,
            dec.height_px,
        )
        .extend(8.0);
    }
}

/// Score pop-ups drift upward for a few ticks, then vanish.
pub fn tick_score_effects(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut query: Query<(Entity, &mut ScoreEffect, &mut Transform)>,
) {
    let Some(sprite) = effects.get(SPR_SCORE_EFFECT_BASE) else {
        return;
    };
    for (entity, mut fx, mut transform) in &mut query {
        fx.age += 1;
        if fx.age > 10 {
            commands.entity(entity).despawn();
            continue;
        }
        fx.y -= 1;
        transform.translation = place(sprite, fx.x, fx.y);
    }
}
