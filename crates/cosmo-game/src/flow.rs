//! Level progression: touching an exit actor (ACT_EXIT_MONSTER_W/N,
//! ACT_EXIT_PLANT, ACT_EXIT_TRANSPORTER) advances to the next level in the
//! episode's `A1 A2 bonus1 bonus2 A3 A4 bonus1 bonus2 ...` pattern (the
//! actual pattern from episode1.h's MAP_NAMES, truncated to whichever A*
//! levels this installer's data actually shipped - see docs/file-formats.md).

use crate::actors::{self, Collectible, Container, ExitTrigger};
use crate::data::GameData;
use crate::level::{self, CurrentLevel, LevelScoped};
use crate::player::Player;
use crate::tileset::TilesetAssets;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Score(pub u32);

/// The original's separate "Stars" status-bar counter (game2.c:1249).
#[derive(Resource, Default)]
pub struct Stars(pub u32);

pub fn collect_pickups(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut stars: ResMut<Stars>,
    player_q: Query<&Player>,
    pickup_q: Query<(Entity, &Collectible)>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    for (entity, c) in &pickup_q {
        if c.act_id == crate::combat::ACT_BOMB_IDLE {
            continue; // stocked by combat::collect_bombs, not scored
        }
        if (c.x - player.x).abs() <= 2 && (c.y - player.y).abs() <= 3 {
            commands.entity(entity).despawn();
            if actors::STAR_ACT_IDS.contains(&c.act_id) {
                stars.0 += 1;
            } else {
                score.0 += 100;
            }
        }
    }
}

/// Breaks a container when the player lands on top of it (not just walks
/// into its side) - approximated as "on the ground, roughly at the
/// container's own row". Awards points if what's inside is food/a gem;
/// containers holding a mechanism (a jump pad, a bomb, ...) just clear
/// without a score bump.
pub fn smash_containers(
    mut commands: Commands,
    mut score: ResMut<Score>,
    player_q: Query<&Player>,
    containers: Query<(Entity, &Container)>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    if !player.on_ground {
        return;
    }
    for (entity, c) in &containers {
        let dx = (c.x - player.x).abs();
        let dy = player.y - c.y;
        if dx <= 2 && (0..=2).contains(&dy) {
            commands.entity(entity).despawn();
            if actors::COLLECTIBLE_ACT_IDS.contains(&c.contents) {
                score.0 += 100;
            }
        }
    }
}

#[derive(Resource)]
pub struct LevelSequence {
    pub order: Vec<String>,
    pub index: usize,
}

impl LevelSequence {
    pub fn build(data: &GameData, start_hint: &str) -> Self {
        let all = data.list_levels();
        let mut a_levels: Vec<String> = all
            .iter()
            .filter(|n| n.starts_with('a') && n[1..].parse::<u32>().is_ok())
            .cloned()
            .collect();
        a_levels.sort_by_key(|n| n[1..].parse::<u32>().unwrap());

        let has_bonus1 = all.iter().any(|n| n == "bonus1");
        let has_bonus2 = all.iter().any(|n| n == "bonus2");

        let mut order = Vec::new();
        let mut i = 0;
        while i < a_levels.len() {
            order.push(a_levels[i].clone());
            if i + 1 < a_levels.len() {
                order.push(a_levels[i + 1].clone());
            }
            if has_bonus1 {
                order.push("bonus1".to_string());
            }
            if has_bonus2 {
                order.push("bonus2".to_string());
            }
            i += 2;
        }
        if order.is_empty() {
            order.push(start_hint.to_string());
        }
        let index = order.iter().position(|n| n == start_hint).unwrap_or(0);
        LevelSequence { order, index }
    }

    pub fn current(&self) -> &str {
        &self.order[self.index]
    }

    pub fn advance(&mut self) -> String {
        self.index = (self.index + 1) % self.order.len();
        self.current().to_string()
    }
}

/// Spawns everything for `stem`, replacing whatever level was previously
/// loaded (caller is responsible for despawning `LevelScoped` entities
/// first). Shared by initial Startup load and mid-game transitions.
pub fn load_level_into_world(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &GameData,
    tileset: &TilesetAssets,
    stem: &str,
) -> Option<CurrentLevel> {
    let level = data.load_level(stem)?;
    let bounds = level::content_bounds(&level);
    level::spawn_backdrop(commands, asset_server, &level, bounds);
    level::spawn_level_tiles(commands, tileset, &level, data);
    actors::spawn_level_actors(commands, asset_server, &level, data);

    Some(CurrentLevel {
        name: stem.to_string(),
        width: level.width,
        height: level.height,
        content_min: (bounds.0, bounds.1),
        content_max: (bounds.2, bounds.3),
        music: level.music.clone(),
    })
}

pub fn check_level_exit(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<GameData>,
    tileset: Res<TilesetAssets>,
    mut sequence: ResMut<LevelSequence>,
    mut current: ResMut<CurrentLevel>,
    exit_q: Query<&ExitTrigger>,
    scoped_q: Query<Entity, With<LevelScoped>>,
    mut player_q: Query<&mut Player>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    let touching = exit_q
        .iter()
        .any(|e| (e.x - player.x).abs() <= 2 && (e.y - player.y).abs() <= 3);
    if !touching {
        return;
    }

    for e in &scoped_q {
        commands.entity(e).despawn();
    }

    let next_name = sequence.advance();
    let Some(new_current) =
        load_level_into_world(&mut commands, &asset_server, &data, &tileset, &next_name)
    else {
        return;
    };

    let level_json = data.load_level(&next_name).unwrap();
    let (sx, sy) = level::find_player_start(&level_json);
    player.x = sx as i32;
    player.y = sy as i32;
    player.is_falling = true;
    player.jump_time = 0;
    player.fall_time = 0;
    player.cling_dir = None;

    *current = new_current;
}
