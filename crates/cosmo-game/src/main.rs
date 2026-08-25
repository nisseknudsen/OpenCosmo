mod actors;
mod audio;
mod camera;
mod data;
mod enemy;
mod flow;
mod hud;
mod level;
mod player;
mod tileset;

use bevy::prelude::*;
use bevy::time::Fixed;
use data::GameData;
use flow::LevelSequence;
use player::{Player, PlayerFrames, PlayerInput};

const START_LEVEL: &str = "a1";

fn main() {
    App::new()
        .add_plugins(
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
        .init_resource::<PlayerInput>()
        .init_resource::<flow::Score>()
        .init_resource::<flow::Stars>()
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (
                player::read_input,
                player::move_player_tick,
                enemy::move_walkers,
                enemy::hazard_damage,
                player::update_death,
                flow::collect_pickups,
                flow::smash_containers,
                flow::check_level_exit,
                player::animate_player,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                player::apply_player_frame,
                player::sync_transform,
                camera::follow_player,
                level::scroll_backdrop,
                hud::update_hud,
                audio::update_music,
            )
                .chain(),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let assets_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let data = GameData::load(&assets_dir);

    let start_level = std::env::var("COSMO_LEVEL").unwrap_or_else(|_| START_LEVEL.to_string());
    let tileset_assets = tileset::load_tileset(&asset_server, &mut layouts, &data);
    let current_level =
        flow::load_level_into_world(&mut commands, &asset_server, &data, &tileset_assets, &start_level)
            .expect("start level missing from generated assets");

    let level_json = data.load_level(&start_level).unwrap();
    let (start_x, start_y) = level::find_player_start(&level_json);
    let player_frames = PlayerFrames::load(&asset_server, &data);
    commands.spawn((
        Player::spawn_at(start_x as i32, start_y as i32),
        Sprite {
            image: player_frames.0.first().cloned().unwrap_or_default(),
            ..default()
        },
        Transform::default(),
    ));
    commands.insert_resource(player_frames);
    commands.insert_resource(LevelSequence::build(&data, &start_level));
    commands.insert_resource(current_level);
    commands.insert_resource(tileset_assets);
    commands.insert_resource(data);

    hud::spawn_hud(&mut commands);
    camera::spawn_camera(commands);
}
