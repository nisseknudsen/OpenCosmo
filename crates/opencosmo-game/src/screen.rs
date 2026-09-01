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
pub(crate) const SCREEN_W_TILES: f32 = 40.0;
pub(crate) const SCREEN_H_TILES: f32 = 25.0;

/// Font tile for the solid panel background (FONT_BACKGROUND_GRAY =
/// byte offset 0x0f28 / 40, graphics.h:71).
pub(crate) const FONT_BACKGROUND_GRAY: usize = 97;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Title,
    Menu,
    Credits,
    Controls,
    Episodes,
    TextPages,
    HighScores,
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
///
/// One quirk: the slot where `'/'` belongs (tile 25) holds a pound-sign-ish
/// glyph in the shipped font, so a slash renders as garbage. Its neighbours
/// `,` `-` `.` are all correct, so this is the artwork rather than the
/// mapping. Avoid `/` in text we author.
pub fn font_tile_for_char(c: char) -> Option<usize> {
    match c {
        ' '..='Z' => Some(c as usize - 22),
        'a'..='z' => Some(c as usize - 28),
        _ => None,
    }
}

pub(crate) fn tile_node(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(x / SCREEN_W_TILES * 100.0),
        top: Val::Percent(y / SCREEN_H_TILES * 100.0),
        width: Val::Percent(w / SCREEN_W_TILES * 100.0),
        height: Val::Percent(h / SCREEN_H_TILES * 100.0),
        ..default()
    }
}

pub(crate) fn font_image(hud: &HudAssets, index: usize) -> ImageNode {
    ImageNode::from_atlas_image(
        hud.font_image.clone(),
        TextureAtlas {
            layout: hud.font_layout.clone(),
            index,
        },
    )
}

/// Draws `text` one font tile per character starting at tile (x, y).
pub(crate) fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    hud: &HudAssets,
    x: f32,
    y: f32,
    text: &str,
) {
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
pub fn spawn_state_camera(commands: &mut Commands, screen: &crate::presentation::VirtualScreen) -> Entity {
    commands
        .spawn((
            ScreenUi,
            Camera2d,
            Camera {
                target: screen.target(),
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

pub(crate) fn screen_panel() -> Node {
    Node {
        height: Val::Percent(100.0),
        max_width: Val::Percent(100.0),
        aspect_ratio: Some(SCREEN_W_TILES / SCREEN_H_TILES),
        position_type: PositionType::Relative,
        ..default()
    }
}

fn spawn_fullscreen_image(
    commands: &mut Commands,
    asset_server: &AssetServer,
    screen: &crate::presentation::VirtualScreen,
    path: &str,
) {
    let camera = spawn_state_camera(commands, screen);
    commands
        .spawn(screen_root(camera))
        .with_children(|root| {
            root.spawn((ImageNode::new(asset_server.load(path)), screen_panel()));
        });
}

pub fn spawn_title(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    screen: Res<crate::presentation::VirtualScreen>,
) {
    let path = crate::data::asset_path("screens/title.png");
    spawn_fullscreen_image(&mut commands, &asset_server, &screen, &path);
}

pub fn spawn_credits(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    screen: Res<crate::presentation::VirtualScreen>,
) {
    let path = crate::data::asset_path("screens/credit.png");
    spawn_fullscreen_image(&mut commands, &asset_server, &screen, &path);
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
    // The original calls this "G)ame Redefine" (game2.c:3629) and splits it
    // into keyboard and joystick screens; one screen covers both here.
    (" G)ame Redefine", MenuAction::Controls),
    // No counterpart in the original, which ships as three separate
    // executables. A remake carrying all three needs a way to choose.
    (" E)pisode", MenuAction::Episodes),
    (" S)tory", MenuAction::Story),
    (" I)nstructions", MenuAction::Instructions),
    (" O)rdering Info.", MenuAction::Ordering),
    (" A)pogee's BBS", MenuAction::Bbs),
    (" H)igh Scores", MenuAction::HighScores),
    (" C)redits", MenuAction::Credits),
    (" T)itle Screen", MenuAction::Title),
    (" Q)uit Game", MenuAction::Quit),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Begin,
    Episodes,
    Story,
    Instructions,
    Ordering,
    Bbs,
    HighScores,
    Controls,
    Credits,
    Title,
    Quit,
}

/// Panel geometry, mirroring `UnfoldTextFrame(2, 20, 20, "MAIN MENU", "")`:
/// 20 tiles wide, starting at row 2, centred horizontally.
const PANEL_X: f32 = 10.0;
const PANEL_Y: f32 = 2.0;
const PANEL_W: f32 = 20.0;
const PANEL_H: f32 = 15.0;

pub fn spawn_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    hud: Res<HudAssets>,
    screen: Res<crate::presentation::VirtualScreen>,
) {
    let camera = spawn_state_camera(&mut commands, &screen);
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
    mut pages: ResMut<TextPages>,
) {
    let action = if keys.just_pressed(KeyCode::KeyB) || keys.just_pressed(KeyCode::Enter) {
        Some(MenuAction::Begin)
    } else if keys.just_pressed(KeyCode::KeyG) {
        Some(MenuAction::Controls)
    } else if keys.just_pressed(KeyCode::KeyE) {
        Some(MenuAction::Episodes)
    } else if keys.just_pressed(KeyCode::KeyS) {
        Some(MenuAction::Story)
    } else if keys.just_pressed(KeyCode::KeyI) {
        Some(MenuAction::Instructions)
    } else if keys.just_pressed(KeyCode::KeyO) {
        Some(MenuAction::Ordering)
    } else if keys.just_pressed(KeyCode::KeyA) {
        Some(MenuAction::Bbs)
    } else if keys.just_pressed(KeyCode::KeyH) {
        Some(MenuAction::HighScores)
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
        Some(MenuAction::Controls) => next.set(GameState::Controls),
        Some(MenuAction::Episodes) => next.set(GameState::Episodes),
        Some(MenuAction::Story) => {
            pages.set(crate::textpages::STORY);
            next.set(GameState::TextPages);
        }
        Some(MenuAction::Instructions) => {
            pages.set(crate::textpages::INSTRUCTIONS);
            next.set(GameState::TextPages);
        }
        Some(MenuAction::Ordering) => {
            pages.set(crate::textpages::ORDERING);
            next.set(GameState::TextPages);
        }
        Some(MenuAction::Bbs) => {
            pages.set(crate::textpages::BBS);
            next.set(GameState::TextPages);
        }
        Some(MenuAction::HighScores) => next.set(GameState::HighScores),
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


/// Marks the episode chooser.
#[derive(Component)]
pub struct EpisodeUi;

/// The three episodes, by the names the game gives them.
const EPISODE_TITLES: [&str; 3] = [
    " 1) Forbidden Planet",
    " 2) Mad Scientist",
    " 3) Secret Sanctum",
];

pub fn spawn_episodes(
    mut commands: Commands,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<UiCamera>,
    data: Res<crate::data::GameData>,
) {
    let mut frame =
        crate::panel::TextFrame::new(4, 9, 26, "SELECT EPISODE", "ESC) Back");
    for (i, title) in EPISODE_TITLES.iter().enumerate() {
        let marker = if data.episode as usize == i + 1 { ">" } else { " " };
        frame = frame.text(7 + i as i32, &format!("{marker}{title}"));
    }
    frame.spawn(&mut commands, &hud, ui_camera.0, EpisodeUi);
}

pub fn episodes_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
    mut chosen: ResMut<crate::data::ChosenEpisode>,
) {
    let pick = if keys.just_pressed(KeyCode::Digit1) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(3)
    } else {
        None
    };
    if let Some(n) = pick {
        // Taken up by `apply_chosen_episode` on the way into a game, which
        // is the only point where swapping the assets under everything is
        // safe.
        chosen.0 = Some(n);
        next.set(GameState::Menu);
        return;
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyT) {
        next.set(GameState::Menu);
    }
}

pub fn despawn_episodes(mut commands: Commands, open: Query<Entity, With<EpisodeUi>>) {
    for entity in &open {
        commands.entity(entity).despawn();
    }
}


/// The paged sequence currently open, and how far through it we are.
#[derive(Resource)]
pub struct TextPages {
    pub pages: &'static [crate::textpages::TextPage],
    pub index: usize,
}

impl Default for TextPages {
    fn default() -> Self {
        // Something rather than nothing, so `COSMO_STATE=story` lands on a
        // real page instead of an empty frame.
        TextPages {
            pages: crate::textpages::STORY,
            index: 0,
        }
    }
}

impl TextPages {
    pub fn set(&mut self, pages: &'static [crate::textpages::TextPage]) {
        self.pages = pages;
        self.index = 0;
    }

    /// Advances, saturating at the last page. Returns false when there is
    /// nowhere further to go, which is what closes the sequence.
    pub fn next_page(&mut self) -> bool {
        if self.index + 1 >= self.pages.len() {
            return false;
        }
        self.index += 1;
        true
    }

    pub fn prev_page(&mut self) {
        self.index = self.index.saturating_sub(1);
    }
}

#[derive(Component)]
pub struct TextPageUi;

fn draw_text_page(
    commands: &mut Commands,
    hud: &crate::hud::HudAssets,
    ui_camera: Entity,
    pages: &TextPages,
    open: &Query<Entity, With<TextPageUi>>,
) {
    for entity in open.iter() {
        commands.entity(entity).despawn();
    }
    let Some(page) = pages.pages.get(pages.index) else {
        return;
    };
    let mut frame =
        crate::panel::TextFrame::new(page.top, page.height, page.width, page.title, page.bottom);
    let x0 = frame.text_x();
    for (dx, row, text) in page.lines {
        frame = frame.line(x0 + dx, *row, text);
    }
    frame.spawn(commands, hud, ui_camera, TextPageUi);
}

pub fn spawn_text_page(
    mut commands: Commands,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<UiCamera>,
    pages: Res<TextPages>,
    open: Query<Entity, With<TextPageUi>>,
) {
    draw_text_page(&mut commands, &hud, ui_camera.0, &pages, &open);
}

pub fn text_page_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut pages: ResMut<TextPages>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<UiCamera>,
    open: Query<Entity, With<TextPageUi>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Menu);
        return;
    }
    // PgUp goes back, as the instructions screen offers; anything else
    // goes forward, and running off the end returns to the menu - which is
    // what each `WaitSpinner` sequence in game2.c does.
    let back = keys.just_pressed(KeyCode::PageUp) || keys.just_pressed(KeyCode::ArrowLeft);
    if back {
        pages.prev_page();
    } else if keys.get_just_pressed().next().is_some() {
        if !pages.next_page() {
            next.set(GameState::Menu);
            return;
        }
    } else {
        return;
    }
    draw_text_page(&mut commands, &hud, ui_camera.0, &pages, &open);
}

pub fn despawn_text_pages(mut commands: Commands, open: Query<Entity, With<TextPageUi>>) {
    for entity in &open {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod text_page_tests {
    use super::*;
    use crate::textpages::{BBS, INSTRUCTIONS, ORDERING, STORY};

    #[test]
    fn every_sequence_has_pages_with_something_on_them() {
        for (name, seq) in [
            ("story", STORY),
            ("instructions", INSTRUCTIONS),
            ("ordering", ORDERING),
            ("bbs", BBS),
        ] {
            assert!(!seq.is_empty(), "{name} has no pages");
            for (i, page) in seq.iter().enumerate() {
                assert!(!page.lines.is_empty(), "{name} page {i} is blank");
                assert!(page.width > 0 && page.height > 0, "{name} page {i}");
            }
        }
    }

    #[test]
    fn no_page_still_carries_a_sprite_glyph() {
        // The source embeds cartoon panels in the strings as `\xFB000`
        // escapes. Those are pictures, not words, and leaking one would
        // print literal garbage into the frame.
        for seq in [STORY, INSTRUCTIONS, ORDERING, BBS] {
            for page in seq {
                for (_, _, text) in page.lines {
                    assert!(
                        !text.contains("\\x"),
                        "a glyph escape survived transcription: {text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn paging_stops_at_the_end_rather_than_running_off_it() {
        let mut p = TextPages::default();
        p.set(BBS);
        assert_eq!(p.index, 0);
        for i in 1..BBS.len() {
            assert!(p.next_page(), "should reach page {i}");
            assert_eq!(p.index, i);
        }
        assert!(!p.next_page(), "the last page reports there is no next");
        assert_eq!(p.index, BBS.len() - 1, "and does not step past it");
    }

    #[test]
    fn paging_back_stops_at_the_first_page() {
        let mut p = TextPages::default();
        p.set(STORY);
        p.next_page();
        p.prev_page();
        assert_eq!(p.index, 0);
        p.prev_page();
        assert_eq!(p.index, 0, "and does not underflow");
    }

    #[test]
    fn switching_sequences_starts_at_the_beginning() {
        let mut p = TextPages::default();
        p.set(ORDERING);
        p.next_page();
        p.set(BBS);
        assert_eq!(p.index, 0);
    }
}
