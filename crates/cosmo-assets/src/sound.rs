//! PC speaker sound effect decoding and square-wave rendering. Confirmed
//! against cosmore's `LoadSoundData()` (game1.c:606-620), `StartSound()`
//! (game1.c:628-635), `PCSpeakerService()` (game1.c:8044-8079), and
//! `TimerInterruptService()`/`InitializeInterruptRate()` (game2.c:421-474):
//!
//! - **Bank layout**: the three group entries `SOUNDS.MNI`, `SOUNDS2.MNI`
//!   and `SOUNDS3.MNI` each begin with a 16-byte header (magic `"SND\0"`
//!   then three words, of which only the count `0x0018` = 24 is
//!   recognisable), followed by a table of 16-byte records. Record `i`
//!   sits at byte `16 + 16*i`, matching `LoadSoundData`'s word arithmetic
//!   `*(dest + (i * 8) + 8)`.
//! - **Record layout**: `{offset: u16 LE, priority: u8, unknown: u8,
//!   name: [u8; 12]}`. `LoadSoundData` reads the offset as a *byte* offset
//!   from the start of the file (it does `>> 1` to turn it into the word
//!   index its `word*` pointer needs) and truncates the following word to
//!   a byte for the priority, leaving that word's high byte unaccounted
//!   for - it is `0x08` in every record across all three banks. The
//!   12-byte null-padded ASCII name is not read by the game at all; only
//!   `SOUNDS.MNI` fills it in with anything meaningful, the other two
//!   banks store the placeholder `"__UnNamed__"`.
//! - **Only 23 records per bank are loaded**, even though the header
//!   count and the table itself both say 24 - `LoadSoundData`'s loop is
//!   `for (i = 0; i < 23; i++)`. The 24th record's data is present but
//!   unreachable in-game, so it is skipped here too, keeping sound
//!   numbering aligned with the `SND_*` constants.
//! - **Sample stream**: a flat run of `u16` LE values terminated by
//!   `END_SOUND` (`WORD_MAX` = `0xFFFF`, sound.h:22). Each value is a PIT
//!   channel-2 divisor written straight to the timer in square-wave mode
//!   (`outportb(0x0043, 0xb6)`, game1.c:8068-8071), so its pitch is
//!   `1193182 / divisor`. A value of `0` means silence: the service
//!   clears the speaker gate bits rather than reprogramming the timer
//!   (game1.c:8066-8067).
//! - **Tick rate**: one sample is consumed per `PCSpeakerService()` call,
//!   which runs at **140 Hz** on both timer configurations - with AdLib
//!   active the interrupt is 560 Hz and the speaker is serviced every 4th
//!   (game2.c:430-432), and without it the interrupt is already 140 Hz and
//!   the speaker is serviced every time (game2.c:439). So one sample lasts
//!   1/140 s regardless.

use anyhow::{ensure, Context, Result};
use std::path::Path;

/// `PCSpeakerService()` consumes one sample per call, at 140 Hz.
pub const TICK_HZ: f64 = 140.0;

/// The PC's 8253/8254 timer input clock. A divisor `d` written to channel
/// 2 in square-wave mode produces `PIT_INPUT_HZ / d`.
pub const PIT_INPUT_HZ: f64 = 1_193_182.0;

/// `END_SOUND` (sound.h:22).
const END_SOUND: u16 = 0xffff;

/// Records the game actually loads out of each bank's 24-entry table.
pub const SOUNDS_PER_BANK: usize = 23;

const HEADER_BYTES: usize = 16;
const RECORD_BYTES: usize = 16;
const NAME_BYTES: usize = 12;

/// The banks in the order `LoadSoundData` stitches them together
/// (game1.c:8255-8257), which is what makes sound numbering contiguous.
pub const SOUND_BANKS: [&str; 3] = ["SOUNDS.MNI", "SOUNDS2.MNI", "SOUNDS3.MNI"];

/// Square wave peak, ~25% of full scale. PC speaker output is a harsh
/// full-duty square; leaving headroom keeps it from dominating the music.
const AMPLITUDE: i16 = 8_000;

/// `SND_*` names from sound.h:28-104, indexed by `sound number - 1`. The
/// game defines 65 of them but loads 69 slots (23 per bank x 3), so the
/// last four are addressable yet unnamed.
pub const SOUND_NAMES: [&str; 65] = [
    "big_prize",
    "player_jump",
    "player_land",
    "player_cling",
    "player_hit_head",
    "player_pounce",
    "player_death",
    "door_unlock",
    "spikes_move",
    "explosion",
    "win_level",
    "barrel_destroy_1",
    "prize",
    "player_hurt",
    "foot_switch_move",
    "foot_switch_on",
    "roamer_gift",
    "destroy_satellite",
    "player_footstep",
    "push_player",
    "drip",
    "pipe_corner_hit",
    "transporter_on",
    "scooter_putt",
    "destroy_solid",
    "projectile_launch",
    "big_object_hit",
    "no_bombs",
    "place_bomb",
    "hint_dialog_alert",
    "red_jumper_jump",
    "red_jumper_land",
    "bghost_egg_crack",
    "bghost_egg_hatch",
    "saw_blade_move",
    "fireball_launch",
    "object_hit",
    "exit_monster_open",
    "exit_monster_ingest",
    "bear_trap_close",
    "pause_game",
    "weeeeeeee",
    "jump_pad_robot",
    "text_typewriter",
    "bonus_stage",
    "shard_bounce",
    "tulip_launch",
    "new_game",
    "rocket_burn",
    "smash",
    "high_score_display",
    "high_score_set",
    "ivy_plant_rise",
    "flame_pulse",
    "boss_damage",
    "boss_move",
    "speech_bubble",
    "baby_ghost_jump",
    "baby_ghost_land",
    "thunder",
    "barrel_destroy_2",
    "tulip_ingest",
    "plant_mouth_open",
    "entering_level_num",
    "boss_launch",
];

/// One decoded effect. `samples` excludes the `END_SOUND` terminator.
#[derive(Debug, Clone)]
pub struct SoundEffect {
    pub priority: u8,
    /// The bank's own 12-byte label. Only `SOUNDS.MNI` carries real names;
    /// the others store `"__UnNamed__"`.
    pub label: String,
    /// PIT divisors, one per 1/140 s tick. `0` is silence.
    pub samples: Vec<u16>,
}

impl SoundEffect {
    pub fn duration_secs(&self) -> f64 {
        self.samples.len() as f64 / TICK_HZ
    }
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// Decodes the 23 reachable effects from one bank.
pub fn parse_sound_bank(bytes: &[u8]) -> Result<Vec<SoundEffect>> {
    ensure!(
        bytes.len() >= HEADER_BYTES + SOUNDS_PER_BANK * RECORD_BYTES,
        "sound bank is {} bytes, too small for a header plus {} records",
        bytes.len(),
        SOUNDS_PER_BANK
    );
    ensure!(
        &bytes[0..4] == b"SND\0",
        "sound bank does not start with the expected \"SND\\0\" magic"
    );

    let mut effects = Vec::with_capacity(SOUNDS_PER_BANK);
    for i in 0..SOUNDS_PER_BANK {
        let record = HEADER_BYTES + i * RECORD_BYTES;
        let offset = read_u16(bytes, record) as usize;
        let priority = bytes[record + 2];
        let name_bytes = &bytes[record + 4..record + 4 + NAME_BYTES];
        let label = String::from_utf8_lossy(
            &name_bytes[..name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_BYTES)],
        )
        .into_owned();

        ensure!(
            offset + 2 <= bytes.len(),
            "sound record {i} points at byte {offset}, past the end of the bank"
        );

        let mut samples = Vec::new();
        let mut at = offset;
        loop {
            ensure!(
                at + 2 <= bytes.len(),
                "sound record {i} ran off the end of the bank without an END_SOUND terminator"
            );
            let sample = read_u16(bytes, at);
            at += 2;
            if sample == END_SOUND {
                break;
            }
            samples.push(sample);
        }

        effects.push(SoundEffect {
            priority,
            label,
            samples,
        });
    }
    Ok(effects)
}

/// Renders an effect to mono PCM as a square wave.
///
/// Phase carries across sample boundaries because the real hardware simply
/// leaves the timer running; restarting the waveform each tick would add a
/// click at every one of the 140-per-second boundaries.
pub fn render_to_pcm(samples: &[u16], sample_rate: u32) -> Vec<i16> {
    let samples_per_tick = sample_rate as f64 / TICK_HZ;
    let mut out = Vec::with_capacity((samples.len() as f64 * samples_per_tick) as usize);
    let mut phase = 0.0f64;

    for (i, &divisor) in samples.iter().enumerate() {
        // Accumulate boundaries in f64 so rounding can't drift over a long
        // effect the way a per-tick integer count would.
        let start = (i as f64 * samples_per_tick).round() as usize;
        let end = ((i + 1) as f64 * samples_per_tick).round() as usize;
        let count = end.saturating_sub(start);

        if divisor == 0 {
            // Speaker gate cleared - silence, and the waveform restarts
            // cleanly whenever a tone resumes.
            phase = 0.0;
            out.extend(std::iter::repeat_n(0i16, count));
            continue;
        }

        let step = (PIT_INPUT_HZ / divisor as f64) / sample_rate as f64;
        for _ in 0..count {
            out.push(if phase < 0.5 { AMPLITUDE } else { -AMPLITUDE });
            phase += step;
            if phase >= 1.0 {
                phase -= phase.floor();
            }
        }
    }
    out
}

pub fn render_to_wav(samples: &[u16], sample_rate: u32, out_path: &Path) -> Result<()> {
    let pcm = render_to_pcm(samples, sample_rate);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(out_path, spec)
        .with_context(|| format!("creating {}", out_path.display()))?;
    for s in pcm {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

/// File stem for a 1-based sound number, e.g. `2` -> `"02_player_jump"`.
/// Numbering follows the `SND_*` constants, which is exactly the order
/// `LoadSoundData` stitches the three banks into.
pub fn sound_stem(sound_number: usize) -> String {
    match SOUND_NAMES.get(sound_number - 1) {
        Some(name) => format!("{sound_number:02}_{name}"),
        None => format!("{sound_number:02}_unnamed"),
    }
}
