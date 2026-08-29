//! Level progression: touching an exit actor (ACT_EXIT_MONSTER_W/N,
//! ACT_EXIT_PLANT, ACT_EXIT_TRANSPORTER) advances to the next level in the
//! episode's `A1 A2 bonus1 bonus2 A3 A4 bonus1 bonus2 ...` pattern (the
//! actual pattern from episode1.h's MAP_NAMES, truncated to whichever A*
//! levels this installer's data actually shipped - see docs/file-formats.md).

use crate::actors::{self, Collectible, ExitTrigger};
use crate::data::GameData;
use crate::level::{self, CurrentLevel, LevelScoped};
use crate::player::Player;
use crate::sfx::{snd, PlaySfx};
use crate::tileset::TilesetAssets;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Score(pub u32);

/// The original's separate "Stars" status-bar counter (game2.c:1249).
#[derive(Resource, Default)]
pub struct Stars(pub u32);

/// The state captured on entering a level, restored when the player dies.
///
/// `InitializeLevel` ends with `SaveGameState('T')` (game1.c:10555) and
/// every death path reloads that same slot (game1.c:9207, 9252), so dying
/// rewinds score, stars, bombs and health to what they were when the level
/// started - it does not merely move the player back. The snapshot's
/// contents are exactly the fields `SaveGameState` writes (game1.c:9369-9375).
#[derive(Resource, Default, Clone, Copy)]
pub struct Checkpoint {
    pub score: u32,
    pub stars: u32,
    pub health: i32,
    pub health_cells: u32,
    pub bombs: u32,
}

impl Checkpoint {
    pub fn capture(score: &Score, stars: &Stars, player: &Player) -> Self {
        Checkpoint {
            score: score.0,
            stars: stars.0,
            health: player.health,
            health_cells: player.health_cells,
            bombs: player.bombs,
        }
    }

    pub fn restore(&self, score: &mut Score, stars: &mut Stars, player: &mut Player) {
        score.0 = self.score;
        stars.0 = self.stars;
        player.health = self.health;
        player.health_cells = self.health_cells;
        player.bombs = self.bombs;
    }
}

pub fn collect_pickups(
    mut commands: Commands,
    effects: Res<crate::effects::EffectAssets>,
    mut score: ResMut<Score>,
    mut stars: ResMut<Stars>,
    mut player_q: Query<&mut Player>,
    pickup_q: Query<(Entity, &Collectible)>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    // The death animation floats the corpse upward through whatever is
    // above it; it should not be hoovering up prizes on the way.
    if player.dead_timer != 0 {
        return;
    }
    for (entity, c) in &pickup_q {
        let Some(pickup) = crate::pickups::pickup_for_sprite(c.spr) else {
            continue;
        };
        if (c.x - player.x).abs() > 2 || (c.y - player.y).abs() > 3 {
            continue;
        }
        commands.entity(entity).despawn();

        // Stars and power-ups get the louder jingle (game1.c:7393, 7475);
        // everything else the ordinary pickup blip (game1.c:7500, 7542).
        sfx.write(PlaySfx(match pickup {
            crate::pickups::Pickup::Star | crate::pickups::Pickup::PowerUp => snd::BIG_PRIZE,
            _ => snd::PRIZE,
        }));

        // Each kind pays out differently; only plain score and the
        // power-up's payout raise a score pop-up.
        let awarded = match pickup {
            crate::pickups::Pickup::Score(points) => {
                score.0 += points;
                Some(points)
            }
            crate::pickups::Pickup::Star => {
                stars.0 += 1;
                None
            }
            crate::pickups::Pickup::Bomb => {
                // The original caps the counter at 9 (game1.c:7561).
                if player.bombs <= 8 {
                    player.bombs += 1;
                }
                score.0 += 100;
                Some(100)
            }
            crate::pickups::Pickup::Hamburger => {
                if player.health_cells < 5 {
                    player.health_cells += 1;
                }
                score.0 += 12800;
                Some(12800)
            }
            crate::pickups::Pickup::PowerUp => {
                // Heals while hurt, otherwise pays out (game1.c:7480-7488).
                if player.health <= player.health_cells as i32 {
                    player.health += 1;
                    score.0 += 100;
                    Some(100)
                } else {
                    score.0 += 12800;
                    Some(12800)
                }
            }
        };
        if let Some(points) = awarded {
            crate::effects::spawn_score_effect(&mut commands, &effects, points, c.x, c.y);
        }
    }
}

/// Raised to restart the current level from its entry snapshot - by dying,
/// and by the F1 menu's "Restart Level". It is a shared path because the
/// original treats them identically: both `playerDeadTime > 36`
/// (game1.c:9256-9258) and `HELP_MENU_RESTART` (game1.c:9821-9822) run
/// `LoadGameState('T'); InitializeLevel(levelNum);`.
#[derive(Event)]
pub struct RestartLevel;

/// Reloads the level and rewinds to its entry snapshot.
///
/// `InitializeLevel` rebuilds the actor array from the map, so every
/// creature the player killed on the failed attempt is back. Moving the
/// player alone - which is what this used to do on death - left the level
/// permanently stripped of whatever they had already cleared.
#[allow(clippy::too_many_arguments)]
pub fn restart_level(
    mut commands: Commands,
    mut events: EventReader<RestartLevel>,
    asset_server: Res<AssetServer>,
    data: Res<GameData>,
    tileset: Option<Res<TilesetAssets>>,
    mut current: ResMut<CurrentLevel>,
    scoped: Query<Entity, With<LevelScoped>>,
    mut player_q: Query<&mut Player>,
    checkpoint: Res<Checkpoint>,
    mut score: ResMut<Score>,
    mut stars: ResMut<Stars>,
    mut scroll: ResMut<crate::camera::Scroll>,
    mut saw_auto: ResMut<crate::hints::SawAutoHintGlobe>,
) {
    if events.read().next().is_none() {
        return;
    }
    let (Some(tileset), Ok(mut player)) = (tileset, player_q.single_mut()) else {
        return;
    };
    for entity in &scoped {
        commands.entity(entity).despawn();
    }
    let name = current.name.clone();
    if let Some(reloaded) =
        load_level_into_world(&mut commands, &asset_server, &data, &tileset, &name)
    {
        *current = reloaded;
    }
    if let Some(level) = data.load_level(&name) {
        let (sx, sy) = level::find_player_start(&level);
        player.x = sx as i32;
        player.y = sy as i32;
    }
    player.is_falling = true;
    player.jump_time = 0;
    player.fall_time = 0;
    player.cling_dir = None;
    player.dead_timer = 0;
    player.hurt_cooldown = 0;
    checkpoint.restore(&mut score, &mut stars, &mut player);
    scroll.centre_on(&player, &current);
    // `InitializeMapGlobals` clears this on every level init, restart
    // included (game1.c:10459), so the level's first globe greets the
    // player again on the retry.
    saw_auto.0 = false;
}

#[derive(Resource)]
pub struct LevelSequence {
    pub order: Vec<String>,
    pub index: usize,
}

impl LevelSequence {
    /// The order comes from the converter's per-episode `order.json`,
    /// which already interleaves each episode's bonus stages into its own
    /// level naming (`A*` for episode 1, `B*`/`C*` for 2 and 3). Deriving
    /// it here from episode 1's naming instead would leave episodes 2 and
    /// 3 with an empty progression.
    pub fn build(data: &GameData, start_hint: &str) -> Self {
        let mut order = data.level_order();
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

    let (width, height, music) = (level.width, level.height, level.music.clone());
    Some(CurrentLevel {
        name: stem.to_string(),
        level,
        width,
        height,
        content_min: (bounds.0, bounds.1),
        content_max: (bounds.2, bounds.3),
        music,
    })
}

/// Raised to move to a *different* level - by reaching an exit, and by the
/// developer level warp. Distinct from `RestartLevel` in that it takes a
/// fresh checkpoint on arrival instead of rewinding to the old one.
#[derive(Event)]
pub struct EnterLevel(pub String);

pub fn check_level_exit(
    exit_q: Query<&ExitTrigger>,
    sign_q: Query<&crate::actors::ExitSign>,
    player_q: Query<&Player>,
    scroll: Res<crate::camera::Scroll>,
    mut sequence: ResMut<LevelSequence>,
    mut enter: EventWriter<EnterLevel>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    if player.dead_timer != 0 {
        return; // drifting into an exit while dead shouldn't count
    }
    let touching = exit_q
        .iter()
        .any(|e| (e.x - player.x).abs() <= 2 && (e.y - player.y).abs() <= 3);
    // The exit sign ends the level on coming into view rather than on
    // contact - see `actors::EXIT_ACT_IDS` for why.
    let sign_in_view = sign_q.iter().any(|s| {
        s.x >= scroll.x
            && s.x < scroll.x + crate::camera::SCROLL_W
            && s.y >= scroll.y
            && s.y < scroll.y + crate::camera::SCROLL_H
    });
    if touching || sign_in_view {
        enter.write(EnterLevel(sequence.advance()));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn enter_level(
    mut commands: Commands,
    mut events: EventReader<EnterLevel>,
    mut checkpoint: ResMut<Checkpoint>,
    score: Res<Score>,
    stars: Res<Stars>,
    asset_server: Res<AssetServer>,
    data: Res<GameData>,
    tileset: Res<TilesetAssets>,
    mut current: ResMut<CurrentLevel>,
    scoped_q: Query<Entity, With<LevelScoped>>,
    mut player_q: Query<&mut Player>,
    mut scroll: ResMut<crate::camera::Scroll>,
    mut saw_auto: ResMut<crate::hints::SawAutoHintGlobe>,
) {
    let Some(EnterLevel(next_name)) = events.read().last() else {
        return;
    };
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };

    for e in &scoped_q {
        commands.entity(e).despawn();
    }

    let Some(new_current) =
        load_level_into_world(&mut commands, &asset_server, &data, &tileset, next_name)
    else {
        return;
    };

    let level_json = data.load_level(next_name).unwrap();
    let (sx, sy) = level::find_player_start(&level_json);
    player.x = sx as i32;
    player.y = sy as i32;
    *checkpoint = Checkpoint::capture(&score, &stars, &player);
    player.is_falling = true;
    player.jump_time = 0;
    player.fall_time = 0;
    player.cling_dir = None;
    player.dead_timer = 0;

    *current = new_current;
    scroll.centre_on(&player, &current);
    // `sawAutoHintGlobe` is per level (game1.c:10459), so the new level's
    // first globe greets the player unprompted like the last one's did.
    saw_auto.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_with(health: i32, cells: u32, bombs: u32) -> Player {
        let mut p = Player::spawn_at(0, 0);
        p.health = health;
        p.health_cells = cells;
        p.bombs = bombs;
        p
    }

    #[test]
    fn restores_everything_banked_since_the_level_started() {
        let score = Score(1200);
        let stars = Stars(3);
        let player = player_with(4, 3, 2);
        let checkpoint = Checkpoint::capture(&score, &stars, &player);

        // Play on: bank points, spend a bomb, take damage.
        let mut score = Score(9900);
        let mut stars = Stars(7);
        let mut player = player_with(1, 4, 0);

        checkpoint.restore(&mut score, &mut stars, &mut player);
        assert_eq!(score.0, 1200, "points banked during the attempt are lost");
        assert_eq!(stars.0, 3);
        assert_eq!(player.health, 4);
        assert_eq!(player.health_cells, 3);
        assert_eq!(player.bombs, 2);
    }

    #[test]
    fn a_fresh_checkpoint_round_trips() {
        let score = Score(0);
        let stars = Stars(0);
        let player = player_with(4, 3, 0);
        let checkpoint = Checkpoint::capture(&score, &stars, &player);

        let mut score = Score(500);
        let mut stars = Stars(1);
        let mut player = player_with(2, 3, 1);
        checkpoint.restore(&mut score, &mut stars, &mut player);
        assert_eq!((score.0, stars.0, player.health, player.bombs), (0, 0, 4, 0));
    }
}
