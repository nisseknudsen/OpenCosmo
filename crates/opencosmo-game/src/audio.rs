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
pub fn update_music(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level: Res<CurrentLevel>,
    mode: Res<AudioMode>,
    mut playing: Local<Option<(String, AudioMode)>>,
    existing: Query<Entity, With<MusicTrack>>,
) {
    if !level.is_changed() && !mode.is_changed() {
        return;
    }
    let Some(track) = level.music.clone() else {
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
