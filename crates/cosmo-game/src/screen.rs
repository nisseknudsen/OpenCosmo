//! Title screen and main menu, drawn with the game's own artwork and font.
//!
//! The original shows TITLE1.MNI and waits, then presents a text-framed
//! main menu (`DrawMainMenu`, game2.c:3615-3641). Its full list also
//! includes Restore/Story/Instructions/High Scores/Game Redefine/Ordering
//! Info/Apogee's BBS/Demo - all of which depend on subsystems this remake
//! hasn't ported (save states, the text-page viewer, the recorded demo
//! player), so only the entries that actually do something are listed here.

use crate::hud::{HudAssets, HUD_RENDER_LAYER};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

/// The original's virtual screen, in 8px tiles: 320x200.
const SCREEN_W_TILES: f32 = 40.0;
const SCREEN_H_TILES: f32 = 25.0;

/// Font tile for the solid panel background (FONT_BACKGROUND_GRAY =
/// byte offset 0x0f28 / 40, graphics.h:71).
const FONT_BACKGROUND_GRAY: usize = 97;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Title,
    Menu,
    Credits,
    Playing,
}

/// Everything belonging to the current non-gameplay screen; despawned
/// wholesale when leaving that state.
#[derive(Component)]
pub struct ScreenUi;

/// Maps a character to its font tile.
///
/// The font runs in ASCII order but skips the six symbols between `Z` and
/// `a`, which is why the two ranges need different offsets. Anchors that
/// pin it down: FONT_0 = 0x0410/40 = tile 26 = `'0'` (ASCII 48) and
/// FONT_LOWER_A = 0x0ac8/40 = tile 69 = `'a'` (ASCII 97).
pub fn font_tile_for_char(c: char) -> Option<usize> {
    match c {
        ' '..='Z' => Some(c as usize - 22),
        'a'..='z' => Some(c as usize - 28),
        _ => None,
    }
}

fn tile_node(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(x / SCREEN_W_TILES * 100.0),
        top: Val::Percent(y / SCREEN_H_TILES * 100.0),
        width: Val::Percent(w / SCREEN_W_TILES * 100.0),
        height: Val::Percent(h / SCREEN_H_TILES * 100.0),
        ..default()
    }
}

fn font_image(hud: &HudAssets, index: usize) -> ImageNode {
    ImageNode::from_atlas_image(
        hud.font_image.clone(),
        TextureAtlas {
            layout: hud.font_layout.clone(),
            index,
        },
    )
}

/// Draws `text` one font tile per character starting at tile (x, y).
fn spawn_text(parent: &mut ChildSpawnerCommands, hud: &HudAssets, x: f32, y: f32, text: &str) {
    for (i, c) in text.chars().enumerate() {
        let Some(tile) = font_tile_for_char(c) else {
            continue;
        };
        parent.spawn((font_image(hud, tile), tile_node(x + i as f32, y, 1.0, 1.0)));
    }
}

/// Each screen owns its camera rather than sharing the status bar's.
///
/// The initial `OnEnter` transition can run before a `Startup` system's
/// deferred `insert_resource` has been applied, so depending on a
/// globally-inserted camera resource here would be a startup race. Owning
/// the camera also means it can clear to black - during these screens there
/// is no game camera underneath to preserve.
fn spawn_screen_camera(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            ScreenUi,
            Camera2d,
            Camera {
                order: 1,
                ..default()
            },
            RenderLayers::layer(HUD_RENDER_LAYER),
        ))
        .id()
}

/// A full-window container holding a 320x200-proportioned screen, so the
/// artwork keeps its aspect instead of stretching to the window.
fn screen_root(ui_camera: Entity) -> impl Bundle {
    (
        ScreenUi,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::BLACK),
        UiTargetCamera(ui_camera),
        RenderLayers::layer(HUD_RENDER_LAYER),
    )
}

fn screen_panel() -> Node {
    Node {
        height: Val::Percent(100.0),
        max_width: Val::Percent(100.0),
        aspect_ratio: Some(SCREEN_W_TILES / SCREEN_H_TILES),
        position_type: PositionType::Relative,
        ..default()
    }
}

fn spawn_fullscreen_image(commands: &mut Commands, asset_server: &AssetServer, path: &str) {
    let camera = spawn_screen_camera(commands);
    commands
        .spawn(screen_root(camera))
        .with_children(|root| {
            root.spawn((ImageNode::new(asset_server.load(path)), screen_panel()));
        });
}

pub fn spawn_title(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_fullscreen_image(&mut commands, &asset_server, &crate::data::asset_path("screens/title.png"));
}

pub fn spawn_credits(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_fullscreen_image(&mut commands, &asset_server, &crate::data::asset_path("screens/credit.png"));
}

/// Any key returns to the menu.
pub fn credits_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.get_just_pressed().next().is_some() {
        next.set(GameState::Menu);
    }
}

/// The menu entries we actually implement, in the original's own order.
const MENU_ITEMS: &[(&str, MenuAction)] = &[
    (" B)egin New Game", MenuAction::Begin),
    (" C)redits", MenuAction::Credits),
    (" T)itle Screen", MenuAction::Title),
    (" Q)uit Game", MenuAction::Quit),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Begin,
    Credits,
    Title,
    Quit,
}

/// Panel geometry, mirroring `UnfoldTextFrame(2, 20, 20, "MAIN MENU", "")`:
/// 20 tiles wide, starting at row 2, centred horizontally.
const PANEL_X: f32 = 10.0;
const PANEL_Y: f32 = 2.0;
const PANEL_W: f32 = 20.0;
const PANEL_H: f32 = 13.0;

pub fn spawn_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    hud: Res<HudAssets>,
) {
    let camera = spawn_screen_camera(&mut commands);
    commands
        .spawn(screen_root(camera))
        .with_children(|root| {
            root.spawn(screen_panel()).with_children(|screen| {
                // The original draws the menu frame over whatever is
                // already on screen, which coming from the title is the
                // title art.
                screen.spawn((
                    ImageNode::new(asset_server.load(&crate::data::asset_path("screens/title.png"))),
                    tile_node(0.0, 0.0, SCREEN_W_TILES, SCREEN_H_TILES),
                ));
                // The frame's solid background. One stretched copy of the
                // font's gray tile rather than a grid of them - it's a flat
                // colour, so stretching is identical to tiling and avoids
                // seams from per-tile percentage rounding.
                screen.spawn((
                    font_image(&hud, FONT_BACKGROUND_GRAY),
                    tile_node(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
                ));
                spawn_text(screen, &hud, PANEL_X + 5.0, PANEL_Y + 1.0, "MAIN MENU");
                for (i, (label, _)) in MENU_ITEMS.iter().enumerate() {
                    spawn_text(
                        screen,
                        &hud,
                        PANEL_X + 1.0,
                        PANEL_Y + 3.0 + i as f32 * 2.0,
                        label,
                    );
                }
            });
        });
}

/// The UI camera the in-game status bar renders through.
#[derive(Resource)]
pub struct UiCamera(pub Entity);

pub fn title_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.get_just_pressed().next().is_some() {
        next.set(GameState::Menu);
    }
}

pub fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: EventWriter<AppExit>,
) {
    let action = if keys.just_pressed(KeyCode::KeyB) || keys.just_pressed(KeyCode::Enter) {
        Some(MenuAction::Begin)
    } else if keys.just_pressed(KeyCode::KeyC) {
        Some(MenuAction::Credits)
    } else if keys.just_pressed(KeyCode::KeyT) {
        Some(MenuAction::Title)
    } else if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Escape) {
        Some(MenuAction::Quit)
    } else {
        None
    };

    match action {
        Some(MenuAction::Begin) => next.set(GameState::Playing),
        Some(MenuAction::Credits) => next.set(GameState::Credits),
        Some(MenuAction::Title) => next.set(GameState::Title),
        Some(MenuAction::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

pub fn despawn_screen(mut commands: Commands, query: Query<Entity, With<ScreenUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
