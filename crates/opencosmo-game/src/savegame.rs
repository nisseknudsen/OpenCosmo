//! Saving and restoring a game, in the original's own file format.
//!
//! `SaveGameState` / `LoadGameState` (game1.c:9317-9382) write a small
//! fixed record to `COSMO<n>.SV<slot>`, slots 1-9. The format is eleven
//! little-endian 16-bit words with a 32-bit score in the middle, and a
//! checksum of five of them.
//!
//! Byte-compatible on purpose: a save written by the original game loads
//! here, and one written here loads there.
//!
//! What gets saved is the state at the *start* of the current level, not
//! the live state - `PromptSaveGame` loads the level-entry checkpoint and
//! writes that (game1.c:9440-9447), which is what the prompt's own "Game
//! is saved at BEGINNING of level" note is telling the player.

use crate::flow::Checkpoint;

/// A save slot's contents.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaveGame {
    pub health: u16,
    pub score: u32,
    pub stars: u16,
    pub level: u16,
    pub bombs: u16,
    pub health_cells: u16,
    pub used_cheat: bool,
}

/// `putw` writes a little-endian word; the score is a little-endian dword
/// in the middle of them (game1.c:9366-9371).
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn get_u16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

impl SaveGame {
    pub fn from_checkpoint(cp: &Checkpoint, level: usize, used_cheat: bool) -> Self {
        SaveGame {
            health: cp.health.max(0) as u16,
            score: cp.score,
            stars: cp.stars as u16,
            level: level as u16,
            bombs: cp.bombs as u16,
            health_cells: cp.health_cells as u16,
            used_cheat,
        }
    }

    /// The checksum the original writes and verifies: a plain wrapping sum
    /// of five of the fields, deliberately *not* including the score
    /// (game1.c:9372).
    pub fn checksum(&self) -> u16 {
        self.health
            .wrapping_add(self.stars)
            .wrapping_add(self.level)
            .wrapping_add(self.bombs)
            .wrapping_add(self.health_cells)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        put_u16(&mut out, self.health);
        out.extend_from_slice(&self.score.to_le_bytes());
        put_u16(&mut out, self.stars);
        put_u16(&mut out, self.level);
        put_u16(&mut out, self.bombs);
        put_u16(&mut out, self.health_cells);
        put_u16(&mut out, u16::from(self.used_cheat));
        // Saving marks all three one-shot hints as already seen, so a
        // restored game never explains the basics again (game1.c:9369).
        put_u16(&mut out, 1); // bomb hint
        put_u16(&mut out, 2); // POUNCE_HINT_SEEN
        put_u16(&mut out, 1); // health hint
        put_u16(&mut out, self.checksum());
        out
    }

    /// `None` for a file that is too short or fails its checksum, which is
    /// what the original's `ShowRestoreGameError` reports.
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        let save = SaveGame {
            health: get_u16(b, 0)?,
            score: u32::from_le_bytes([
                *b.get(2)?,
                *b.get(3)?,
                *b.get(4)?,
                *b.get(5)?,
            ]),
            stars: get_u16(b, 6)?,
            level: get_u16(b, 8)?,
            bombs: get_u16(b, 10)?,
            health_cells: get_u16(b, 12)?,
            used_cheat: get_u16(b, 14)? != 0,
        };
        // Ten words plus the dword score puts the checksum at 22.
        if get_u16(b, 22)? != save.checksum() {
            return None;
        }
        Some(save)
    }

    /// `COSMO<episode>.SV<slot>` (episode1.h:18-21), in the same directory
    /// as the other saved settings.
    pub fn path(episode: u8, slot: u8) -> std::path::PathBuf {
        crate::input::config_path()
            .with_file_name(format!("COSMO{episode}.SV{slot}"))
    }

    pub fn save(&self, episode: u8, slot: u8) -> std::io::Result<()> {
        let path = Self::path(episode, slot);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_bytes())
    }

    pub fn load(episode: u8, slot: u8) -> Option<Self> {
        Self::from_bytes(&std::fs::read(Self::path(episode, slot)).ok()?)
    }
}

use bevy::prelude::*;

/// Which slot prompt is open, if either.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum SlotPrompt {
    #[default]
    None,
    Save,
    Restore,
    /// A restore that found nothing in that slot
    /// (`ShowRestoreGameError`, game2.c:2559).
    NotFound,
}

#[derive(Component)]
pub struct SlotPromptUi;

#[derive(Event, Clone, Copy)]
pub struct OpenSlotPrompt(pub SlotPrompt);

/// Raised when a slot was restored; the flow layer loads the level.
#[derive(Event)]
pub struct RestoredGame(pub SaveGame);

fn prompt_frame(prompt: SlotPrompt) -> Option<crate::panel::TextFrame> {
    Some(match prompt {
        SlotPrompt::None => return None,
        // game1.c:9425-9430
        SlotPrompt::Save => crate::panel::TextFrame::new(8, 10, 28, "Save a game.", "Press ESC to quit.")
            .text(11, " What game number (1-9)?")
            .text(13, " NOTE: Game is saved at")
            .text(14, " BEGINNING of level."),
        // game1.c:9391-9393
        SlotPrompt::Restore => {
            crate::panel::TextFrame::new(11, 7, 28, "Restore a game.", "Press ESC to quit.")
                .text(14, " What game number (1-9)?")
        }
        SlotPrompt::NotFound => {
            crate::panel::TextFrame::new(11, 4, 28, "Can not find that game!", "Press ANY key.")
        }
    })
}

pub fn open_slot_prompt(
    mut commands: Commands,
    mut events: EventReader<OpenSlotPrompt>,
    mut prompt: ResMut<SlotPrompt>,
    mut paused: ResMut<crate::help::Paused>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
) {
    let Some(OpenSlotPrompt(which)) = events.read().last().copied() else {
        return;
    };
    *prompt = which;
    paused.0 = true;
    if let Some(frame) = prompt_frame(which) {
        frame.spawn(&mut commands, &hud, ui_camera.0, SlotPromptUi);
    }
}

/// Digits 1-9 pick a slot; ESC, space and Enter all back out, as the
/// original's prompt accepts (game1.c:9396).
pub fn slot_prompt_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut prompt: ResMut<SlotPrompt>,
    mut paused: ResMut<crate::help::Paused>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    data: Res<crate::data::GameData>,
    checkpoint: Res<crate::flow::Checkpoint>,
    sequence: Res<crate::flow::LevelSequence>,
    mut restored: EventWriter<RestoredGame>,
    open: Query<Entity, With<SlotPromptUi>>,
) {
    if *prompt == SlotPrompt::None {
        return;
    }
    let close = |commands: &mut Commands, paused: &mut crate::help::Paused, prompt: &mut SlotPrompt| {
        for entity in open.iter() {
            commands.entity(entity).despawn();
        }
        *prompt = SlotPrompt::None;
        paused.0 = false;
    };

    if *prompt == SlotPrompt::NotFound {
        if keys.get_just_pressed().next().is_some() {
            close(&mut commands, &mut paused, &mut prompt);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
    {
        close(&mut commands, &mut paused, &mut prompt);
        return;
    }

    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    let Some(slot) = DIGITS.iter().position(|k| keys.just_pressed(*k)) else {
        return;
    };
    let slot = slot as u8 + 1;

    match *prompt {
        SlotPrompt::Save => {
            // The checkpoint, not the live state - the prompt says so.
            let save = SaveGame::from_checkpoint(&checkpoint, sequence.index, false);
            if let Err(err) = save.save(data.episode, slot) {
                warn!("could not save to slot {slot}: {err}");
            }
            close(&mut commands, &mut paused, &mut prompt);
        }
        SlotPrompt::Restore => match SaveGame::load(data.episode, slot) {
            Some(save) => {
                restored.write(RestoredGame(save));
                close(&mut commands, &mut paused, &mut prompt);
            }
            None => {
                for entity in open.iter() {
                    commands.entity(entity).despawn();
                }
                *prompt = SlotPrompt::NotFound;
                if let Some(frame) = prompt_frame(SlotPrompt::NotFound) {
                    frame.spawn(&mut commands, &hud, ui_camera.0, SlotPromptUi);
                }
            }
        },
        _ => {}
    }
}

/// Puts a restored game into effect: the counters come back, and the level
/// it was saved at is loaded.
pub fn apply_restored_game(
    mut events: EventReader<RestoredGame>,
    mut score: ResMut<crate::flow::Score>,
    mut stars: ResMut<crate::flow::Stars>,
    mut checkpoint: ResMut<crate::flow::Checkpoint>,
    mut sequence: ResMut<crate::flow::LevelSequence>,
    mut enter: EventWriter<crate::flow::EnterLevel>,
    mut player_q: Query<&mut crate::player::Player>,
) {
    let Some(RestoredGame(save)) = events.read().last() else {
        return;
    };
    score.0 = save.score;
    stars.0 = save.stars as u32;
    checkpoint.score = save.score;
    checkpoint.stars = save.stars as u32;
    checkpoint.health = save.health as i32;
    checkpoint.health_cells = save.health_cells as u32;
    checkpoint.bombs = save.bombs as u32;
    if let Ok(mut player) = player_q.single_mut() {
        player.health = save.health as i32;
        player.health_cells = save.health_cells as u32;
        player.bombs = save.bombs as u32;
    }
    let index = (save.level as usize).min(sequence.order.len().saturating_sub(1));
    sequence.index = index;
    enter.write(crate::flow::EnterLevel {
        level: sequence.current().to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveGame {
        SaveGame {
            health: 3,
            score: 123_456,
            stars: 17,
            level: 5,
            bombs: 2,
            health_cells: 4,
            used_cheat: false,
        }
    }

    #[test]
    fn a_save_round_trips() {
        let s = sample();
        assert_eq!(SaveGame::from_bytes(&s.to_bytes()), Some(s));
    }

    #[test]
    fn the_record_is_the_size_the_original_writes() {
        // Ten 16-bit words plus a 32-bit score: 24 bytes. A different
        // length means the original could not read it.
        assert_eq!(sample().to_bytes().len(), 24);
    }

    #[test]
    fn the_fields_sit_where_the_original_put_them() {
        let b = sample().to_bytes();
        assert_eq!(&b[0..2], &3u16.to_le_bytes(), "health first");
        assert_eq!(&b[2..6], &123_456u32.to_le_bytes(), "then the 32-bit score");
        assert_eq!(&b[6..8], &17u16.to_le_bytes(), "stars");
        assert_eq!(&b[8..10], &5u16.to_le_bytes(), "level");
    }

    #[test]
    fn saving_marks_the_hints_as_already_seen() {
        // game1.c:9369-9371 writes them as seen unconditionally, so a
        // restored game does not explain bombs and pouncing again.
        let b = sample().to_bytes();
        assert_eq!(get_u16(&b, 16), Some(1), "bomb hint seen");
        assert_eq!(get_u16(&b, 18), Some(2), "POUNCE_HINT_SEEN");
    }

    #[test]
    fn a_corrupt_save_is_refused() {
        let mut b = sample().to_bytes();
        b[8] ^= 0xFF; // change the level, leave the checksum
        assert_eq!(SaveGame::from_bytes(&b), None);
    }

    #[test]
    fn a_truncated_save_is_refused_rather_than_panicking() {
        let b = sample().to_bytes();
        for len in 0..b.len() {
            assert_eq!(SaveGame::from_bytes(&b[..len]), None, "len {len}");
        }
    }

    #[test]
    fn the_checksum_ignores_the_score() {
        // Deliberate in the original: the score is not summed in, so two
        // saves differing only by score share a checksum.
        let a = sample();
        let mut b = sample();
        b.score = 999_999;
        assert_eq!(a.checksum(), b.checksum());
    }

    #[test]
    fn the_filename_matches_the_originals() {
        let p = SaveGame::path(2, 7);
        assert_eq!(p.file_name().unwrap(), "COSMO2.SV7");
    }
}
