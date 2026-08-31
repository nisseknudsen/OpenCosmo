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

/// How long the invincibility bubble lasts (game1.c:5325).
pub const INVINCIBLE_TICKS: u32 = 240;

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
            crate::pickups::Pickup::Star
            | crate::pickups::Pickup::PowerUp
            | crate::pickups::Pickup::Invincibility => snd::BIG_PRIZE,
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
            crate::pickups::Pickup::Invincibility => {
                // 240 ticks, the lifetime of the bubble actor
                // (game1.c:5325). The score effect shows 12800 and awards
                // nothing, which is the original's own bug.
                player.invincible_ticks = INVINCIBLE_TICKS;
                crate::effects::spawn_score_effect(&mut commands, &effects, 12800, c.x, c.y);
                None
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
    mut tile_index: ResMut<level::TileIndex>,
    mut switches: ResMut<crate::enemy_ai::SwitchState>,
    mut images: ResMut<Assets<Image>>,
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
        load_level_into_world(&mut commands, &asset_server, &data, &tileset, &mut tile_index, &mut switches, &mut images, &name)
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

    /// `NextLevel()` (game1.c:9968-10046).
    ///
    /// Not simply "the next entry". `order.json` is the level *number*
    /// table the original indexes with `levelNum`, and the progression runs
    /// in fours: two main levels, then a pair of bonus stages. Which of
    /// those you get - if any - depends on how many stars you have
    /// collected, counted across the whole game rather than per section:
    ///
    /// - more than 49 stars: the better of the two bonus stages
    /// - more than 24: the lesser one
    /// - otherwise: skipped entirely, straight on to the next section
    ///
    /// Advancing by one every time, as this used to, handed out both bonus
    /// stages unconditionally and never let a section be skipped.
    ///
    /// Returns the intermission to show before the next level loads.
    pub fn advance(&mut self, stars: u32) -> Option<Intermission> {
        let len = self.order.len();
        let intermission = match self.index % 4 {
            // A first level: straight on to the second, no ceremony.
            0 => {
                self.index += 1;
                None
            }
            // A second level: the section is over. The bonus stages are
            // the reward for stars, and are skipped without them.
            1 => {
                if stars > 49 {
                    self.index += 2;
                } else if stars > 24 {
                    self.index += 1;
                } else {
                    self.index += 3;
                }
                Some(Intermission::SectionCompleted)
            }
            // The lesser bonus stage falls through to the better one's
            // case in the original, so both land on the next section.
            2 => {
                self.index += 2;
                Some(Intermission::BonusCompleted)
            }
            _ => {
                self.index += 1;
                Some(Intermission::BonusCompleted)
            }
        };
        self.index %= len.max(1);
        intermission
    }
}

/// The frame shown between levels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intermission {
    SectionCompleted,
    BonusCompleted,
}

impl Intermission {
    pub fn title(self) -> &'static str {
        match self {
            Intermission::SectionCompleted => "Section Completed!",
            Intermission::BonusCompleted => "Bonus Level Completed!!",
        }
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
    tile_index: &mut level::TileIndex,
    switches: &mut crate::enemy_ai::SwitchState,
    images: &mut Assets<Image>,
    stem: &str,
) -> Option<CurrentLevel> {
    let mut level = data.load_level(stem)?;
    switches.reset_for_level(&level);
    // Pedestal caps are solid floor; stamped in before anything reads the
    // map so collision and rendering agree.
    actors::apply_pedestal_platforms(&mut level);
    let bounds = level::content_bounds(&level);
    level::spawn_backdrop(commands, asset_server, &level, bounds);
    level::spawn_level_tiles(commands, tileset, &level, data, tile_index);
    level::spawn_level_lights(commands, images, &level, data);
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
/// Raised when a level is beaten. Separate from `EnterLevel` because the
/// next level must not load until any intermission has been dismissed -
/// otherwise the frame announcing "Section Completed!" is drawn over the
/// *next* level rather than the one just finished.
#[derive(Event)]
pub struct LevelFinished {
    pub level: String,
    pub intermission: Option<Intermission>,
}

#[derive(Event)]
pub struct EnterLevel {
    pub level: String,
}

/// The level waiting behind an open intermission frame.
#[derive(Resource, Default)]
pub struct PendingLevel(pub Option<String>);

pub fn check_level_exit(
    exit_q: Query<&ExitTrigger>,
    sign_q: Query<&crate::actors::ExitSign>,
    player_q: Query<&Player>,
    scroll: Res<crate::camera::Scroll>,
    stars: Res<Stars>,
    mut sequence: ResMut<LevelSequence>,
    mut finished: EventWriter<LevelFinished>,
    mut sfx: EventWriter<PlaySfx>,
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
        // `StartSound(SND_WIN_LEVEL)` fires on every level win
        // (game1.c:10171), intermission or not - which is the only
        // acknowledgement you get for the levels that have no frame.
        sfx.write(PlaySfx(snd::WIN_LEVEL));
        let intermission = sequence.advance(stars.0);
        finished.write(LevelFinished {
            level: sequence.current().to_string(),
            intermission,
        });
    }
}

/// Marks the between-levels frame.
#[derive(Component)]
pub struct IntermissionUi;

/// Shows "Section Completed!" / "Bonus Level Completed!!" over the finished
/// level and waits for a key (`ShowSectionIntermission`, game1.c:10009 and
/// 10028). Without it a level simply cut to the next one mid-stride, which
/// is what made finishing a level feel like a glitch rather than an event.
#[allow(clippy::too_many_arguments)]
pub fn show_intermission(
    mut commands: Commands,
    mut events: EventReader<LevelFinished>,
    mut pending: ResMut<PendingLevel>,
    mut paused: ResMut<crate::help::Paused>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    mut enter: EventWriter<EnterLevel>,
) {
    let Some(event) = events.read().last() else {
        return;
    };
    let Some(intermission) = event.intermission else {
        // Nothing to announce - load straight through.
        enter.write(EnterLevel {
            level: event.level.clone(),
        });
        return;
    };
    pending.0 = Some(event.level.clone());
    paused.0 = true;
    crate::panel::TextFrame::new(7, 7, 34, intermission.title(), "Press ANY key.")
        .spawn(&mut commands, &hud, ui_camera.0, IntermissionUi);
}

/// Any key dismisses it and lets the queued level load.
pub fn close_intermission(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<crate::player::PlayerInput>,
    mut paused: ResMut<crate::help::Paused>,
    mut pending: ResMut<PendingLevel>,
    mut enter: EventWriter<EnterLevel>,
    open: Query<Entity, With<IntermissionUi>>,
) {
    let pressed = keys.get_just_pressed().next().is_some() || input.dismiss;
    if open.is_empty() || !pressed {
        return;
    }
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
    if let Some(level) = pending.0.take() {
        enter.write(EnterLevel { level });
    }
}

/// Closes it if the level goes away underneath it.
pub fn clear_intermission(
    mut commands: Commands,
    mut paused: ResMut<crate::help::Paused>,
    mut pending: ResMut<PendingLevel>,
    open: Query<Entity, With<IntermissionUi>>,
) {
    pending.0 = None;
    for entity in &open {
        commands.entity(entity).despawn();
        paused.0 = false;
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
    mut tile_index: ResMut<level::TileIndex>,
    mut switches: ResMut<crate::enemy_ai::SwitchState>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(EnterLevel { level: next_name }) = events.read().last() else {
        return;
    };
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };

    for e in &scoped_q {
        commands.entity(e).despawn();
    }

    let Some(new_current) =
        load_level_into_world(&mut commands, &asset_server, &data, &tileset, &mut tile_index, &mut switches, &mut images, next_name)
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

    fn sequence() -> LevelSequence {
        LevelSequence {
            order: (0..12).map(|i| format!("L{i}")).collect(),
            index: 0,
        }
    }

    #[test]
    fn a_first_level_leads_straight_to_the_second() {
        let mut s = sequence();
        assert_eq!(s.advance(0), None, "no ceremony mid-section");
        assert_eq!(s.index, 1);
    }

    #[test]
    fn too_few_stars_skips_the_bonus_stages_entirely() {
        // game1.c:10042 - `levelNum += 3`, straight past both.
        let mut s = sequence();
        s.index = 1;
        assert_eq!(s.advance(24), Some(Intermission::SectionCompleted));
        assert_eq!(s.index, 4, "should land on the next section's first level");
    }

    #[test]
    fn twenty_five_stars_earns_the_lesser_bonus_stage() {
        let mut s = sequence();
        s.index = 1;
        assert_eq!(s.advance(25), Some(Intermission::SectionCompleted));
        assert_eq!(s.index, 2);
    }

    #[test]
    fn fifty_stars_earns_the_better_one() {
        let mut s = sequence();
        s.index = 1;
        assert_eq!(s.advance(50), Some(Intermission::SectionCompleted));
        assert_eq!(s.index, 3);
    }

    #[test]
    fn either_bonus_stage_leads_to_the_next_section() {
        // The lesser one falls through the better one's case in the
        // original, so both end up on the same level.
        for (from, stars) in [(2usize, 0u32), (3, 0)] {
            let mut s = sequence();
            s.index = from;
            assert_eq!(s.advance(stars), Some(Intermission::BonusCompleted));
            assert_eq!(s.index, 4, "from bonus slot {from}");
        }
    }

    #[test]
    fn a_full_run_without_stars_never_visits_a_bonus_stage() {
        let mut s = sequence();
        let mut visited = vec![s.index];
        for _ in 0..5 {
            s.advance(0);
            visited.push(s.index);
        }
        assert!(
            !visited.iter().any(|i| i % 4 == 2 || i % 4 == 3),
            "starless run reached a bonus slot: {visited:?}"
        );
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
