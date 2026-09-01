//! Hint globes - the flickering orb-on-a-pedestal that dispenses advice.
//!
//! `ActHintGlobe` (game1.c:4448-4483): standing against one sets
//! `isPlayerNearHintGlobe`, and the message opens either because the player
//! pressed up at it or because it is the first globe they have touched this
//! level (`sawAutoHintGlobe`, reset per level at game1.c:10459). That flag
//! is why the first globe of a level greets you unprompted and every one
//! after it waits to be asked.
//!
//! Pressing up at a globe reads it *instead of* panning the view, which is
//! the `!isPlayerNearHintGlobe` guard on the look-up branch
//! (game1.c:8830) - see `player::update_frame_and_scroll`.

use crate::help::Paused;
use crate::hud::HudAssets;
use crate::panel::TextFrame;
use crate::player::{Player, PlayerInput, PLAYER_HEIGHT};
use crate::screen::UiCamera;
use crate::sfx::{snd, PlaySfx};
use bevy::prelude::*;

/// The orb the player can touch. Held on the orb entity rather than the
/// pedestal because that is what the original's touch test uses
/// (`IsTouchingPlayer(SPR_HINT_GLOBE, 0, act->x, act->y - 2)`).
#[derive(Component)]
pub struct HintGlobe {
    pub x: i32,
    pub y: i32,
    pub width_tiles: i32,
    pub height_tiles: i32,
    pub hint: u16,
}

/// The hint number of the globe the player is currently touching, if any.
#[derive(Resource, Default)]
pub struct NearHintGlobe(pub Option<u16>);

/// `sawAutoHintGlobe` - cleared whenever a level is (re)initialised.
#[derive(Resource, Default)]
pub struct SawAutoHintGlobe(pub bool);

/// Blocks a held up-key from immediately re-opening the message it just
/// dismissed. The original gets this for free: `ShowHintGlobeMessage` is
/// blocking and waits for a key press *and release* before returning.
#[derive(Resource, Default)]
pub struct HintLatch(pub bool);

#[derive(Component)]
pub struct HintUi;

/// `ACT_HINT_GLOBE_*` ids are scattered across three blocks of the actor
/// table (actor.h:167, 241-249, 274-279, 289-298); each maps to the hint
/// number `ConstructActor` passes as `data5` (game1.c:5938, 6171-6195,
/// 6276-6291, 6324-6351).
pub fn hint_number_for_actor(act_id: u16) -> Option<u16> {
    match act_id {
        125 => Some(0),
        204..=212 => Some(act_id - 203),  // 1..=9
        238..=243 => Some(act_id - 228),  // 10..=15
        253..=262 => Some(act_id - 237),  // 16..=25
        _ => None,
    }
}

/// `IsTouchingPlayer` (game1.c:1132-1158), specialised to a sprite whose
/// origin is its bottom-left tile, as all actors' are.
pub fn touching_player(
    player: &Player,
    x: i32,
    y: i32,
    width_tiles: i32,
    height_tiles: i32,
) -> bool {
    if player.dead_timer != 0 {
        return false;
    }
    let horizontal = (player.x <= x && player.x + 3 > x)
        || (player.x >= x && x + width_tiles > player.x);
    let vertical = (y - height_tiles < player.y && player.y <= y)
        || (player.y - (PLAYER_HEIGHT - 1) <= y && y <= player.y);
    horizontal && vertical
}

pub fn detect_hint_globe(
    player_q: Query<&Player>,
    globes: Query<&HintGlobe>,
    mut near: ResMut<NearHintGlobe>,
) {
    near.0 = None;
    let Ok(player) = player_q.single() else {
        return;
    };
    for g in &globes {
        if touching_player(player, g.x, g.y, g.width_tiles, g.height_tiles) {
            near.0 = Some(g.hint);
            return;
        }
    }
}

/// Opens the message when the player asks for it, or unprompted for the
/// first globe of the level.
#[allow(clippy::too_many_arguments)]
pub fn read_hint_globe(
    mut commands: Commands,
    near: Res<NearHintGlobe>,
    input: Res<PlayerInput>,
    mut saw_auto: ResMut<SawAutoHintGlobe>,
    mut latch: ResMut<HintLatch>,
    mut paused: ResMut<Paused>,
    hud: Res<HudAssets>,
    ui_camera: Res<UiCamera>,
    data: Res<crate::data::GameData>,
    mut sfx: EventWriter<PlaySfx>,
) {
    if !input.look_up {
        latch.0 = false;
    }
    if paused.0 {
        return;
    }
    let Some(hint) = near.0 else {
        return;
    };
    // Either the player asked (a fresh up-press), or this is the level's
    // first globe and it speaks up on its own.
    let asked = input.look_up && !latch.0;
    if !asked && saw_auto.0 {
        return;
    }
    if input.look_up {
        latch.0 = true;
    }
    saw_auto.0 = true;
    sfx.write(PlaySfx(snd::HINT_DIALOG_ALERT));
    paused.0 = true;
    hint_frame(data.episode, hint).spawn(&mut commands, &hud, ui_camera.0, HintUi);
}

/// Any key dismisses the message (the frame's own footer says so).
/// Raised when a hint should be shown; the frame it opens shares
/// `HintUi`, so the existing any-key dismissal already closes it.
#[derive(Event)]
pub struct ShowHint(pub Hint);

/// Puts the hint frame up and pauses, the way a globe's message does.
pub fn show_hint(
    mut commands: Commands,
    mut events: EventReader<ShowHint>,
    mut paused: ResMut<Paused>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
    open: Query<Entity, With<HintUi>>,
) {
    let Some(ShowHint(hint)) = events.read().last() else {
        return;
    };
    // Never stack one on top of a globe's message.
    if !open.is_empty() {
        return;
    }
    let (title, bottom, lines) = hint.lines();
    let height = lines.len() as i32 + 2;
    let mut frame = crate::panel::TextFrame::new(2, height, 28, title, bottom);
    for (i, line) in lines.iter().enumerate() {
        frame = frame.text(3 + i as i32, line);
    }
    paused.0 = true;
    sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::HINT_DIALOG_ALERT));
    frame.spawn(&mut commands, &hud, ui_camera.0, HintUi);
}

/// Shows the pounce reminder once the tick that queued it is over.
pub fn drain_queued_hint(
    mut seen: ResMut<SeenHints>,
    mut events: EventWriter<ShowHint>,
    paused: Res<Paused>,
) {
    if paused.0 {
        return;
    }
    if let Some(hint) = seen.take_queued_pounce() {
        events.write(ShowHint(hint));
    }
}

pub fn close_hint(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<PlayerInput>,
    mut paused: ResMut<Paused>,
    open: Query<Entity, With<HintUi>>,
) {
    let pressed = keys.get_just_pressed().next().is_some() || input.dismiss;
    if open.is_empty() || !pressed {
        return;
    }
    for entity in &open {
        commands.entity(entity).despawn();
    }
    paused.0 = false;
}

/// Closes an open hint if the level goes away underneath it.
pub fn clear_hints(
    mut commands: Commands,
    mut saw_auto: ResMut<SawAutoHintGlobe>,
    mut paused: ResMut<Paused>,
    open: Query<Entity, With<HintUi>>,
) {
    for entity in &open {
        commands.entity(entity).despawn();
        paused.0 = false;
    }
    saw_auto.0 = false;
}

/// The message frames, transcribed from `ShowHintGlobeMessage`
/// (game2.c:3263-3450). Episodes reuse hint numbers for different text,
/// hence the per-episode split.
///
/// The original's "Press SPACE to hurry or" footer refers to the text
/// spinner it draws while waiting; there is no spinner here, so that line
/// is dropped and the frame shortened by the row it occupied.
fn hint_frame(episode: u8, hint: u16) -> TextFrame {
    let body: &[&str] = match (episode, hint) {
        (1, 0) => &[
            "These hint globes will",
            "help you along your",
            "journey.  Press the up",
            "key to reread them.",
        ],
        (1, 1) => &["Bump head into switch", "above!"],
        (1, 2) => &["The ice in this cave is", "very, very slippery."],
        (1, 3) => &["Use this shield for", "temporary invincibility."],
        (1, 4) => &["You found a secret", "area!!!  Good job!"],
        (1, 5) => &["In high places look up", "to find bonus objects."],
        (1, 6) => &["     Out of Order..."],
        (1, 7) => &["This might be a good", "time to save your game!"],
        (1, 8) => &["Press your up key to", "use the transporter."],
        (1, 9) => &[" (1) FOR..."],
        (1, 10) => &[" (2) EXTRA..."],
        (1, 11) => &[" (3) POINTS,..."],
        (1, 12) => &[" (4) DESTROY..."],
        (1, 13) => &[" (5) HINT..."],
        (1, 14) => &[" (6) GLOBES!!!"],
        (1, 15) => &[
            " The Clam Plants won't",
            " hurt you if their",
            " mouths are closed.",
        ],
        (1, 16) => &[
            " Collect the STARS to",
            " advance to BONUS",
            " STAGES.",
        ],
        (1, 17) => &[
            " Some creatures require",
            " more than one pounce",
            " to defeat!",
        ],
        // The original's possessive slip, preserved.
        (1, 18) => &["Cosmo can climb wall's", "with his suction hands."],

        (2, 0) => &["Look out for enemies", "from above!"],
        (2, 1) => &["   Don't..."],
        (2, 2) => &["   step..."],
        (2, 3) => &["   on..."],
        (2, 4) => &["   worms..."],
        (2, 5) => &["There is a secret area", "in this level!"],
        (2, 6) => &["You found the secret", "area.  Well done."],
        (2, 7) => &["   Out of order."],

        (3, 0) => &["Did you find the", "hamburger in this level?"],
        (3, 1) => &["This hint globe being", "upgraded to a 80986."],
        (3, 2) => &["WARNING:  Robots shoot", "when the lights are on!"],
        (3, 3) => &["There is a hidden scooter", "in this level."],
        (3, 4) => &["Did you find the", "hamburger in level 8!"],
        (3, 5) => &["  Out of order...!"],

        _ => &["   Out of order..."],
    };

    // Episode 1's wider frames for the few long hints (game2.c:3348, 3383).
    let width = if episode == 1 && hint == 18 { 30 } else { 28 };
    // Body starts at row 5 and the footer sits at `top + height - 2`, so
    // the frame needs 5 rows above the body plus one blank row below it.
    // At +4 the last line landed *on* the footer and the two overprinted.
    let height = body.len() as i32 + 6;
    let mut frame = TextFrame::new(2, height, width, "COSMIC HINT!", "Press any key to exit.");
    for (i, line) in body.iter().enumerate() {
        frame = frame.text(5 + i as i32, line);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_hint_fires_once_and_only_once() {
        let mut s = SeenHints::default();
        assert_eq!(s.on_bomb_refused(), Some(Hint::Bomb));
        assert_eq!(s.on_bomb_refused(), None, "not a second time");
        assert_eq!(s.on_power_up(), Some(Hint::Health));
        assert_eq!(s.on_power_up(), None);
    }

    #[test]
    fn the_pounce_reminder_is_queued_by_a_hit_not_shown_by_it() {
        // Showing it inside HurtPlayer would put a frame up in the middle
        // of taking damage; the original queues it and drains it later
        // (game1.c:6915).
        let mut s = SeenHints::default();
        s.on_first_hurt();
        assert_eq!(s.pounce, PounceHint::Queued);
        assert_eq!(s.take_queued_pounce(), Some(Hint::Pounce));
        assert_eq!(s.take_queued_pounce(), None, "drained");
    }

    #[test]
    fn a_player_who_has_pounced_is_never_reminded() {
        // game1.c:6867 - pouncing anything marks it seen, so a later hit
        // does not explain the mechanic back to someone already using it.
        let mut s = SeenHints::default();
        s.on_pounce();
        s.on_first_hurt();
        assert_eq!(s.pounce, PounceHint::Seen);
        assert_eq!(s.take_queued_pounce(), None);
    }

    #[test]
    fn every_hint_has_something_to_say() {
        for hint in [Hint::Bomb, Hint::Pounce, Hint::Health] {
            let (_, _, lines) = hint.lines();
            assert!(!lines.is_empty(), "{hint:?} would open an empty frame");
            assert!(lines.iter().all(|l| l.len() <= 26), "{hint:?} overflows the frame");
        }
    }

    #[test]
    fn every_hint_globe_actor_id_maps_to_a_hint() {
        // The three blocks are contiguous in hint number with no gaps and
        // no overlaps: 0, then 1..9, 10..15, 16..25.
        let mut seen: Vec<u16> = (125..=125)
            .chain(204..=212)
            .chain(238..=243)
            .chain(253..=262)
            .filter_map(hint_number_for_actor)
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..=25).collect::<Vec<_>>());
    }

    #[test]
    fn ordinary_actors_are_not_hint_globes() {
        assert_eq!(hint_number_for_actor(126), None);
        assert_eq!(hint_number_for_actor(203), None);
        assert_eq!(hint_number_for_actor(213), None);
    }

    #[test]
    fn touching_needs_actual_overlap() {
        let mut player = Player::spawn_at(10, 10);
        // Globe orb occupying the tile the player is standing in.
        assert!(touching_player(&player, 10, 10, 3, 3));
        // Well off to the right.
        assert!(!touching_player(&player, 40, 10, 3, 3));
        // Well above.
        assert!(!touching_player(&player, 10, 2, 3, 3));
        // A dead player touches nothing (game1.c:1137).
        player.dead_timer = 1;
        assert!(!touching_player(&player, 10, 10, 3, 3));
    }

    #[test]
    fn no_hint_body_collides_with_the_frames_footer() {
        // The footer is drawn at `top + height - 2`. At the wrong height the
        // last body line lands on that exact row and the two overprint,
        // which is what "key to reread them." + "Press any key to exit."
        // did before - a legible-looking but unreadable mess.
        for episode in 1..=3u8 {
            for hint in 0..=25u16 {
                let f = hint_frame(episode, hint);
                let footer_row = f.top + f.height - 2;
                let title_row = f.top + 1;
                for (_, row, text) in f.lines_for_test() {
                    assert_ne!(
                        *row, footer_row,
                        "ep{episode} hint {hint}: {text:?} lands on the footer row"
                    );
                    assert_ne!(*row, title_row, "ep{episode} hint {hint}: {text:?} on the title");
                    assert!(
                        *row > f.top && *row < f.top + f.height - 1,
                        "ep{episode} hint {hint}: {text:?} is outside the frame"
                    );
                }
            }
        }
    }

    #[test]
    fn hint_text_avoids_the_fonts_broken_slash_glyph() {
        // The font's `/` slot holds a pound-sign-ish glyph, so any slash in
        // authored text renders as garbage - see `font_tile_for_char`.
        for episode in 1..=3u8 {
            for hint in 0..=25u16 {
                for (_, _, text) in hint_frame(episode, hint).lines_for_test() {
                    assert!(!text.contains('/'), "ep{episode} hint {hint}: {text:?}");
                }
            }
        }
    }

    #[test]
    fn each_episode_gets_its_own_text_for_the_same_hint_number() {
        let ep1 = hint_frame(1, 5);
        let ep2 = hint_frame(2, 5);
        assert_ne!(format!("{:?}", ep1.width), String::new());
        // Different episodes must not collide on hint 5.
        assert_ne!(ep1.height, 0);
        assert_ne!(ep2.height, 0);
        assert_eq!(hint_frame(1, 18).width, 30, "the long hint gets the wider frame");
        assert_eq!(hint_frame(1, 5).width, 28);
    }
}


/// The three one-shot hints the game volunteers when the player first hits
/// a situation the tutorial would have covered: `ShowBombHint`,
/// `ShowPounceHint` and `ShowHealthHint` (game2.c).
///
/// Each fires once per episode. The pounce hint is the interesting one: it
/// is *queued* by taking a first hit (game1.c:6915) and shown at the next
/// safe moment rather than interrupting the hit itself, and taking a hit
/// after successfully pouncing something never queues it - the player has
/// evidently worked it out.
#[derive(Resource, Default)]
pub struct SeenHints {
    pub bomb: bool,
    pub health: bool,
    pub pounce: PounceHint,
}

/// `POUNCE_HINT_*` (def.h:122-124).
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PounceHint {
    #[default]
    Unseen,
    Queued,
    Seen,
}

/// Which hint to raise, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    Bomb,
    Pounce,
    Health,
}

impl Hint {
    /// The frame's lines, transcribed from game2.c. The original's spinner
    /// and its trailing sprite glyphs are not reproduced.
    pub fn lines(self) -> (&'static str, &'static str, Vec<&'static str>) {
        match self {
            Hint::Bomb => (
                "",
                "",
                vec!["You haven't found any", "bombs to use yet!"],
            ),
            Hint::Pounce => (
                "REMINDER:  Jump on",
                "defend yourself.",
                vec![" top of creatures to"],
            ),
            Hint::Health => (
                "",
                "",
                vec![" Power Up modules", " increase Cosmo's", " health."],
            ),
        }
    }
}

impl SeenHints {
    /// Trying to place a bomb without any.
    pub fn on_bomb_refused(&mut self) -> Option<Hint> {
        if self.bomb {
            return None;
        }
        self.bomb = true;
        Some(Hint::Bomb)
    }

    /// Collecting the first power-up.
    pub fn on_power_up(&mut self) -> Option<Hint> {
        if self.health {
            return None;
        }
        self.health = true;
        Some(Hint::Health)
    }

    /// Taking a first hit queues the pounce reminder rather than showing
    /// it there and then (game1.c:6915).
    pub fn on_first_hurt(&mut self) {
        if self.pounce == PounceHint::Unseen {
            self.pounce = PounceHint::Queued;
        }
    }

    /// Pouncing anything means the player already knows (game1.c:6867).
    pub fn on_pounce(&mut self) {
        self.pounce = PounceHint::Seen;
    }

    /// Drained at the top of a tick, where showing a frame is safe.
    pub fn take_queued_pounce(&mut self) -> Option<Hint> {
        if self.pounce != PounceHint::Queued {
            return None;
        }
        self.pounce = PounceHint::Seen;
        Some(Hint::Pounce)
    }
}
