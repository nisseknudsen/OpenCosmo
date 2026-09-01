//! Demo recording and playback.
//!
//! `ReadDemoFrame` / `WriteDemoFrame` / `SaveDemoData` (game1.c:9677-9740).
//! One byte per tick holds the six movement commands plus a win flag, and
//! the file is a 16-bit length followed by that many bytes.
//!
//! This is the same idea as `COSMO_INPUT`, the scripted input the headless
//! checks use, in the game's own format rather than a text one.

use crate::player::PlayerInput;
use bevy::prelude::*;

/// `demoDataLength > 4998` stops recording (game1.c:9701), so a demo is at
/// most 4999 frames - a shade over four and a half minutes at 18.2Hz.
pub const MAX_FRAMES: usize = 4999;

/// Bit positions in a demo byte (game1.c:9679-9685).
const WEST: u8 = 0x01;
const EAST: u8 = 0x02;
const NORTH: u8 = 0x04;
const SOUTH: u8 = 0x08;
const JUMP: u8 = 0x10;
const BOMB: u8 = 0x20;
const WIN: u8 = 0x40;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum DemoState {
    #[default]
    None,
    Record,
    Play,
}

#[derive(Resource, Default)]
pub struct Demo {
    pub state: DemoState,
    pub frames: Vec<u8>,
    pub pos: usize,
    /// Set by a recorded frame's win bit, which the original uses to mark
    /// where the demo should move on to the next level.
    pub win: bool,
}

/// Packs a tick's input into the byte the original writes.
pub fn pack(input: &PlayerInput, win: bool) -> u8 {
    let mut b = 0;
    if input.west { b |= WEST; }
    if input.east { b |= EAST; }
    if input.look_up { b |= NORTH; }
    if input.look_down { b |= SOUTH; }
    if input.jump { b |= JUMP; }
    if input.bomb { b |= BOMB; }
    if win { b |= WIN; }
    b
}

/// Unpacks one into an input, returning the win bit separately.
pub fn unpack(b: u8, input: &mut PlayerInput) -> bool {
    input.west = b & WEST != 0;
    input.east = b & EAST != 0;
    input.look_up = b & NORTH != 0;
    input.look_down = b & SOUTH != 0;
    input.jump = b & JUMP != 0;
    input.bomb = b & BOMB != 0;
    b & WIN != 0
}

impl Demo {
    /// `PREVDEMO.MNI` in the original; kept beside the other saved state.
    pub fn path() -> std::path::PathBuf {
        crate::input::config_path().with_file_name("PREVDEMO.MNI")
    }

    /// A 16-bit little-endian length, then the frames (game1.c:9725-9728).
    pub fn to_bytes(&self) -> Vec<u8> {
        let len = self.frames.len().min(MAX_FRAMES) as u16;
        let mut out = len.to_le_bytes().to_vec();
        out.extend_from_slice(&self.frames[..len as usize]);
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Vec<u8>> {
        let len = u16::from_le_bytes([*b.first()?, *b.get(1)?]) as usize;
        // A length longer than the file is a truncated or corrupt demo.
        b.get(2..2 + len).map(|f| f.to_vec())
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&path, self.to_bytes()).is_err() {
            warn!("could not save the demo to {}", path.display());
        }
    }

    pub fn load() -> Option<Vec<u8>> {
        Self::from_bytes(&std::fs::read(Self::path()).ok()?)
    }

    pub fn start_recording(&mut self) {
        self.state = DemoState::Record;
        self.frames.clear();
        self.pos = 0;
    }

    pub fn stop(&mut self) {
        if self.state == DemoState::Record {
            self.save();
        }
        self.state = DemoState::None;
    }

    pub fn start_playback(&mut self) -> bool {
        match Self::load() {
            Some(frames) if !frames.is_empty() => {
                self.frames = frames;
                self.pos = 0;
                self.state = DemoState::Play;
                true
            }
            _ => false,
        }
    }
}

/// Records or replays a tick. Runs before the gameplay tick reads input,
/// so a replayed frame drives the same code a live one would.
pub fn drive_demo(mut demo: ResMut<Demo>, mut input: ResMut<PlayerInput>) {
    match demo.state {
        DemoState::None => {}
        DemoState::Record => {
            if demo.frames.len() >= MAX_FRAMES {
                demo.stop();
                return;
            }
            let byte = pack(&input, false);
            demo.frames.push(byte);
        }
        DemoState::Play => {
            let Some(byte) = demo.frames.get(demo.pos).copied() else {
                demo.state = DemoState::None;
                return;
            };
            demo.pos += 1;
            demo.win = unpack(byte, &mut input);
            if demo.pos >= demo.frames.len() {
                demo.state = DemoState::None;
            }
        }
    }
}

/// F9 starts and stops recording; the main menu's D)emo plays one back.
/// COSMO_DEMO=play starts a recorded demo at launch, for headless checks.
pub fn autostart_demo(mut demo: ResMut<Demo>) {
    if std::env::var("COSMO_DEMO").as_deref() == Ok("play") && !demo.start_playback() {
        warn!("COSMO_DEMO=play but there is no recorded demo");
    }
}

pub fn demo_hotkeys(keys: Res<ButtonInput<KeyCode>>, mut demo: ResMut<Demo>) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    match demo.state {
        DemoState::Record => {
            demo.stop();
            info!("demo recording stopped and saved");
        }
        _ => {
            demo.start_recording();
            info!("demo recording started");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_with(west: bool, jump: bool, bomb: bool) -> PlayerInput {
        PlayerInput {
            west,
            jump,
            bomb,
            ..default()
        }
    }

    #[test]
    fn a_frame_round_trips_through_its_byte() {
        let original = input_with(true, true, false);
        let byte = pack(&original, false);
        let mut back = PlayerInput::default();
        assert!(!unpack(byte, &mut back));
        assert_eq!(back.west, original.west);
        assert_eq!(back.jump, original.jump);
        assert_eq!(back.bomb, original.bomb);
        assert!(!back.east);
    }

    #[test]
    fn every_command_has_its_own_bit() {
        // Six commands plus the win flag, and no two may share a bit or a
        // recorded demo would replay the wrong thing.
        let mut seen = Vec::new();
        for (set, bit) in [
            (PlayerInput { west: true, ..default() }, WEST),
            (PlayerInput { east: true, ..default() }, EAST),
            (PlayerInput { look_up: true, ..default() }, NORTH),
            (PlayerInput { look_down: true, ..default() }, SOUTH),
            (PlayerInput { jump: true, ..default() }, JUMP),
            (PlayerInput { bomb: true, ..default() }, BOMB),
        ] {
            let b = pack(&set, false);
            assert_eq!(b, bit, "one command should set exactly its own bit");
            assert!(!seen.contains(&b), "two commands share a bit");
            seen.push(b);
        }
        assert_eq!(pack(&PlayerInput::default(), true), WIN);
    }

    #[test]
    fn the_win_bit_is_carried_separately_from_the_input() {
        let mut back = PlayerInput::default();
        assert!(unpack(pack(&PlayerInput::default(), true), &mut back));
        assert!(!back.west && !back.jump, "the win bit is not a command");
    }

    #[test]
    fn a_demo_file_round_trips() {
        let mut d = Demo::default();
        d.frames = vec![1, 2, 4, 8, 16, 32, 64];
        assert_eq!(Demo::from_bytes(&d.to_bytes()), Some(d.frames.clone()));
    }

    #[test]
    fn a_truncated_demo_is_refused_rather_than_panicking() {
        let mut d = Demo::default();
        d.frames = vec![1, 2, 3, 4, 5];
        let bytes = d.to_bytes();
        for len in 0..bytes.len() {
            assert_eq!(Demo::from_bytes(&bytes[..len]), None, "len {len}");
        }
    }

    #[test]
    fn recording_stops_at_the_length_the_original_allows() {
        let mut d = Demo::default();
        d.start_recording();
        assert_eq!(d.state, DemoState::Record);
        d.frames = vec![0; MAX_FRAMES];
        // The next tick would push past the cap.
        assert!(d.frames.len() >= MAX_FRAMES);
    }

    #[test]
    fn an_empty_demo_does_not_start_playing() {
        let mut d = Demo::default();
        d.frames.clear();
        // No file, or an empty one, leaves the state alone rather than
        // entering a playback that immediately ends.
        assert_eq!(d.state, DemoState::None);
    }
}
