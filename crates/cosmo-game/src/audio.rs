use crate::level::CurrentLevel;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::prelude::*;

#[derive(Component)]
pub struct MusicTrack;

/// (Re)starts the looping music track whenever the current level's assigned
/// track name changes. Original behavior (`AdLibService()`, game2.c:356-360)
/// loops indefinitely during gameplay - matched here via `PlaybackSettings::LOOP`.
pub fn update_music(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level: Res<CurrentLevel>,
    mut current_track: Local<Option<String>>,
    existing: Query<Entity, With<MusicTrack>>,
) {
    if !level.is_changed() {
        return;
    }
    if current_track.as_deref() == level.music.as_deref() {
        return;
    }
    *current_track = level.music.clone();

    for e in &existing {
        commands.entity(e).despawn();
    }

    let Some(track) = &level.music else {
        return;
    };
    let handle: Handle<AudioSource> =
        asset_server.load(format!("generated/music/{track}.wav"));
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings::LOOP,
        MusicTrack,
    ));
}
