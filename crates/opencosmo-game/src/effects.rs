//! Transient visual effects: explosions, pounce debris, and score pop-ups.
//!
//! These sprites are never placed in a level - the game creates them at
//! runtime - so `opencosmo_assets::convert::EFFECT_SPRITES` force-converts them
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
        for &spr in opencosmo_assets::convert::EFFECT_SPRITES {
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
                        .map(|f| asset_server.load(crate::data::asset_path(&format!("{rel_dir}/{}", f.file))))
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

/// A short-lived animated sprite that drifts in a straight line - the
/// original's `Decoration` (game1.c:1430-1476): smoke, sparkles, splashes.
///
/// It plays its frames `times_left` times and then vanishes; `None` means
/// it runs until it leaves the view, which is what `numtimes == 0` does.
#[derive(Component)]
pub struct Decoration {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    frame: usize,
    times_left: Option<u32>,
    frames: Vec<Handle<Image>>,
    width_px: f32,
    height_px: f32,
}

/// `dir8X` / `dir8Y` (game1.c:82-83), indexed by `DIR8_*` (def.h:49-57).
/// Note index 0 is *none*, not north - the compass starts at 1.
pub const DIR8: [(i32, i32); 9] = [
    (0, 0),   // DIR8_NONE
    (0, -1),  // DIR8_NORTH
    (1, -1),  // DIR8_NORTHEAST
    (1, 0),   // DIR8_EAST
    (1, 1),   // DIR8_SOUTHEAST
    (0, 1),   // DIR8_SOUTH
    (-1, 1),  // DIR8_SOUTHWEST
    (-1, 0),  // DIR8_WEST
    (-1, -1), // DIR8_NORTHWEST
];

pub const DIR8_NONE: usize = 0;
pub const DIR8_NORTH: usize = 1;
pub const DIR8_NORTHEAST: usize = 2;
pub const DIR8_SOUTH: usize = 5;
pub const DIR8_SOUTHWEST: usize = 6;
pub const DIR8_WEST: usize = 7;
pub const DIR8_NORTHWEST: usize = 8;

/// `NewDecoration` (game1.c:1409-1428). The general form the behaviours
/// ask for: a sprite, how many frames of it to cycle, where, which way it
/// drifts, and how many times to repeat before it goes.
pub fn spawn_decoration(
    commands: &mut Commands,
    effects: &EffectAssets,
    spr: u16,
    num_frames: usize,
    x: i32,
    y: i32,
    dir8: usize,
    num_times: u32,
) {
    let Some(sprite) = effects.get(spr) else {
        return;
    };
    let frames: Vec<Handle<Image>> = sprite
        .frames
        .iter()
        .take(num_frames.max(1))
        .cloned()
        .collect();
    if frames.is_empty() {
        return;
    }
    let (dx, dy) = DIR8[dir8.min(8)];
    commands.spawn((
        Sprite {
            image: frames[0].clone(),
            ..default()
        },
        Transform::from_translation(place(sprite, x, y)),
        Decoration {
            x,
            y,
            dx,
            dy,
            frame: 0,
            times_left: (num_times != 0).then_some(num_times),
            frames,
            width_px: sprite.width_px,
            height_px: sprite.height_px,
        },
        LevelScoped,
    ));
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
                times_left: Some(1),
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
            // Blast damage goes through the same gate as contact damage:
            // `HurtPlayer` refuses outright while invincible
            // (game1.c:6905), so a bubble protects against your own bombs.
            if p.dead_timer == 0
                && p.hurt_cooldown == 0
                && !p.is_invincible()
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
                // Matches HurtPlayer's split: the cooldown and the hurt
                // sound belong to the *survived* branch, and a fatal hit
                // goes straight into the death animation.
                p.health -= 1;
                if p.health <= 0 {
                    p.dead_timer = 1;
                } else {
                    sfx.write(PlaySfx(snd::PLAYER_HURT));
                    p.hurt_cooldown = 44;
                }
            }
        }
    }
}

/// A piece of debris thrown by something breaking - `NewShard`
/// (game1.c:6435-6470) and `MoveAndDrawShards` (game1.c:1780-1860).
///
/// Unlike a decoration, a shard has physics: it is flung upward for four
/// ticks, then falls two rows a tick, bounces once off the first floor it
/// meets, and dies at forty or on leaving the view. It is drawn flipped
/// after the first tick, which is what makes debris read as tumbling.
#[derive(Component)]
pub struct Shard {
    x: i32,
    y: i32,
    age: u32,
    /// The horizontal drift, 0-4: east, west, still, double east, double
    /// west (game1.c:6437-6445).
    xmode: u32,
    bounced: bool,
    width_tiles: i32,
    height_tiles: i32,
}

/// `xmode` is a single counter shared by every shard and never reset, so
/// consecutive pieces fan out instead of moving together. cosmore flags
/// that this makes debris differ on every run; keeping it as one counter
/// rather than a per-shard random reproduces the fan.
#[derive(Resource, Default)]
pub struct ShardXMode(pub u32);

/// `NewShard` (game1.c:6435).
pub fn spawn_shard(
    commands: &mut Commands,
    effects: &EffectAssets,
    xmode: &mut ShardXMode,
    spr: u16,
    frame: usize,
    x: i32,
    y: i32,
) {
    let Some(sprite) = effects.get(spr) else {
        return;
    };
    let Some(image) = sprite.frames.get(frame).or_else(|| sprite.frames.first()) else {
        return;
    };
    xmode.0 = (xmode.0 + 1) % 5;
    commands.spawn((
        Sprite {
            image: image.clone(),
            flip_y: true,
            ..default()
        },
        Transform::from_translation(place(sprite, x, y)),
        Shard {
            x,
            y,
            age: 1,
            xmode: xmode.0,
            bounced: false,
            width_tiles: (sprite.width_px / 8.0).ceil() as i32,
            height_tiles: (sprite.height_px / 8.0).ceil() as i32,
        },
        LevelScoped,
    ));
}

/// One tick of a shard's arc, as a pure function so the trajectory can be
/// tested without a world. Returns false when the shard is spent.
///
/// The original expresses the bounce with a `goto` back to the top of the
/// vertical section (game1.c:1806); resetting the age to 3 and re-running
/// the same block is what that does.
pub fn step_shard(sh: &mut Shard, solid_below: impl Fn(i32, i32) -> bool, visible: bool) -> bool {
    match sh.xmode {
        0 | 3 => sh.x += if sh.xmode == 3 { 2 } else { 1 },
        1 | 4 => sh.x -= if sh.xmode == 4 { 2 } else { 1 },
        _ => {}
    }

    loop {
        if sh.age < 5 {
            sh.y -= 2;
        } else if sh.age == 5 {
            sh.y -= 1;
        } else if sh.age == 8 {
            if solid_below(sh.x, sh.y + 1) {
                sh.age = 3;
                sh.y += 2;
                continue;
            }
            sh.y += 1;
        }

        if sh.age >= 9 {
            if sh.age > 16 && !visible {
                return false;
            }
            for _ in 0..2 {
                if !sh.bounced && solid_below(sh.x, sh.y + 1) {
                    sh.age = 3;
                    sh.bounced = true;
                    break;
                }
                sh.y += 1;
            }
            if sh.age == 3 {
                continue;
            }
        }
        break;
    }

    sh.age += 1;
    sh.age <= 40
}

pub fn tick_shards(
    mut commands: Commands,
    level: Res<crate::level::CurrentLevel>,
    data: Res<GameData>,
    scroll: Res<crate::camera::Scroll>,
    mut query: Query<(Entity, &mut Shard, &mut Sprite, &mut Transform)>,
) {
    for (entity, mut sh, mut spr, mut transform) in &mut query {
        let solid = |x: i32, y: i32| {
            if x < 0 || y < 0 {
                return false;
            }
            data.tile_attr(level.level.tile_at(x as usize, y as usize))
                & crate::data::TILE_ATTR_BLOCK_SOUTH
                != 0
        };
        let visible = crate::enemy_ai::is_visible_at(
            sh.x, sh.y, sh.width_tiles, sh.height_tiles, scroll.x, scroll.y,
        );
        if !step_shard(&mut sh, solid, visible) {
            commands.entity(entity).despawn();
            continue;
        }
        // White on its first tick, tumbling after (game1.c:1855-1859).
        spr.flip_y = sh.age != 1;
        let h = sh.height_tiles as f32;
        transform.translation = tile_topleft_to_center(
            sh.x as f32,
            sh.y as f32 - h + 1.0,
            sh.width_tiles as f32 * 8.0,
            sh.height_tiles as f32 * 8.0,
        )
        .extend(8.0);
    }
}

pub fn tick_decorations(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Decoration, &mut Sprite, &mut Transform)>,
) {
    for (entity, mut dec, mut spr, mut transform) in &mut query {
        dec.frame += 1;
        if dec.frame >= dec.frames.len() {
            // One pass of the frames done. A decoration with a repeat
            // count loops until it runs out (game1.c:1465-1473).
            dec.frame = 0;
            match dec.times_left.as_mut() {
                Some(1) | None => {
                    commands.entity(entity).despawn();
                    continue;
                }
                Some(n) => *n -= 1,
            }
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

#[cfg(test)]
mod shard_tests {
    use super::*;

    fn shard(xmode: u32) -> Shard {
        Shard {
            x: 20,
            y: 20,
            age: 1,
            xmode,
            bounced: false,
            width_tiles: 1,
            height_tiles: 1,
        }
    }

    /// A floor at row 25, nothing else.
    fn floor_at(row: i32) -> impl Fn(i32, i32) -> bool {
        move |_x, y| y >= row
    }

    #[test]
    fn a_shard_is_thrown_up_before_it_falls() {
        let mut sh = shard(2); // no horizontal drift
        let start = sh.y;
        let mut highest = sh.y;
        for _ in 0..8 {
            step_shard(&mut sh, floor_at(100), true);
            highest = highest.min(sh.y);
        }
        assert!(highest < start, "it should rise first, got {highest} vs {start}");
    }

    #[test]
    fn a_shard_bounces_once_and_only_once() {
        let mut sh = shard(2);
        let mut bounces = 0;
        for _ in 0..40 {
            let was = sh.bounced;
            step_shard(&mut sh, floor_at(25), true);
            if sh.bounced && !was {
                bounces += 1;
            }
        }
        assert_eq!(bounces, 1, "the original bounces a shard exactly once");
    }

    #[test]
    fn the_xmodes_fan_debris_out_in_five_ways() {
        // east, west, still, double east, double west (game1.c:6437-6445).
        let drift = |xmode| {
            let mut sh = shard(xmode);
            let x0 = sh.x;
            for _ in 0..5 {
                step_shard(&mut sh, floor_at(100), true);
            }
            sh.x - x0
        };
        assert_eq!(drift(0), 5, "east");
        assert_eq!(drift(1), -5, "west");
        assert_eq!(drift(2), 0, "still");
        assert_eq!(drift(3), 10, "double east");
        assert_eq!(drift(4), -10, "double west");
    }

    #[test]
    fn a_shard_does_not_live_forever() {
        let mut sh = shard(2);
        let mut ticks = 0;
        while step_shard(&mut sh, floor_at(1000), true) {
            ticks += 1;
            assert!(ticks < 200, "a shard that never expires would leak entities");
        }
        assert!(ticks <= 40, "it dies at forty (game1.c:1862)");
    }

    #[test]
    fn a_shard_off_screen_is_dropped_early() {
        let mut sh = shard(2);
        for _ in 0..17 {
            step_shard(&mut sh, floor_at(1000), true);
        }
        assert!(
            !step_shard(&mut sh, floor_at(1000), false),
            "past sixteen it is culled the moment it leaves the view"
        );
    }

    #[test]
    fn the_xmode_counter_is_shared_so_consecutive_shards_differ() {
        // cosmore notes this counter is never reset, which is what makes a
        // burst of debris fan out instead of moving as one.
        let mut xm = ShardXMode::default();
        let seen: Vec<u32> = (0..5)
            .map(|_| {
                xm.0 = (xm.0 + 1) % 5;
                xm.0
            })
            .collect();
        assert_eq!(seen, vec![1, 2, 3, 4, 0], "five distinct drifts in a row");
    }
}
