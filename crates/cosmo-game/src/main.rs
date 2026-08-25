mod camera;
mod data;
mod level;
mod player;
mod tileset;

use bevy::prelude::*;
use bevy::time::Fixed;
use data::GameData;
use level::CurrentLevel;
use player::{Player, PlayerFrames, PlayerInput};
use tileset::TilesetAssets;

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
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (
                player::read_input,
                player::move_player_tick,
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

    let level = data
        .load_level(START_LEVEL)
        .expect("start level missing from generated assets");

    let tileset_assets = tileset::load_tileset(&asset_server, &mut layouts, &data);
    level::spawn_level_tiles(&mut commands, &tileset_assets, &level, &data);

    let (start_x, start_y) = level::find_player_start(&level);
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

    commands.insert_resource(CurrentLevel {
        name: START_LEVEL.to_string(),
        width: level.width,
        height: level.height,
    });
    commands.insert_resource(tileset_assets);
    commands.insert_resource(data);

    camera::spawn_camera(commands);
}
