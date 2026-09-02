use crate::level::CurrentLevel;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::prelude::*;

#[derive(Component)]
pub struct MusicTrack;

/// Which soundtrack plays.
///
/// Both are the same music: the remastered mix is rendered from the notes
/// recovered out of the original's own register stream, so it is Bobby
/// Prince's composition either way - only the instruments differ. See
/// `opencosmo_assets::notes` and `opencosmo_assets::lofi`.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioMode {
    /// The AdLib OPL2 mix, as the hardware produced it.
    Authentic,
    /// Re-voiced, lo-fi.
    Remaster,
}

impl AudioMode {
    /// Authentic by default. The re-voiced mix is kept and still reachable
    /// with F6 or `COSMO_AUDIO=remaster`, but it is not what you get on
    /// launch - it was judged not good enough to be the default, and the
    /// approach is being reconsidered.
    pub fn from_env() -> Self {
        match std::env::var("COSMO_AUDIO").as_deref() {
            Ok("remaster") => AudioMode::Remaster,
            _ => AudioMode::Authentic,
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            AudioMode::Authentic => AudioMode::Remaster,
            AudioMode::Remaster => AudioMode::Authentic,
        }
    }

    /// The converted-asset subdirectory this mode's tracks live in.
    pub fn music_dir(self) -> &'static str {
        match self {
            AudioMode::Authentic => "music",
            AudioMode::Remaster => "music_remaster",
        }
    }
}

impl Default for AudioMode {
    fn default() -> Self {
        Self::from_env()
    }
}

/// F6 switches soundtracks, mirroring F5 for the picture.
pub fn toggle_audio_mode(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<AudioMode>) {
    if keys.just_pressed(KeyCode::F6) {
        *mode = mode.toggled();
        info!("audio: {:?}", *mode);
    }
}

/// (Re)starts the looping music track whenever the current level's assigned
/// track name changes, or the player switches soundtracks. The original
/// (`AdLibService()`, game2.c:356-360) loops indefinitely during gameplay -
/// matched here via `PlaybackSettings::LOOP`.
/// A track that replaces the level's own for as long as it is set - the
/// boss fight is the only thing that does this (`StartGameMusic(MUSIC_BOSS)`,
/// game1.c:3626). Cleared when a level loads, so it does not follow the
/// player out of the fight.
#[derive(Resource, Default)]
pub struct MusicOverride(pub Option<&'static str>);

/// `MUSIC_BOSS` (music.h:21).
pub const MUSIC_BOSS: &str = "mboss";

pub fn update_music(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level: Res<CurrentLevel>,
    mode: Res<AudioMode>,
    over: Res<MusicOverride>,
    mut playing: Local<Option<(String, AudioMode)>>,
    existing: Query<Entity, With<MusicTrack>>,
) {
    if !level.is_changed() && !mode.is_changed() && !over.is_changed() {
        return;
    }
    let track = over.0.map(str::to_string).or_else(|| level.music.clone());
    let Some(track) = track else {
        for e in &existing {
            commands.entity(e).despawn();
        }
        *playing = None;
        return;
    };
    if playing.as_ref() == Some(&(track.clone(), *mode)) {
        return;
    }
    *playing = Some((track.clone(), *mode));

    for e in &existing {
        commands.entity(e).despawn();
    }

    let path = format!("{}/{track}.wav", mode.music_dir());
    let handle: Handle<AudioSource> = asset_server.load(crate::data::asset_path(&path));
    commands.spawn((AudioPlayer(handle), PlaybackSettings::LOOP, MusicTrack));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_replaces_the_levels_own_track() {
        // The boss fight is the only thing that does this; everything else
        // plays whatever the level header assigns.
        let over = MusicOverride(Some(MUSIC_BOSS));
        let level_track = Some("mcaves".to_string());
        let chosen = over.0.map(str::to_string).or_else(|| level_track.clone());
        assert_eq!(chosen.as_deref(), Some(MUSIC_BOSS));

        let none = MusicOverride(None);
        let chosen = none.0.map(str::to_string).or_else(|| level_track.clone());
        assert_eq!(chosen.as_deref(), Some("mcaves"), "otherwise the level wins");
    }

    #[test]
    fn the_boss_track_is_the_one_the_game_ships() {
        // MUSIC_BOSS is index 2 (music.h:21), which the converter writes
        // out as mboss.
        assert_eq!(MUSIC_BOSS, "mboss");
    }
}
