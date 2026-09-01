//! The Hall of Fame: ten names and scores, persisted alongside the
//! controls, plus the "you made it" prompt at the end of a game.
//!
//! `ShowHighScoreTable` and `CheckHighScoreAndShow` (game2.c), and the
//! defaults `LoadConfigurationData` writes when there is no config yet
//! (game2.c:2981-3006).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// `HighScoreName` is a 16-byte field and the entry routine reserves two
/// of them, so a name is at most fourteen characters (game2.c, glue.h:91).
pub const MAX_NAME: usize = 14;
pub const TABLE_SIZE: usize = 10;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct HighScore {
    pub name: String,
    pub score: u32,
}

#[derive(Resource, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct HighScores {
    pub entries: Vec<HighScore>,
}

impl Default for HighScores {
    /// The table a fresh install starts with - the Simpsons, as shipped
    /// (game2.c:2981-3006). Note the tenth slot is empty in the original
    /// too: only nine defaults are written.
    fn default() -> Self {
        let seed: [(&str, u32); 9] = [
            ("BART", 1_000_000),
            ("LISA", 900_000),
            ("MARGE", 800_000),
            ("ITCHY", 700_000),
            ("SCRATCHY", 600_000),
            ("MR. BURNS", 500_000),
            ("MAGGIE", 400_000),
            ("KRUSTY", 300_000),
            ("HOMER", 200_000),
        ];
        let mut entries: Vec<HighScore> = seed
            .into_iter()
            .map(|(name, score)| HighScore {
                name: name.to_string(),
                score,
            })
            .collect();
        entries.push(HighScore {
            name: String::new(),
            score: 0,
        });
        HighScores { entries }
    }
}

impl HighScores {
    /// Where a score would land, or `None` if it does not make the table.
    ///
    /// The original walks the table and takes the first slot whose score
    /// is *strictly less* than the new one (game2.c), so matching an
    /// existing score does not displace it.
    pub fn rank_for(&self, score: u32) -> Option<usize> {
        self.entries.iter().position(|e| e.score < score)
    }

    /// Inserts at `rank`, pushing everything below it down and dropping
    /// whatever fell off the end.
    pub fn insert_at(&mut self, rank: usize, name: &str, score: u32) {
        self.entries.insert(
            rank,
            HighScore {
                name: name.chars().take(MAX_NAME).collect(),
                score,
            },
        );
        self.entries.truncate(TABLE_SIZE);
    }

    /// F10 on the table, after the confirmation (game2.c).
    pub fn erase(&mut self) {
        self.entries = (0..TABLE_SIZE)
            .map(|_| HighScore {
                name: String::new(),
                score: 0,
            })
            .collect();
    }

    pub fn path() -> std::path::PathBuf {
        crate::input::config_path().with_file_name("highscores.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice::<HighScores>(&b).ok())
            .filter(|h| h.entries.len() == TABLE_SIZE)
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_vec_pretty(self).map(|b| std::fs::write(&path, b)) {
            Ok(Ok(())) => {}
            _ => warn!("could not save high scores to {}", path.display()),
        }
    }
}

/// The name being typed into the "you made it" prompt.
#[derive(Resource, Default)]
pub struct PendingEntry {
    pub rank: Option<usize>,
    pub score: u32,
    pub name: String,
}

#[derive(Component)]
pub struct NameEntryUi;

/// `CheckHighScoreAndShow` (game2.c). Run on leaving a game: if the score
/// made the table, ask for a name before returning to the menu.
pub fn check_high_score(
    mut commands: Commands,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    scores: Res<HighScores>,
    score: Res<crate::flow::Score>,
    mut pending: ResMut<PendingEntry>,
    mut sfx: EventWriter<crate::sfx::PlaySfx>,
) {
    *pending = PendingEntry::default();
    let Some(rank) = scores.rank_for(score.0) else {
        return;
    };
    pending.rank = Some(rank);
    pending.score = score.0;
    sfx.write(crate::sfx::PlaySfx(crate::sfx::snd::HIGH_SCORE_SET));
    crate::panel::TextFrame::new(5, 7, 36, "You made it into the hall of fame!", "")
        .text(8, "Enter your name:")
        .spawn(&mut commands, &hud, ui_camera.0, NameEntryUi);
}

/// Types the name. Letters, digits, space and a few marks; backspace
/// deletes; Enter or Escape commits what is there.
pub fn name_entry_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    mut pending: ResMut<PendingEntry>,
    mut scores: ResMut<HighScores>,
    open: Query<Entity, With<NameEntryUi>>,
) {
    let Some(rank) = pending.rank else {
        return;
    };
    let mut changed = false;
    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Escape => {
                scores.insert_at(rank, &pending.name.clone(), pending.score);
                scores.save();
                pending.rank = None;
                for entity in &open {
                    commands.entity(entity).despawn();
                }
                return;
            }
            KeyCode::Backspace => {
                pending.name.pop();
                changed = true;
            }
            other => {
                if let Some(c) = key_char(*other) {
                    if pending.name.chars().count() < MAX_NAME {
                        pending.name.push(c);
                        changed = true;
                    }
                }
            }
        }
    }
    if !changed {
        return;
    }
    for entity in &open {
        commands.entity(entity).despawn();
    }
    let typed = pending.name.clone();
    crate::panel::TextFrame::new(5, 7, 36, "You made it into the hall of fame!", "")
        .text(8, &format!("Enter your name: {typed}"))
        .spawn(&mut commands, &hud, ui_camera.0, NameEntryUi);
}

/// The characters the name prompt accepts. The original echoes whatever
/// the keyboard produces; this covers the printable subset a name needs.
fn key_char(key: KeyCode) -> Option<char> {
    use KeyCode::*;
    Some(match key {
        KeyA => 'A', KeyB => 'B', KeyC => 'C', KeyD => 'D', KeyE => 'E',
        KeyF => 'F', KeyG => 'G', KeyH => 'H', KeyI => 'I', KeyJ => 'J',
        KeyK => 'K', KeyL => 'L', KeyM => 'M', KeyN => 'N', KeyO => 'O',
        KeyP => 'P', KeyQ => 'Q', KeyR => 'R', KeyS => 'S', KeyT => 'T',
        KeyU => 'U', KeyV => 'V', KeyW => 'W', KeyX => 'X', KeyY => 'Y',
        KeyZ => 'Z',
        Digit0 => '0', Digit1 => '1', Digit2 => '2', Digit3 => '3',
        Digit4 => '4', Digit5 => '5', Digit6 => '6', Digit7 => '7',
        Digit8 => '8', Digit9 => '9',
        Space => ' ',
        Period => '.',
        Minus => '-',
        _ => return None,
    })
}

/// Marks the Hall of Fame frame.
#[derive(Component)]
pub struct HighScoreUi;

/// Where the table screen is in its little flow.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum TableMode {
    #[default]
    Table,
    /// F10 asks before erasing (game2.c).
    ConfirmErase,
}

fn draw_table(
    commands: &mut Commands,
    hud: &crate::hud::HudAssets,
    ui_camera: Entity,
    scores: &HighScores,
    mode: TableMode,
    open: &Query<Entity, With<HighScoreUi>>,
) {
    for entity in open.iter() {
        commands.entity(entity).despawn();
    }
    if mode == TableMode::ConfirmErase {
        crate::panel::TextFrame::new(5, 4, 28, "Are you sure you want to", "ERASE High Scores?")
            .text(7, "  Y / N")
            .spawn(commands, hud, ui_camera, HighScoreUi);
        return;
    }
    let mut frame =
        crate::panel::TextFrame::new(2, 17, 30, "Hall of Fame", "any other key to exit.");
    let x = frame.text_x();
    for (i, e) in scores.entries.iter().enumerate() {
        let row = i as i32 + 5;
        frame = frame.line(x + 1, row, &format!("{:2}.", i + 1));
        // Scores are flush right against a fixed column, so the table
        // stays a column however many digits each has.
        let value = e.score.to_string();
        frame = frame.line(x + 12 - value.len() as i32, row, &value);
        frame = frame.line(x + 13, row, &e.name);
    }
    frame
        .line(x + 3, 16, "Press 'F10' to erase or")
        .spawn(commands, hud, ui_camera, HighScoreUi);
}

pub fn spawn_high_scores(
    mut commands: Commands,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    scores: Res<HighScores>,
    mut mode: ResMut<TableMode>,
    open: Query<Entity, With<HighScoreUi>>,
) {
    *mode = TableMode::Table;
    draw_table(&mut commands, &hud, ui_camera.0, &scores, *mode, &open);
}

pub fn high_scores_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    hud: Res<crate::hud::HudAssets>,
    ui_camera: Res<crate::screen::UiCamera>,
    mut scores: ResMut<HighScores>,
    mut mode: ResMut<TableMode>,
    mut next: ResMut<NextState<crate::screen::GameState>>,
    open: Query<Entity, With<HighScoreUi>>,
) {
    match *mode {
        TableMode::ConfirmErase => {
            if keys.just_pressed(KeyCode::KeyY) {
                scores.erase();
                scores.save();
                *mode = TableMode::Table;
                draw_table(&mut commands, &hud, ui_camera.0, &scores, *mode, &open);
            } else if keys.get_just_pressed().next().is_some() {
                *mode = TableMode::Table;
                draw_table(&mut commands, &hud, ui_camera.0, &scores, *mode, &open);
            }
        }
        TableMode::Table => {
            if keys.just_pressed(KeyCode::F10) {
                *mode = TableMode::ConfirmErase;
                draw_table(&mut commands, &hud, ui_camera.0, &scores, *mode, &open);
            } else if keys.get_just_pressed().next().is_some() {
                next.set(crate::screen::GameState::Menu);
            }
        }
    }
}

pub fn despawn_high_scores(mut commands: Commands, open: Query<Entity, With<HighScoreUi>>) {
    for entity in &open {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_table_is_the_one_the_game_ships() {
        let h = HighScores::default();
        assert_eq!(h.entries.len(), TABLE_SIZE);
        assert_eq!(h.entries[0].name, "BART");
        assert_eq!(h.entries[0].score, 1_000_000);
        assert_eq!(h.entries[8].name, "HOMER");
        // The original only seeds nine (game2.c:2981-3006).
        assert_eq!(h.entries[9].score, 0);
        assert!(h.entries[9].name.is_empty());
    }

    #[test]
    fn the_table_is_in_descending_order() {
        let h = HighScores::default();
        assert!(h.entries.windows(2).all(|w| w[0].score >= w[1].score));
    }

    #[test]
    fn a_score_lands_where_it_beats_someone() {
        let h = HighScores::default();
        assert_eq!(h.rank_for(2_000_000), Some(0), "beating everyone is first");
        // 1,000,000 BART / 900,000 LISA / 800,000 MARGE
        assert_eq!(h.rank_for(950_000), Some(1), "between Bart and Lisa");
        assert_eq!(h.rank_for(850_000), Some(2), "between Lisa and Marge");
        assert_eq!(h.rank_for(1), Some(9), "anything beats the empty slot");
        assert_eq!(h.rank_for(0), None, "and nothing beats nothing");
    }

    #[test]
    fn an_equal_score_does_not_displace_the_holder() {
        // The test is `<`, not `<=` - matching Bart does not unseat him.
        let h = HighScores::default();
        assert_eq!(h.rank_for(1_000_000), Some(1));
    }

    #[test]
    fn inserting_pushes_the_rest_down_and_drops_the_last() {
        let mut h = HighScores::default();
        let rank = h.rank_for(950_000).unwrap();
        h.insert_at(rank, "NISSE", 950_000);
        assert_eq!(h.entries.len(), TABLE_SIZE, "the table never grows");
        assert_eq!(h.entries[0].name, "BART");
        assert_eq!(h.entries[1].name, "NISSE");
        assert_eq!(h.entries[2].name, "LISA");
        assert_eq!(h.entries[9].name, "HOMER", "Homer moved down, nobody lost");
    }

    #[test]
    fn a_long_name_is_cut_rather_than_overflowing_the_frame() {
        let mut h = HighScores::default();
        h.insert_at(0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 9_999_999);
        assert_eq!(h.entries[0].name.chars().count(), MAX_NAME);
    }

    #[test]
    fn erasing_clears_every_slot() {
        let mut h = HighScores::default();
        h.erase();
        assert_eq!(h.entries.len(), TABLE_SIZE);
        assert!(h.entries.iter().all(|e| e.score == 0 && e.name.is_empty()));
        assert_eq!(h.rank_for(1), Some(0), "and the table is open again");
    }

    #[test]
    fn the_name_prompt_takes_the_characters_a_name_needs() {
        use bevy::prelude::KeyCode;
        assert_eq!(key_char(KeyCode::KeyA), Some('A'));
        assert_eq!(key_char(KeyCode::Digit7), Some('7'));
        assert_eq!(key_char(KeyCode::Space), Some(' '));
        assert_eq!(key_char(KeyCode::Period), Some('.'));
        // "MR. BURNS" is a shipped default, so space and period both have
        // to work or the table could not hold its own contents.
        assert!("MR. BURNS".chars().all(|c| c == ' '
            || c == '.'
            || ('A'..='Z').contains(&c)));
        assert_eq!(key_char(KeyCode::F10), None, "not every key is a letter");
    }

    #[test]
    fn a_saved_table_round_trips() {
        let mut h = HighScores::default();
        h.insert_at(0, "NISSE", 1_234_567);
        let json = serde_json::to_vec(&h).unwrap();
        let back: HighScores = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, h);
    }
}
