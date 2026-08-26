mod actors;
mod audio;
mod camera;
mod combat;
mod data;
mod effects;
mod enemy;
mod enemy_ai;
mod flow;
mod hud;
mod level;
mod pickups;
mod player;
mod screen;
mod sfx;
mod tileset;

use bevy::prelude::*;
use bevy::time::Fixed;
use data::GameData;
use flow::LevelSequence;
use player::{Player, PlayerFrames, PlayerInput};
use screen::{GameState, UiCamera};

const START_LEVEL: &str = "a1";

fn main() {
    let mut app = App::new();
    app.add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Cosmo's Cosmic Adventure (Reboot)".into(),
                        resolution: (1024.0, 640.0).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(Time::<Fixed>::from_hz(18.2))
        // COSMO_STATE lets a dev (or a headless screenshot run) jump
        // straight to a screen instead of clicking through the title.
        .insert_state(match std::env::var("COSMO_STATE").as_deref() {
            Ok("menu") => GameState::Menu,
            Ok("credits") => GameState::Credits,
            Ok("playing") => GameState::Playing,
            _ => GameState::Title,
        })
        .init_resource::<PlayerInput>()
        .init_resource::<flow::Score>()
        .init_resource::<flow::Stars>()
        .init_resource::<flow::Checkpoint>()
        .init_resource::<sfx::SfxState>()
        .add_event::<sfx::PlaySfx>()
        // --- Title / menu / credits ---
        .add_systems(OnEnter(GameState::Title), screen::spawn_title)
        .add_systems(OnExit(GameState::Title), screen::despawn_screen)
        .add_systems(OnEnter(GameState::Menu), screen::spawn_menu)
        .add_systems(OnExit(GameState::Menu), screen::despawn_screen)
        .add_systems(OnEnter(GameState::Credits), screen::spawn_credits)
        .add_systems(OnExit(GameState::Credits), screen::despawn_screen)
        .add_systems(
            Update,
            (
                screen::title_input.run_if(in_state(GameState::Title)),
                screen::credits_input.run_if(in_state(GameState::Credits)),
                screen::menu_input.run_if(in_state(GameState::Menu)),
            ),
        )
        // --- Gameplay ---
        .add_systems(OnEnter(GameState::Playing), setup_game)
        .add_systems(OnExit(GameState::Playing), (teardown_game, sfx::stop_all_sfx))
        .add_systems(
            FixedUpdate,
            (
                player::read_input,
                player::move_player_tick,
                enemy::move_walkers,
                // Runs before hazard_damage so contact tests see this
                // tick's positions rather than the previous one's.
                enemy_ai::tick_enemies,
                // Pounce resolves before contact damage so landing on an
                // enemy kills it instead of hurting the player.
                combat::pounce_enemies,
                combat::pounce_containers,
                enemy::hazard_damage,
                combat::place_bomb,
                combat::tick_bombs,
                effects::tick_explosions,
                combat::explosion_damage,
                effects::tick_decorations,
                effects::tick_score_effects,
                player::update_death,
                flow::collect_pickups,
                flow::check_level_exit,
                player::animate_player,
                sfx::play_queued_sfx,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                player::apply_player_frame,
                player::sync_transform,
                actors::animate_sprites,
                actors::track_player,
                camera::follow_player,
                level::scroll_backdrop,
                hud::update_status_bar,
                hud::fit_camera_to_play_area,
                audio::update_music,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );

    insert_core_resources(&mut app);
    app.run();
}

/// Resources that outlive any single screen: the converted game data, the
/// shared font atlas, and the camera the status bar renders through.
///
/// These are inserted directly on the world before the app runs rather than
/// from a `Startup` system, because Bevy runs the *initial* state
/// transition before `Startup`'s deferred commands are applied - so an
/// `OnEnter` system would find these missing and panic.
fn insert_core_resources(app: &mut App) {
    let assets_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    app.insert_resource(GameData::load(&assets_dir));

    let font_image = app
        .world()
        .resource::<AssetServer>()
        .load(data::asset_path("font.png"));
    let font_layout = app
        .world_mut()
        .resource_mut::<Assets<TextureAtlasLayout>>()
        .add(TextureAtlasLayout::from_grid(
            UVec2::splat(8),
            cosmo_assets::convert::FONT_ATLAS_COLS,
            10,
            None,
            None,
        ));
    app.insert_resource(hud::HudAssets {
        font_image,
        font_layout,
    });

    let ui_camera = hud::spawn_ui_camera_on(app.world_mut());
    app.insert_resource(UiCamera(ui_camera));
}

/// Everything that only exists while a level is being played.
fn setup_game(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    data: Res<GameData>,
    hud_assets: Res<hud::HudAssets>,
    ui_camera: Res<UiCamera>,
) {
    // Each episode names its levels differently, so the default start
    // comes from that episode's own progression rather than a literal.
    let start_level = std::env::var("COSMO_LEVEL").unwrap_or_else(|_| {
        data.level_order()
            .first()
            .cloned()
            .unwrap_or_else(|| START_LEVEL.to_string())
    });
    let tileset_assets = tileset::load_tileset(&asset_server, &mut layouts, &data);
    let current_level = flow::load_level_into_world(
        &mut commands,
        &asset_server,
        &data,
        &tileset_assets,
        &start_level,
    )
    .expect("start level missing from generated assets");

    let level_json = data.load_level(&start_level).unwrap();
    // COSMO_SPAWN=x,y overrides the level's own start point, which is how
    // situational behaviour (landing on a particular enemy, say) gets
    // exercised deterministically instead of hoping a playthrough wanders
    // into it.
    let (start_x, start_y) = std::env::var("COSMO_SPAWN")
        .ok()
        .and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .unwrap_or_else(|| level::find_player_start(&level_json));
    let player_frames = PlayerFrames::load(&asset_server, &data);
    let mut player = Player::spawn_at(start_x as i32, start_y as i32);
    // COSMO_GIVE_BOMBS stocks the bomb counter up front, so the bomb path
    // can be exercised without first hunting down a bomb pickup.
    if let Ok(n) = std::env::var("COSMO_GIVE_BOMBS") {
        player.bombs = n.trim().parse().unwrap_or(0);
    }
    let (player_health, player_cells, player_bombs) =
        (player.health, player.health_cells, player.bombs);
    commands.spawn((
        player,
        Sprite {
            image: player_frames.0.first().cloned().unwrap_or_default(),
            ..default()
        },
        Transform::default(),
    ));
    commands.insert_resource(flow::Checkpoint {
        score: 0,
        stars: 0,
        health: player_health,
        health_cells: player_cells,
        bombs: player_bombs,
    });
    commands.insert_resource(player_frames);
    commands.insert_resource(LevelSequence::build(&data, &start_level));
    commands.insert_resource(current_level);
    commands.insert_resource(tileset_assets);
    commands.insert_resource(effects::EffectAssets::load(&asset_server, &data));
    commands.insert_resource(sfx::SfxAssets::load(&asset_server, &data));

    hud::spawn_hud(
        &mut commands,
        &hud_assets,
        asset_server.load(data::asset_path("status_bar.png")),
        ui_camera.0,
    );

    camera::spawn_camera(&mut commands);
}

fn teardown_game(
    mut commands: Commands,
    scoped: Query<Entity, With<level::LevelScoped>>,
    players: Query<Entity, With<Player>>,
    status_bar: Query<Entity, With<hud::StatusBarUi>>,
    game_camera: Query<Entity, With<camera::GameCamera>>,
) {
    for entity in scoped
        .iter()
        .chain(players.iter())
        .chain(status_bar.iter())
        .chain(game_camera.iter())
    {
        commands.entity(entity).despawn();
    }
}
