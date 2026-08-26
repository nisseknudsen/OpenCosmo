//! Pouncing on enemies and placing bombs - the two ways the player deals
//! damage back.
//!
//! Pounce eligibility follows the `isPounceReady` test (game1.c:7085-7092):
//! the player must be descending onto the actor's top row, horizontally
//! overlapping it, with an extra row of tolerance once falling fast.
//! `TryPounce` itself lives on `Player` since it's pure player state.
//!
//! Bombs follow `MovePlayer`'s bomb branch (game1.c:8497-8546) and
//! `ActBombArmed` (game1.c:2286-2316): a placed bomb sits for a short fuse
//! and then produces an explosion, which is what actually does the damage.

use crate::actors::Collectible;
use crate::effects::{self, EffectAssets, Explosion, SPR_BOMB_ARMED};
use crate::actors::Container;
use crate::enemy_ai::{Enemy, EnemyKind, CONTAINER_POUNCE_RECOIL};
use crate::flow::Score;
use crate::level::{tile_topleft_to_center, LevelScoped};
use crate::player::{Player, PlayerInput, PLAYER_WIDTH};
use bevy::prelude::*;

const POUNCE_SCORE: u32 = 100;

/// Ticks a placed bomb sits before detonating. `ActBombArmed` advances a
/// 4-frame fuse every 5 ticks and then counts 10 more at the last frame.
const BOMB_FUSE_TICKS: u32 = 30;

#[derive(Component)]
pub struct ArmedBomb {
    pub x: i32,
    pub y: i32,
    pub fuse: u32,
}

/// The `isPounceReady` geometry test (game1.c:7085-7092), lifted out as a
/// pure function so it can be unit-tested without a running world.
///
/// `enemy_y` is the actor's *bottom* row, matching the original's
/// convention, so its top row is `enemy_y - height + 1`. Falling fast earns
/// one extra row of leeway, so a two-tile-per-tick drop can't tunnel
/// straight past a one-tile-tall target.
#[allow(clippy::too_many_arguments)]
pub fn is_pounce_aligned(
    player_x: i32,
    player_y: i32,
    fall_time: u32,
    enemy_x: i32,
    enemy_y: i32,
    width_tiles: i32,
    height_tiles: i32,
) -> bool {
    let top_row = enemy_y - height_tiles + 1;
    let leeway = if fall_time > 3 { 1 } else { 0 };
    let vertically_aligned = leeway + top_row >= player_y && top_row - 1 <= player_y;
    let horizontally_aligned =
        player_x + PLAYER_WIDTH - 1 >= enemy_x && enemy_x + width_tiles - 1 >= player_x;
    vertically_aligned && horizontally_aligned
}

pub fn pounce_enemies(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut score: ResMut<Score>,
    mut player_q: Query<&mut Player>,
    mut enemies: Query<(Entity, &mut Enemy)>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.dead_timer != 0 {
        return;
    }

    for (entity, mut enemy) in &mut enemies {
        let Some(spec) = enemy.kind.pounce_spec() else {
            continue;
        };
        if enemy.dead {
            continue;
        }
        if !is_pounce_aligned(
            player.x,
            player.y,
            player.fall_time,
            enemy.x,
            enemy.y,
            enemy.width_tiles,
            enemy.height_tiles,
        ) {
            continue;
        }
        if !player.try_pounce(spec.recoil) {
            continue;
        }

        // Tougher actors soak several pounces before dying, each one still
        // bouncing the player (game1.c:7160-7175, 7247-7260).
        enemy.pounce_hits -= 1;
        if enemy.pounce_hits > 0 {
            debug!(
                "pounced enemy at ({}, {}), {} hit(s) left",
                enemy.x, enemy.y, enemy.pounce_hits
            );
            break;
        }

        debug!("pounce killed enemy at ({}, {})", enemy.x, enemy.y);
        commands.entity(entity).despawn();
        effects::spawn_pounce_debris(&mut commands, &effects, enemy.x, enemy.y);
        effects::spawn_score_effect(&mut commands, &effects, POUNCE_SCORE, enemy.x, enemy.y);
        score.0 += POUNCE_SCORE;
        break; // one pounce per tick
    }
}

/// Baskets and barrels burst when landed on, bouncing the player a little
/// less than a creature does (game1.c:7152-7158).
pub fn pounce_containers(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut score: ResMut<Score>,
    mut player_q: Query<&mut Player>,
    containers: Query<(Entity, &Container)>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if player.dead_timer != 0 {
        return;
    }
    for (entity, container) in &containers {
        // Baskets and barrels are 2x2.
        if !is_pounce_aligned(
            player.x,
            player.y,
            player.fall_time,
            container.x,
            container.y,
            2,
            2,
        ) {
            continue;
        }
        if !player.try_pounce(CONTAINER_POUNCE_RECOIL) {
            continue;
        }
        debug!("burst container at ({}, {})", container.x, container.y);
        commands.entity(entity).despawn();
        effects::spawn_pounce_debris(&mut commands, &effects, container.x, container.y);
        effects::spawn_score_effect(
            &mut commands,
            &effects,
            POUNCE_SCORE,
            container.x,
            container.y,
        );
        score.0 += POUNCE_SCORE;
        break;
    }
}

pub fn place_bomb(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    input: Res<PlayerInput>,
    mut latch: Local<bool>,
    mut player_q: Query<&mut Player>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    if !input.bomb {
        *latch = false;
        return;
    }
    if *latch || player.bombs == 0 || player.dead_timer != 0 {
        return;
    }
    *latch = true;
    player.bombs -= 1;
    debug!("placed bomb at ({}, {})", player.x, player.y);

    let Some(sprite) = effects.get(SPR_BOMB_ARMED) else {
        return;
    };
    // Dropped just behind the player, on the side being faced.
    let (x, y) = (player.x, player.y);
    let h_tiles = (sprite.height_px / 8.0).ceil();
    commands.spawn((
        Sprite {
            image: sprite.frames[0].clone(),
            ..default()
        },
        Transform::from_translation(
            tile_topleft_to_center(
                x as f32,
                y as f32 - h_tiles + 1.0,
                sprite.width_px,
                sprite.height_px,
            )
            .extend(7.0),
        ),
        ArmedBomb {
            x,
            y,
            fuse: BOMB_FUSE_TICKS,
        },
        LevelScoped,
    ));
}

pub fn tick_bombs(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut bombs: Query<(Entity, &mut ArmedBomb)>,
) {
    for (entity, mut bomb) in &mut bombs {
        if bomb.fuse > 0 {
            bomb.fuse -= 1;
            continue;
        }
        debug!("bomb detonated at ({}, {})", bomb.x, bomb.y);
        commands.entity(entity).despawn();
        effects::spawn_explosion(&mut commands, &effects, bomb.x, bomb.y);
    }
}

/// Do two tile rectangles overlap? Both are given by their *bottom* row and
/// left column, matching the actor coordinate convention, so a rect spans
/// rows `y - h + 1 ..= y` and columns `x ..= x + w - 1`.
pub fn rects_overlap(
    ax: i32,
    ay: i32,
    aw: i32,
    ah: i32,
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
) -> bool {
    let horizontal = ax <= bx + bw - 1 && bx <= ax + aw - 1;
    let vertical = ay - ah + 1 <= by && by - bh + 1 <= ay;
    horizontal && vertical
}

/// Anything caught in a blast dies. `IsNearExplosion` (game1.c:6655-6670)
/// intersects the explosion's sprite rect with the target's, which matters
/// here because the blast is a 6x6-tile sprite anchored two rows *below*
/// the bomb - a symmetric radius around that origin would sit too low and
/// too small, and miss things standing right next to the bomb.
pub fn explosion_damage(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    mut score: ResMut<Score>,
    explosions: Query<&Explosion>,
    enemies: Query<(Entity, &Enemy)>,
) {
    let (blast_w, blast_h) = effects
        .get(effects::SPR_EXPLOSION)
        .map(|s| {
            (
                (s.width_px / 8.0).ceil() as i32,
                (s.height_px / 8.0).ceil() as i32,
            )
        })
        .unwrap_or((6, 6));

    for explosion in &explosions {
        for (entity, enemy) in &enemies {
            // Blasts destroy more than pounces do - the roamer slug, for
            // instance, can only be killed this way (it appears in the
            // shard/destruction switch at game1.c:6955-7010 but never
            // calls TryPounce). Floating collectibles are the exception.
            if enemy.dead || enemy.kind == EnemyKind::Prize {
                continue;
            }
            if rects_overlap(
                explosion.x,
                explosion.y,
                blast_w,
                blast_h,
                enemy.x,
                enemy.y,
                enemy.width_tiles,
                enemy.height_tiles,
            ) {
                debug!("explosion killed enemy at ({}, {})", enemy.x, enemy.y);
                commands.entity(entity).despawn();
                effects::spawn_pounce_debris(&mut commands, &effects, enemy.x, enemy.y);
                effects::spawn_score_effect(&mut commands, &effects, POUNCE_SCORE, enemy.x, enemy.y);
                score.0 += POUNCE_SCORE;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;

    /// A 2x2 enemy whose bottom row is 20, i.e. occupying rows 19..20.
    fn aligned(player_y: i32, fall_time: u32) -> bool {
        is_pounce_aligned(10, player_y, fall_time, 10, 20, 2, 2)
    }

    #[test]
    fn lands_on_the_enemys_top_row() {
        assert!(aligned(19, 0), "standing on the top row should pounce");
        assert!(aligned(18, 0), "arriving one row above should pounce");
    }

    #[test]
    fn ignores_targets_far_above_or_below() {
        assert!(!aligned(10, 0), "well above the enemy");
        assert!(!aligned(25, 0), "already below the enemy");
    }

    #[test]
    fn falling_fast_earns_one_extra_row_of_leeway() {
        // Without leeway this row is past the target; a fast fall still counts.
        assert!(!aligned(20, 0));
        assert!(aligned(20, 4));
    }

    #[test]
    fn requires_horizontal_overlap() {
        // Enemy far to the right of a 3-tile-wide player at x=10.
        assert!(!is_pounce_aligned(10, 19, 0, 40, 20, 2, 2));
    }

    #[test]
    fn blast_rect_reaches_targets_beside_and_above_it() {
        // 6x6 blast anchored at (495, 55) covers rows 50..55, cols 495..500.
        let blast = (495, 55, 6, 6);
        // A 3x3 enemy at (496, 53) sits squarely inside it.
        assert!(rects_overlap(blast.0, blast.1, blast.2, blast.3, 496, 53, 3, 3));
        // One just off the left edge still overlaps via its own width.
        assert!(rects_overlap(blast.0, blast.1, blast.2, blast.3, 493, 53, 3, 3));
        // Far away in either axis does not.
        assert!(!rects_overlap(blast.0, blast.1, blast.2, blast.3, 480, 53, 3, 3));
        assert!(!rects_overlap(blast.0, blast.1, blast.2, blast.3, 496, 20, 3, 3));
    }

    #[test]
    fn pounce_requires_descending() {
        let mut player = Player::spawn_at(0, 0);
        player.is_falling = false;
        player.jump_time = 0;
        assert!(!player.try_pounce(7), "grounded player cannot pounce");

        player.is_falling = true;
        assert!(player.try_pounce(7), "falling player can pounce");
        assert!(player.is_recoiling);
        assert_eq!(player.recoil_left, 8);
    }

    #[test]
    fn pounce_does_not_retrigger_mid_bounce() {
        let mut player = Player::spawn_at(0, 0);
        player.is_falling = true;
        assert!(player.try_pounce(7));
        // Still early in the bounce, so a second hit is refused.
        assert!(!player.try_pounce(7));
    }
}
