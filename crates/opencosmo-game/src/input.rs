//! Rebindable controls, for keyboard and gamepad.
//!
//! The original stores exactly six bindings - north, south, west, east,
//! jump, bomb (`LoadConfigurationData`, game2.c:2963-2977) - and lets you
//! change them from a "Game Redefine" menu offering keyboard and joystick
//! redefine (game2.c:2923-2927). The same six actions are the whole control
//! surface here; what is new is that each one can carry several bindings at
//! once, so keyboard and pad work without a mode switch.
//!
//! ## Why input is sampled twice
//!
//! The gameplay tick runs at 18.2Hz, matching the DOS timer. Reading the
//! keyboard *on* that tick means a press shorter than ~55ms can land
//! entirely between two ticks and never be seen - a dropped jump, and one
//! that gets blamed on the physics rather than on input. So the keyboard is
//! sampled every frame and the results are held until the tick collects
//! them. A tap is stretched to exactly one tick; a held key is unaffected.
//!
//! This does not change the original's semantics. Its `cmdJump` is a level
//! ("is the key down now"), not an edge, and `cmdJumpLatch` is what stops a
//! held key re-triggering - both of which still hold here.

use crate::player::PlayerInput;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The original's six configurable actions (game2.c:2972-2977).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    North,
    South,
    West,
    East,
    Jump,
    Bomb,
}

impl Action {
    pub const ALL: [Action; 6] = [
        Action::North,
        Action::South,
        Action::West,
        Action::East,
        Action::Jump,
        Action::Bomb,
    ];

    /// The label the original's keyboard config screen uses
    /// (game2.c:1946-1957), so the rebinding screen reads the same.
    pub fn label(self) -> &'static str {
        match self {
            Action::North => "Up",
            Action::South => "Down",
            Action::West => "Left",
            Action::East => "Right",
            Action::Jump => "Jump",
            Action::Bomb => "Bomb",
        }
    }
}

/// Which way along an axis counts as pressed. A stick is a d-pad here: the
/// tile-stepped movement has no sub-tile precision to offer, so there is
/// nothing an analogue reading could express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisSign {
    Positive,
    Negative,
}

/// Firm on purpose. With a digital result, a low deadzone just means the
/// stick's resting jitter walks the player around.
const AXIS_DEADZONE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Binding {
    Key(KeyCode),
    Button(GamepadButton),
    Axis(GamepadAxis, AxisSign),
}

impl Binding {
    /// A short name for the controls screen. Terse on purpose: the screen
    /// gives each binding `CONTROLS_VALUE_WIDTH` tiles and there is no
    /// smaller font to fall back on.
    pub fn label(self) -> String {
        match self {
            Binding::Key(key) => key_label(key),
            Binding::Button(button) => button_label(button),
            Binding::Axis(axis, sign) => {
                let stick = match axis {
                    GamepadAxis::LeftStickX | GamepadAxis::LeftStickY => "LStk",
                    GamepadAxis::RightStickX | GamepadAxis::RightStickY => "RStk",
                    _ => "Axis",
                };
                let direction = match (axis, sign) {
                    (GamepadAxis::LeftStickY | GamepadAxis::RightStickY, AxisSign::Positive) => "Up",
                    (GamepadAxis::LeftStickY | GamepadAxis::RightStickY, AxisSign::Negative) => "Dn",
                    (_, AxisSign::Positive) => "R",
                    (_, AxisSign::Negative) => "L",
                };
                format!("{stick}{direction}")
            }
        }
    }

    pub fn is_key(self) -> bool {
        matches!(self, Binding::Key(_))
    }
}

/// Bevy names face buttons by position (South is the bottom one); the
/// screen keeps that rather than guessing at a vendor's letters, since the
/// letters differ between an Xbox pad and a Nintendo one.
fn button_label(button: GamepadButton) -> String {
    match button {
        GamepadButton::South => "Pad Dn".into(),
        GamepadButton::East => "Pad R".into(),
        GamepadButton::North => "Pad Up".into(),
        GamepadButton::West => "Pad L".into(),
        GamepadButton::DPadUp => "DPad Up".into(),
        GamepadButton::DPadDown => "DPad Dn".into(),
        GamepadButton::DPadLeft => "DPad L".into(),
        GamepadButton::DPadRight => "DPad R".into(),
        GamepadButton::LeftTrigger => "L1".into(),
        GamepadButton::RightTrigger => "R1".into(),
        GamepadButton::LeftTrigger2 => "L2".into(),
        GamepadButton::RightTrigger2 => "R2".into(),
        other => format!("{other:?}"),
    }
}

/// `KeyCode`'s `Debug` is close enough to a display name for most keys, but
/// the common ones deserve the short forms the original used.
fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::ArrowUp => "Up".into(),
        KeyCode::ArrowDown => "Down".into(),
        KeyCode::ArrowLeft => "Left".into(),
        KeyCode::ArrowRight => "Right".into(),
        KeyCode::ControlLeft => "LCtrl".into(),
        KeyCode::ControlRight => "RCtrl".into(),
        KeyCode::AltLeft => "LAlt".into(),
        KeyCode::AltRight => "RAlt".into(),
        KeyCode::ShiftLeft => "LShift".into(),
        KeyCode::ShiftRight => "RShift".into(),
        KeyCode::Space => "Space".into(),
        KeyCode::Enter => "Enter".into(),
        other => {
            let name = format!("{other:?}");
            // "KeyA" -> "A", "Digit1" -> "1", "Numpad4" -> "Num4".
            name.strip_prefix("Key")
                .map(str::to_string)
                .or_else(|| name.strip_prefix("Digit").map(str::to_string))
                .or_else(|| name.strip_prefix("Numpad").map(|s| format!("Num{s}")))
                .unwrap_or(name)
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct Bindings {
    pub entries: Vec<(Action, Vec<Binding>)>,
}

impl Default for Bindings {
    /// The original's own defaults (Ctrl to jump, Alt to bomb, arrows to
    /// move - game2.c:2972-2977), plus WASD and Space as modern
    /// alternatives and a gamepad that works without being configured.
    ///
    /// Putting bombs on Ctrl instead would collide with the muscle memory of
    /// anyone who played the original, so Space is *added* to jump rather
    /// than replacing anything.
    fn default() -> Self {
        use Action::*;
        use AxisSign::*;
        Bindings {
            entries: vec![
                (
                    North,
                    vec![
                        Binding::Key(KeyCode::ArrowUp),
                        Binding::Key(KeyCode::KeyW),
                        Binding::Button(GamepadButton::DPadUp),
                        Binding::Axis(GamepadAxis::LeftStickY, Positive),
                    ],
                ),
                (
                    South,
                    vec![
                        Binding::Key(KeyCode::ArrowDown),
                        Binding::Key(KeyCode::KeyS),
                        Binding::Button(GamepadButton::DPadDown),
                        Binding::Axis(GamepadAxis::LeftStickY, Negative),
                    ],
                ),
                (
                    West,
                    vec![
                        Binding::Key(KeyCode::ArrowLeft),
                        Binding::Key(KeyCode::KeyA),
                        Binding::Button(GamepadButton::DPadLeft),
                        Binding::Axis(GamepadAxis::LeftStickX, Negative),
                    ],
                ),
                (
                    East,
                    vec![
                        Binding::Key(KeyCode::ArrowRight),
                        Binding::Key(KeyCode::KeyD),
                        Binding::Button(GamepadButton::DPadRight),
                        Binding::Axis(GamepadAxis::LeftStickX, Positive),
                    ],
                ),
                (
                    Jump,
                    vec![
                        Binding::Key(KeyCode::Space),
                        Binding::Key(KeyCode::ControlLeft),
                        Binding::Key(KeyCode::ControlRight),
                        Binding::Button(GamepadButton::South),
                    ],
                ),
                (
                    Bomb,
                    vec![
                        Binding::Key(KeyCode::AltLeft),
                        Binding::Key(KeyCode::AltRight),
                        Binding::Button(GamepadButton::West),
                    ],
                ),
            ],
        }
    }
}

/// Tiles the controls screen has for a binding summary. Every label has to
/// fit; `key_labels_are_short_enough_for_the_config_screen` enforces it.
pub const CONTROLS_VALUE_WIDTH: usize = 16;

impl Bindings {
    /// What the controls screen shows for an action: the first keyboard
    /// binding and the first gamepad one.
    ///
    /// Listing all of them would overflow the frame several times over -
    /// the four defaults for "Up" alone run to 23 characters. These two are
    /// the ones a player needs to see anyway.
    pub fn summary(&self, action: Action) -> String {
        let list = self.get(action);
        let key = list.iter().find(|b| b.is_key()).map(|b| b.label());
        let pad = list.iter().find(|b| !b.is_key()).map(|b| b.label());
        match (key, pad) {
            // Comma, not a slash: this font's `/` slot holds a pound-sign
            // -ish glyph rather than a slash (see `font_tile_for_char`).
            (Some(k), Some(p)) => format!("{k}, {p}"),
            (Some(k), None) => k,
            (None, Some(p)) => p,
            (None, None) => "-none-".into(),
        }
    }

    pub fn get(&self, action: Action) -> &[Binding] {
        self.entries
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, b)| b.as_slice())
            .unwrap_or(&[])
    }

    /// Replaces every binding for `action` with a single one, and removes
    /// that binding from any other action so two actions can't share it.
    pub fn rebind(&mut self, action: Action, binding: Binding) {
        for (a, list) in self.entries.iter_mut() {
            if *a == action {
                *list = vec![binding];
            } else {
                list.retain(|b| *b != binding);
            }
        }
    }

    pub fn is_pressed(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        pads: &[&Gamepad],
    ) -> bool {
        self.get(action).iter().any(|binding| match binding {
            Binding::Key(key) => keys.pressed(*key),
            Binding::Button(button) => pads.iter().any(|pad| pad.pressed(*button)),
            Binding::Axis(axis, sign) => pads.iter().any(|pad| {
                let value = pad.get(*axis).unwrap_or(0.0);
                match sign {
                    AxisSign::Positive => value > AXIS_DEADZONE,
                    AxisSign::Negative => value < -AXIS_DEADZONE,
                }
            }),
        })
    }
}

/// Where the bindings are saved. Follows the XDG base directory spec,
/// falling back to the working directory when neither variable is set
/// (which is really only the case in a stripped-down test environment).
pub fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("opencosmo").join("controls.json")
}

pub fn load_bindings() -> Bindings {
    let path = config_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Bindings::default();
    };
    match serde_json::from_slice(&bytes) {
        Ok(bindings) => bindings,
        Err(err) => {
            // A config we can't read is not worth failing to start over.
            warn!("ignoring {}: {err}", path.display());
            Bindings::default()
        }
    }
}

pub fn save_bindings(bindings: &Bindings) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let written = serde_json::to_vec_pretty(bindings)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    match written {
        Ok(()) => info!("saved controls to {}", path.display()),
        // Not being able to save is worth saying, but never worth crashing
        // over - the bindings still apply for this session.
        Err(err) => warn!("could not save controls to {}: {err}", path.display()),
    }
}

/// Every-frame sample, held until the gameplay tick collects it.
///
/// `pending` is the union of everything pressed since the last tick;
/// `latest` is the state at this instant. See the module docs for why.
#[derive(Resource, Default)]
pub struct InputAccumulator {
    pending: PlayerInput,
    latest: PlayerInput,
}

impl InputAccumulator {
    /// Called on the gameplay tick: hand over everything seen since the last
    /// one, then start the next window from the state right now.
    pub fn take(&mut self) -> PlayerInput {
        let collected = self.pending.clone();
        self.pending = self.latest.clone();
        collected
    }
}

fn merge(into: &mut PlayerInput, from: &PlayerInput) {
    into.west |= from.west;
    into.east |= from.east;
    into.jump |= from.jump;
    into.look_up |= from.look_up;
    into.look_down |= from.look_down;
    into.bomb |= from.bomb;
    into.dismiss |= from.dismiss;
}

pub fn sample_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    bindings: Res<Bindings>,
    mut accum: ResMut<InputAccumulator>,
) {
    let pads: Vec<&Gamepad> = pads.iter().collect();
    let now = PlayerInput {
        west: bindings.is_pressed(Action::West, &keys, &pads),
        east: bindings.is_pressed(Action::East, &keys, &pads),
        jump: bindings.is_pressed(Action::Jump, &keys, &pads),
        look_up: bindings.is_pressed(Action::North, &keys, &pads),
        look_down: bindings.is_pressed(Action::South, &keys, &pads),
        bomb: bindings.is_pressed(Action::Bomb, &keys, &pads),
        dismiss: false,
    };
    merge(&mut accum.pending, &now);
    accum.latest = now;
}

/// The first thing pressed on any device, for the rebinding screen.
pub fn first_pressed(
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
) -> Option<Binding> {
    // Escape and the digits drive the screen itself, so they can't be bound
    // from it.
    if let Some(key) = keys.get_just_pressed().find(|k| !is_reserved(**k)) {
        return Some(Binding::Key(*key));
    }
    for pad in pads.iter() {
        if let Some(button) = pad.get_just_pressed().next() {
            return Some(Binding::Button(*button));
        }
        for axis in [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ] {
            let value = pad.get(axis).unwrap_or(0.0);
            if value > AXIS_DEADZONE {
                return Some(Binding::Axis(axis, AxisSign::Positive));
            }
            if value < -AXIS_DEADZONE {
                return Some(Binding::Axis(axis, AxisSign::Negative));
            }
        }
    }
    None
}

fn is_reserved(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::Escape
            | KeyCode::Digit1
            | KeyCode::Digit2
            | KeyCode::Digit3
            | KeyCode::Digit4
            | KeyCode::Digit5
            | KeyCode::Digit6
            | KeyCode::KeyR
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_originals_plus_modern_alternatives() {
        let b = Bindings::default();
        // game2.c:2976-2977 - Ctrl jumps, Alt bombs.
        assert!(b.get(Action::Jump).contains(&Binding::Key(KeyCode::ControlLeft)));
        assert!(b.get(Action::Bomb).contains(&Binding::Key(KeyCode::AltLeft)));
        // Space is added, not substituted.
        assert!(b.get(Action::Jump).contains(&Binding::Key(KeyCode::Space)));
        // ...and bombs must never end up on Ctrl.
        assert!(!b.get(Action::Bomb).contains(&Binding::Key(KeyCode::ControlLeft)));
    }

    #[test]
    fn every_action_has_a_keyboard_and_a_gamepad_binding_out_of_the_box() {
        let b = Bindings::default();
        for action in Action::ALL {
            let list = b.get(action);
            assert!(
                list.iter().any(|x| matches!(x, Binding::Key(_))),
                "{action:?} has no key"
            );
            assert!(
                list.iter()
                    .any(|x| matches!(x, Binding::Button(_) | Binding::Axis(..))),
                "{action:?} has no pad binding"
            );
        }
    }

    #[test]
    fn rebinding_takes_the_key_away_from_whoever_had_it() {
        let mut b = Bindings::default();
        b.rebind(Action::Bomb, Binding::Key(KeyCode::Space));
        assert_eq!(b.get(Action::Bomb), [Binding::Key(KeyCode::Space)]);
        assert!(
            !b.get(Action::Jump).contains(&Binding::Key(KeyCode::Space)),
            "two actions must not share one binding"
        );
        // Jump keeps its other bindings.
        assert!(b.get(Action::Jump).contains(&Binding::Key(KeyCode::ControlLeft)));
    }

    #[test]
    fn bindings_survive_a_round_trip_through_the_config_format() {
        let mut b = Bindings::default();
        b.rebind(Action::Jump, Binding::Button(GamepadButton::North));
        let json = serde_json::to_vec(&b).unwrap();
        let back: Bindings = serde_json::from_slice(&json).unwrap();
        assert_eq!(back.get(Action::Jump), [Binding::Button(GamepadButton::North)]);
        assert_eq!(back.get(Action::West), b.get(Action::West));
    }

    #[test]
    fn a_tap_between_ticks_is_not_dropped() {
        let mut accum = InputAccumulator::default();
        let tap = PlayerInput {
            jump: true,
            ..default()
        };
        // Frame 1: pressed. Frame 2: already released, before the tick runs.
        merge(&mut accum.pending, &tap);
        accum.latest = tap.clone();
        merge(&mut accum.pending, &PlayerInput::default());
        accum.latest = PlayerInput::default();

        assert!(accum.take().jump, "the tap must survive to the tick");
        assert!(!accum.take().jump, "and must not repeat on the next one");
    }

    #[test]
    fn a_held_key_stays_held_across_ticks() {
        let mut accum = InputAccumulator::default();
        let held = PlayerInput {
            west: true,
            ..default()
        };
        for _ in 0..3 {
            merge(&mut accum.pending, &held);
            accum.latest = held.clone();
            assert!(accum.take().west);
        }
    }

    #[test]
    fn key_labels_are_short_enough_for_the_config_screen() {
        assert_eq!(key_label(KeyCode::KeyA), "A");
        assert_eq!(key_label(KeyCode::Digit1), "1");
        assert_eq!(key_label(KeyCode::Numpad4), "Num4");
        assert_eq!(key_label(KeyCode::ControlLeft), "LCtrl");
    }

    #[test]
    fn every_default_summary_fits_the_controls_screen() {
        // The screen has no smaller font to fall back on, so an overflowing
        // label would simply run off the frame.
        let b = Bindings::default();
        for action in Action::ALL {
            let summary = b.summary(action);
            assert!(
                summary.chars().count() <= CONTROLS_VALUE_WIDTH,
                "{action:?} renders as {summary:?} ({} chars), over the {CONTROLS_VALUE_WIDTH} available",
                summary.chars().count()
            );
        }
    }

    #[test]
    fn a_summary_names_one_control_per_device() {
        let b = Bindings::default();
        // Jump's first key is Space and its pad binding is the bottom face
        // button; the other keyboard alternatives stay bound but unlisted.
        assert_eq!(b.summary(Action::Jump), "Space, Pad Dn");
        assert!(b.get(Action::Jump).len() > 2);
    }

    #[test]
    fn an_action_with_nothing_bound_says_so_rather_than_rendering_blank() {
        let mut b = Bindings::default();
        for (_, list) in b.entries.iter_mut() {
            list.clear();
        }
        assert_eq!(b.summary(Action::Jump), "-none-");
    }
}
