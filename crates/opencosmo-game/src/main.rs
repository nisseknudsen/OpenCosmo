mod actors;
mod audio;
mod camera;
mod combat;
mod controls_screen;
mod data;
mod devmenu;
mod effects;
mod enemy;
mod enemy_ai;
mod flow;
mod help;
mod hints;
mod hud;
mod input;
mod level;
mod motion;
mod panel;
mod pickups;
mod player;
mod presentation;
mod screen;
mod sfx;
mod tileset;
mod trace;

use bevy::audio::{AudioPlugin, GlobalVolume, Volume};
use bevy::prelude::*;
use bevy::time::Fixed;
use data::GameData;
use flow::LevelSequence;
use player::{Player, PlayerFrames, PlayerInput};
use screen::{GameState, UiCamera};

const START_LEVEL: &str = "a1";

/// The gameplay tick, in order.
///
/// Named sets rather than one long `.chain()`. The chain had to be split in
/// two to get under Bevy's 20-element tuple limit, and `.chain()` on the
/// outer tuple only orders the two halves against each other - every
/// constraint *inside* a half was silently dropped, which went unnoticed
/// until a position snapshot started running after the movement it was
/// supposed to precede. Sets cannot fail that way: the ordering is declared
/// once, and adding a system to a set cannot quietly reorder its
/// neighbours.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum Tick {
    /// Record where everything was, before anything moves.
    Snapshot,
    /// Player and actor movement.
    Movement,
    /// Pounces, bombs, blasts, contact damage.
    Combat,
    /// Explosions, debris, score pop-ups.
    Effects,
    /// Death, pickups, level exits, hint globes.
    Resolve,
    /// Sprite frame and scroll position for the tick just simulated.
    Present,
}

/// `COSMO_VSYNC=off` uncaps the frame rate.
///
/// Vsync is the right default - it is what stops tearing, and the game's
/// logic runs on its own fixed 18.2Hz clock regardless, so a higher frame
/// rate buys smoother *presentation* and nothing else. Uncapped is mostly
/// useful for measuring what a frame actually costs, since a vsynced frame
/// that misses its deadline reports the next multiple down and tells you
/// nothing about by how much.
fn present_mode() -> bevy::window::PresentMode {
    match std::env::var("COSMO_VSYNC").as_deref() {
        Ok("off") | Ok("no") => bevy::window::PresentMode::AutoNoVsync,
        _ => bevy::window::PresentMode::AutoVsync,
    }
}

/// `COSMO_HEADLESS=1` runs the game without showing a window and without
/// making a sound, so a verification run can happen while the machine is
/// being used for something else.
///
/// It is deliberately *not* a separate code path: the window is still
/// created and still renders, so screenshots, the present shader and every
/// system behave exactly as they do normally - the window is simply never
/// mapped, and the audio output's global volume is zero. A mode that
/// skipped rendering would not be testing the thing that ships.
fn headless() -> bool {
    matches!(std::env::var("COSMO_HEADLESS").as_deref(), Ok("1") | Ok("on") | Ok("yes"))
}

/// The two things headless mode actually changes, as plain functions of the
/// flag so they can be asserted without standing up an app, a window or an
/// audio device - which is the whole point of the mode.
fn primary_window(headless: bool) -> Window {
    Window {
        title: "OpenCosmo".into(),
        resolution: window_size(),
        present_mode: present_mode(),
        visible: !headless,
        ..default()
    }
}

fn global_volume(headless: bool) -> GlobalVolume {
    GlobalVolume::new(if headless {
        Volume::SILENT
    } else {
        Volume::Linear(1.0)
    })
}

/// `COSMO_SPEED=8` runs the clock faster, so a hundred-tick check does not
/// take five and a half seconds of wall time. It scales virtual time, which
/// is what `Time<Fixed>` accumulates from, so the simulation still sees the
/// same 18.2Hz steps - there are just more of them per real second.
fn time_scale() -> Option<f32> {
    std::env::var("COSMO_SPEED")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|s| *s > 0.0 && (*s - 1.0).abs() > f32::EPSILON)
}

/// `COSMO_WINDOW=1280x800` sets the starting window size. Mostly for
/// like-for-like performance comparisons: a window the compositor decides
/// to maximise has several times the pixel count of one it leaves alone,
/// and the present shader's cost is per output pixel.
fn window_size() -> bevy::window::WindowResolution {
    let parsed = std::env::var("COSMO_WINDOW").ok().and_then(|v| {
        let (w, h) = v.split_once(['x', 'X'])?;
        Some((w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?))
    });
    let (w, h) = parsed.unwrap_or((1024.0, 640.0));
    bevy::window::WindowResolution::new(w, h)
}

/// Applies `COSMO_SPEED` once the app is up.
fn apply_time_scale(mut virt: ResMut<Time<Virtual>>) {
    if let Some(scale) = time_scale() {
        virt.set_relative_speed(scale);
        info!("clock running at {scale}x");
    }
}

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
                    primary_window: Some(primary_window(headless())),
                    ..default()
                })
                .set(AudioPlugin {
                    global_volume: global_volume(headless()),
                    ..default()
                }),
        )
        .add_plugins(presentation::PresentationPlugin)
        .insert_resource(Time::<Fixed>::from_hz(18.2))
        .add_systems(Startup, apply_time_scale)
        // COSMO_STATE lets a dev (or a headless screenshot run) jump
        // straight to a screen instead of clicking through the title.
        .insert_state(match std::env::var("COSMO_STATE").as_deref() {
            Ok("menu") => GameState::Menu,
            Ok("credits") => GameState::Credits,
            Ok("controls") => GameState::Controls,
            Ok("playing") => GameState::Playing,
            _ => GameState::Title,
        })
        .init_resource::<PlayerInput>()
        .insert_resource(input::load_bindings())
        .init_resource::<input::InputAccumulator>()
        .init_resource::<controls_screen::Rebinding>()
        .init_resource::<flow::Score>()
        .init_resource::<flow::Stars>()
        .init_resource::<flow::Checkpoint>()
        .insert_resource(trace::Hooks::from_env())
        .insert_resource(motion::MotionOverride::from_env())
        .init_resource::<help::Paused>()
        .init_resource::<audio::AudioMode>()
        .init_resource::<camera::Scroll>()
        .init_resource::<motion::PrevScroll>()
        .init_resource::<hints::NearHintGlobe>()
        .init_resource::<hints::SawAutoHintGlobe>()
        .init_resource::<hints::HintLatch>()
        .init_resource::<level::TileIndex>()
        .init_resource::<enemy_ai::SwitchState>()
        .init_resource::<enemy_ai::TransporterState>()
        .init_resource::<flow::LevelIntroTimer>()
        .init_resource::<devmenu::WarpCursor>()
        .add_event::<flow::RestartLevel>()
        .add_event::<flow::EnterLevel>()
        .add_event::<flow::LevelFinished>()
        .init_resource::<flow::PendingLevel>()
        .add_event::<devmenu::OpenLevelWarp>()
        .init_resource::<sfx::SfxState>()
        .add_event::<sfx::PlaySfx>()
        // --- Title / menu / credits ---
        .add_systems(OnEnter(GameState::Title), screen::spawn_title)
        .add_systems(OnExit(GameState::Title), screen::despawn_screen)
        .add_systems(OnEnter(GameState::Menu), screen::spawn_menu)
        .add_systems(OnExit(GameState::Menu), screen::despawn_screen)
        .add_systems(OnEnter(GameState::Credits), screen::spawn_credits)
        .add_systems(OnExit(GameState::Credits), screen::despawn_screen)
        .add_systems(OnEnter(GameState::Controls), controls_screen::spawn_controls)
        .add_systems(OnExit(GameState::Controls), screen::despawn_screen)
        .add_systems(
            Update,
            (
                screen::title_input.run_if(in_state(GameState::Title)),
                screen::credits_input.run_if(in_state(GameState::Credits)),
                screen::menu_input.run_if(in_state(GameState::Menu)),
                controls_screen::controls_input.run_if(in_state(GameState::Controls)),
                // Sampled every frame, drained by the gameplay tick - see
                // `input.rs` for why the 18.2Hz tick can't read keys itself.
                input::sample_input,
                audio::toggle_audio_mode,
            ),
        )
        // --- Gameplay ---
        .add_systems(OnEnter(GameState::Playing), setup_game)
        .add_systems(
            OnExit(GameState::Playing),
            (
                teardown_game,
                sfx::stop_all_sfx,
                help::close_help,
                hints::clear_hints,
                flow::clear_intermission,
                devmenu::close_level_warp,
            ),
        )
        .configure_sets(
            FixedUpdate,
            (
                Tick::Snapshot,
                Tick::Movement,
                Tick::Combat,
                Tick::Effects,
                Tick::Resolve,
                Tick::Present,
            )
                .chain()
                .after(player::read_input)
                .run_if(in_state(GameState::Playing).and(help::not_paused)),
        )
        .add_systems(FixedUpdate, motion::snapshot_positions.in_set(Tick::Snapshot))
        .add_systems(
            FixedUpdate,
            (
                player::move_player_tick,
                enemy::move_walkers,
                // Runs before hazard_damage so contact tests see this
                // tick's positions rather than the previous one's.
                enemy_ai::tick_enemies,
                // Builds whatever the behaviors asked for - a turret's
                // projectile, a hatching egg's ghost. After the ticks, so
                // a thing spawned this tick first moves on the next one,
                // as it does in the original.
                enemy_ai::spawn_queued_actors,
                // Force fields are beams rather than bodies, so their
                // drawing and their damage live outside the tick that
                // measures them.
                enemy_ai::draw_force_field_beams,
                enemy_ai::run_transporters,
                enemy_ai::run_pipes,
                level::move_platforms,
                level::apply_light_switch,
                enemy_ai::finish_on_boss_defeat,
            )
                .chain()
                .in_set(Tick::Movement),
        )
        .add_systems(
            FixedUpdate,
            (
                // Pounce resolves before contact damage so landing on an
                // enemy kills it instead of hurting the player.
                combat::pounce_enemies,
                combat::pounce_containers,
                enemy::hazard_damage,
                combat::place_bomb,
                combat::tick_bombs,
            )
                .chain()
                .in_set(Tick::Combat),
        )
        .add_systems(
            FixedUpdate,
            (
                effects::tick_explosions,
                combat::explosion_damage,
                combat::explosion_bursts_containers,
                actors::collapse_pedestals,
                effects::tick_decorations,
                effects::tick_score_effects,
            )
                .chain()
                .in_set(Tick::Effects),
        )
        .add_systems(
            FixedUpdate,
            (
                player::update_death,
                // Markers follow whatever moved this tick before anything
                // tests against them.
                actors::sync_actor_positions,
                flow::collect_pickups,
                flow::check_level_exit,
                // Globe proximity is settled before the frame/scroll pass,
                // which needs it to know whether looking up pans the view
                // or reads the globe.
                hints::detect_hint_globe,
            )
                .chain()
                .in_set(Tick::Resolve),
        )
        .add_systems(
            FixedUpdate,
            (
                player::update_frame_and_scroll,
                hints::read_hint_globe,
                sfx::play_queued_sfx,
            )
                .chain()
                .in_set(Tick::Present),
        )
        // Input and tracing keep running while a text frame is up: a
        // scripted run needs to be able to dismiss one, and a trace that
        // went silent on pause would look like a hang.
        .add_systems(
            FixedUpdate,
            (
                player::read_input,
                trace::trace_tick.run_if(trace::trace_enabled),
            )
                .run_if(in_state(GameState::Playing)),
        )
        // Capture and auto-quit are not gameplay - they have to work on the
        // menus too, which is where several of the things worth checking are.
        .add_systems(Update, (trace::screenshot_at, trace::report_frame_rate, trace::quit_after))
        .add_systems(PostUpdate, trace::trace_drawn_position)
        .add_systems(
            Update,
            (
                player::apply_player_frame,
                player::sync_transform,
                player::sync_player_visibility,
                actors::animate_sprites,
                actors::track_player,
                camera::apply_scroll,
                // Overrides the snapped positions above with interpolated
                // ones. Ordered after them so either can be the last word.
                (
                    motion::interpolate_player,
                    motion::interpolate_enemies,
                    motion::interpolate_scroll,
                )
                    .run_if(motion::interpolation_enabled),
                level::scroll_backdrop,
                hud::update_status_bar,
                audio::update_music,
            )
                .chain()
                .run_if(in_state(GameState::Playing).and(help::not_paused)),
        )
        // Modal frames must keep running while everything else is paused.
        //
        // The level (re)loads live here too, after the systems that ask for
        // them. They cannot sit in `FixedUpdate`: an event written from
        // `Update` (the warp menu) would have to survive until the next
        // fixed tick, and at 18.2Hz against a 60Hz frame rate the event
        // buffers are swapped out from under it first. The other direction
        // is safe - `FixedUpdate` runs earlier in the same frame - so
        // `update_death`'s restart request still arrives.
        .add_systems(
            Update,
            (
                help::help_menu_input,
                devmenu::open_level_warp,
                devmenu::level_warp_input,
                hints::close_hint,
                flow::show_intermission,
                flow::close_intermission,
                flow::restart_level,
                flow::enter_level,
                // After enter_level, so the intro is raised for the level
                // that was just loaded rather than the one being left.
                flow::show_level_intro,
                flow::close_level_intro,
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
    presentation::insert_virtual_screen(app);

    let font_image = app
        .world()
        .resource::<AssetServer>()
        .load(data::asset_path("font.png"));
    let font_layout = app
        .world_mut()
        .resource_mut::<Assets<TextureAtlasLayout>>()
        .add(TextureAtlasLayout::from_grid(
            UVec2::splat(8),
            opencosmo_assets::convert::FONT_ATLAS_COLS,
            10,
            None,
            None,
        ));
    app.insert_resource(hud::HudAssets {
        font_image,
        font_layout,
    });

    let target = app.world().resource::<presentation::VirtualScreen>().target();
    let ui_camera = hud::spawn_ui_camera_on(app.world_mut(), target);
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
    mut scroll: ResMut<camera::Scroll>,
    mut saw_auto: ResMut<hints::SawAutoHintGlobe>,
    mut images: ResMut<Assets<Image>>,
    mut switches: ResMut<enemy_ai::SwitchState>,
    mut tile_index: ResMut<level::TileIndex>,
    screen: Res<presentation::VirtualScreen>,
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
        &mut tile_index,
        &mut switches,
        &mut images,
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
    // `SPA_PLAYER_START` frames the view around the player as the map is
    // read (game1.c:10195-10209); from here on the scroll is carried
    // forward tick to tick rather than recomputed.
    scroll.centre_on(&player, &current_level);
    saw_auto.0 = false;
    commands.spawn((
        player,
        motion::PrevPos { x: start_x as i32, y: start_y as i32 },
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

    camera::spawn_camera(&mut commands, &screen);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_mode_shows_no_window_and_makes_no_sound() {
        assert!(!primary_window(true).visible, "headless must not map a window");
        assert_eq!(
            global_volume(true).volume,
            Volume::SILENT,
            "headless must be silent"
        );
    }

    #[test]
    fn a_normal_run_is_visible_and_audible() {
        // The control: without this, a mode that silenced *everything*
        // would pass the test above and still be wrong.
        assert!(primary_window(false).visible);
        assert_eq!(global_volume(false).volume, Volume::Linear(1.0));
    }
}
