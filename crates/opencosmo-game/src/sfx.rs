//! PC speaker sound effect playback.
//!
//! The effects themselves are decoded and rendered to WAV at asset-conversion
//! time (see `opencosmo_assets::sound`); this module loads them and reproduces
//! the original's *dispatch* behaviour, which is not simply "play a clip".
//!
//! A real PC speaker can only make one sound at a time, so `StartSound()`
//! (game1.c:628-635) is monophonic with priorities: a new effect is dropped
//! outright unless its priority is at least that of whatever is already
//! playing, and `PCSpeakerService()` clears the active priority back to 0
//! when a sound reaches its `END_SOUND` terminator (game1.c:8056-8060).
//! Reproducing that matters for feel as much as faithfulness - played
//! polyphonically, footfalls and jumps would pile up into mush, and quiet
//! effects would talk over the death jingle.
//!
//! Sound numbers are the `SND_*` constants (sound.h:28-104), 1-based.

use crate::data::GameData;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::Deserialize;

/// The `SND_*` numbers (sound.h:27-91), all of them - the converter
/// produces every effect, so the only thing that ever limited which are
/// heard is which behaviours ask for one.
pub mod snd {
    pub const BIG_PRIZE: u16 = 1;
    pub const PLAYER_JUMP: u16 = 2;
    pub const PLAYER_LAND: u16 = 3;
    pub const PLAYER_CLING: u16 = 4;
    pub const PLAYER_HIT_HEAD: u16 = 5;
    pub const PLAYER_POUNCE: u16 = 6;
    pub const PLAYER_DEATH: u16 = 7;
    pub const DOOR_UNLOCK: u16 = 8;
    pub const SPIKES_MOVE: u16 = 9;
    pub const EXPLOSION: u16 = 10;
    pub const WIN_LEVEL: u16 = 11;
    pub const BARREL_DESTROY_1: u16 = 12;
    pub const PRIZE: u16 = 13;
    pub const PLAYER_HURT: u16 = 14;
    pub const FOOT_SWITCH_MOVE: u16 = 15;
    pub const FOOT_SWITCH_ON: u16 = 16;
    pub const ROAMER_GIFT: u16 = 17;
    pub const DESTROY_SATELLITE: u16 = 18;
    pub const PLAYER_FOOTSTEP: u16 = 19;
    pub const PUSH_PLAYER: u16 = 20;
    pub const DRIP: u16 = 21;
    pub const PIPE_CORNER_HIT: u16 = 22;
    pub const TRANSPORTER_ON: u16 = 23;
    pub const SCOOTER_PUTT: u16 = 24;
    pub const DESTROY_SOLID: u16 = 25;
    pub const PROJECTILE_LAUNCH: u16 = 26;
    pub const BIG_OBJECT_HIT: u16 = 27;
    pub const NO_BOMBS: u16 = 28;
    pub const PLACE_BOMB: u16 = 29;
    pub const HINT_DIALOG_ALERT: u16 = 30;
    pub const RED_JUMPER_JUMP: u16 = 31;
    pub const RED_JUMPER_LAND: u16 = 32;
    pub const BGHOST_EGG_CRACK: u16 = 33;
    pub const BGHOST_EGG_HATCH: u16 = 34;
    pub const SAW_BLADE_MOVE: u16 = 35;
    pub const FIREBALL_LAUNCH: u16 = 36;
    pub const OBJECT_HIT: u16 = 37;
    pub const EXIT_MONSTER_OPEN: u16 = 38;
    pub const EXIT_MONSTER_INGEST: u16 = 39;
    pub const BEAR_TRAP_CLOSE: u16 = 40;
    pub const PAUSE_GAME: u16 = 41;
    pub const WEEEEEEEE: u16 = 42;
    pub const JUMP_PAD_ROBOT: u16 = 43;
    pub const TEXT_TYPEWRITER: u16 = 44;
    pub const BONUS_STAGE: u16 = 45;
    pub const SHARD_BOUNCE: u16 = 46;
    pub const TULIP_LAUNCH: u16 = 47;
    pub const NEW_GAME: u16 = 48;
    pub const ROCKET_BURN: u16 = 49;
    pub const SMASH: u16 = 50;
    pub const HIGH_SCORE_DISPLAY: u16 = 51;
    pub const HIGH_SCORE_SET: u16 = 52;
    pub const IVY_PLANT_RISE: u16 = 53;
    pub const FLAME_PULSE: u16 = 54;
    pub const BOSS_DAMAGE: u16 = 55;
    pub const BOSS_MOVE: u16 = 56;
    pub const SPEECH_BUBBLE: u16 = 57;
    pub const BABY_GHOST_JUMP: u16 = 58;
    pub const BABY_GHOST_LAND: u16 = 59;
    pub const THUNDER: u16 = 60;
    pub const BARREL_DESTROY_2: u16 = 61;
    pub const TULIP_INGEST: u16 = 62;
    pub const PLANT_MOUTH_OPEN: u16 = 63;
    pub const ENTERING_LEVEL_NUM: u16 = 64;
    pub const BOSS_LAUNCH: u16 = 65;
}

/// One sample lasts a single `PCSpeakerService()` call at 140 Hz - see
/// `opencosmo_assets::sound` for the derivation.
const TICK_HZ: f64 = opencosmo_assets::sound::TICK_HZ;

/// Request to play an effect. Triggers fire these rather than spawning
/// audio directly, so the priority rule lives in exactly one place.
#[derive(Event, Clone, Copy)]
pub struct PlaySfx(pub u16);

#[derive(Deserialize)]
struct SoundManifestEntry {
    number: usize,
    stem: String,
    priority: u8,
    ticks: usize,
}

struct Clip {
    handle: Handle<AudioSource>,
    priority: u8,
    duration: f64,
}

#[derive(Resource, Default)]
pub struct SfxAssets {
    clips: HashMap<u16, Clip>,
}

impl SfxAssets {
    pub fn load(asset_server: &AssetServer, data: &GameData) -> Self {
        let path = data.root.join("sfx").join("manifest.json");
        let entries: Vec<SoundManifestEntry> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();

        let clips = entries
            .into_iter()
            .map(|e| {
                (
                    e.number as u16,
                    Clip {
                        handle: asset_server.load(crate::data::asset_path(&format!("sfx/{}.wav", e.stem))),
                        priority: e.priority,
                        duration: e.ticks as f64 / TICK_HZ,
                    },
                )
            })
            .collect();
        Self { clips }
    }
}

/// Mirrors the original's `activeSoundPriority` (game1.c:216).
#[derive(Resource, Default)]
pub struct SfxState {
    active_priority: u8,
    /// Elapsed-time stamp at which the active effect reaches `END_SOUND`.
    active_until: f64,
    playing: Option<Entity>,
}

impl SfxState {
    /// Frees the channel once the active effect has run out, mirroring
    /// `PCSpeakerService()` clearing `activeSoundPriority` at `END_SOUND`
    /// (game1.c:8056-8060).
    fn release_if_finished(&mut self, now: f64) {
        if now >= self.active_until {
            self.active_priority = 0;
        }
    }

    /// `StartSound`'s gate: anything quieter than what's playing is dropped
    /// outright (game1.c:630).
    fn accepts(&self, priority: u8) -> bool {
        priority >= self.active_priority
    }

    fn begin(&mut self, priority: u8, now: f64, duration: f64) {
        self.active_priority = priority;
        self.active_until = now + duration;
    }
}

/// Drains queued requests and plays at most one, applying the original's
/// priority rule.
pub fn play_queued_sfx(
    mut commands: Commands,
    mut events: EventReader<PlaySfx>,
    assets: Res<SfxAssets>,
    mut state: ResMut<SfxState>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    state.release_if_finished(now);
    if state.active_priority == 0 {
        state.playing = None;
    }

    for PlaySfx(number) in events.read().copied() {
        let Some(clip) = assets.clips.get(&number) else {
            continue;
        };
        if !state.accepts(clip.priority) {
            continue;
        }

        // Equal-or-higher priority interrupts, so stop the old one first.
        if let Some(entity) = state.playing.take() {
            commands.entity(entity).despawn();
        }

        let entity = commands
            .spawn((AudioPlayer(clip.handle.clone()), PlaybackSettings::DESPAWN))
            .id();
        state.begin(clip.priority, now, clip.duration);
        state.playing = Some(entity);
    }
}

/// Silences any effect still playing when a level or screen is torn down.
pub fn stop_all_sfx(mut commands: Commands, mut state: ResMut<SfxState>) {
    if let Some(entity) = state.playing.take() {
        commands.entity(entity).despawn();
    }
    state.active_priority = 0;
    state.active_until = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Priorities and durations of two real effects, from the converted
    /// manifest: the jump chirp is the lowest priority in the game, the
    /// death jingle the highest.
    const JUMP: (u8, f64) = (1, 18.0 / TICK_HZ);
    const DEATH: (u8, f64) = (255, 151.0 / TICK_HZ);

    #[test]
    fn a_quieter_effect_cannot_interrupt_a_louder_one() {
        let mut state = SfxState::default();
        state.begin(DEATH.0, 0.0, DEATH.1);
        state.release_if_finished(0.1);
        assert!(
            !state.accepts(JUMP.0),
            "a jump must not cut off the death jingle"
        );
    }

    #[test]
    fn equal_priority_interrupts() {
        let mut state = SfxState::default();
        state.begin(JUMP.0, 0.0, JUMP.1);
        state.release_if_finished(0.01);
        // StartSound's test is `<`, so equal priority is allowed through -
        // consecutive footfalls retrigger rather than being swallowed.
        assert!(state.accepts(JUMP.0));
    }

    #[test]
    fn the_channel_frees_once_the_effect_ends() {
        let mut state = SfxState::default();
        state.begin(DEATH.0, 0.0, DEATH.1);
        state.release_if_finished(DEATH.1 - 0.01);
        assert!(!state.accepts(JUMP.0), "still playing, so still blocking");

        state.release_if_finished(DEATH.1 + 0.01);
        assert!(state.accepts(JUMP.0), "finished, so anything may play");
        assert_eq!(state.active_priority, 0);
    }

    #[test]
    fn an_idle_channel_accepts_anything() {
        let state = SfxState::default();
        assert!(state.accepts(0));
        assert!(state.accepts(JUMP.0));
        assert!(state.accepts(DEATH.0));
    }
}
