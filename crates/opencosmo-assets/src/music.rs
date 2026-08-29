//! IMF music decoding and OPL2 rendering. Confirmed against cosmore's
//! `AdLibService()` (game2.c:335-354), `SwitchMusic()`/`LoadMusicData()`
//! (game2.c:603-618, 1561-1576), and `SetInterruptRate()`/`SetPIT0Value()`
//! (game2.c:148-182):
//!
//! - **On-disk format**: a flat stream of 4-byte records with **no
//!   header/length prefix** - `LoadMusicData()`'s `Music.length` field is
//!   populated from the VOL directory's own recorded entry size
//!   (`lastGroupEntryLength`), not read from inside the file. Empirically
//!   confirmed too: every real `M*.MNI` file's byte size is an exact
//!   multiple of 4 (e.g. MCAVES.MNI = 17080 = 4270*4). Each record is
//!   `{register: u8, value: u8, delay: u16 LE}` - standard IMF.
//! - **Playback semantics**: `AdLibService()` applies `(register, value)` to
//!   the AdLib immediately, then computes `musicNextDue = musicTickCount +
//!   delay`, which gates when the *next* record becomes eligible (checked at
//!   the top of the `while` loop) - i.e. `delay` is consumed *after* the
//!   register write, standard "delay-after" IMF semantics. Multiple records
//!   with `delay == 0` apply within the same tick.
//! - **Tick rate**: `InitializeInterruptRate()` (game2.c:463-474) programs
//!   the PIT to literally 560 Hz via `SetInterruptRate(560)` ->
//!   `SetPIT0Value(1192030 / 560)` (game2.c:180-182: `SetInterruptRate`'s own
//!   doc comment confirms its argument is literal interrupts-per-second) when
//!   AdLib is active, and `AdLibService()` runs once per timer interrupt
//!   (game2.c:427-428, unconditionally - unlike the PC speaker service, which
//!   is only serviced every 2nd/4th interrupt). So **1 tick = 1/560 second**.
//!   This is the well-known "Apogee-flavor" IMF tick rate, distinct from id
//!   Software's own titles which typically use 700Hz.
//! - **Looping**: `AdLibService()` restarts from the beginning once
//!   `musicDataLeft` reaches 0 (game2.c:356-360) - tracks loop indefinitely
//!   during real gameplay. This module renders exactly one pass; runtime
//!   looping is a playback-time concern for the game, not this converter.
//!
//! OPL2 FM synthesis itself is delegated to `nuked-opl3`, a pure-Rust port of
//! the cycle-accurate Nuked-OPL3 emulator (register-compatible with OPL2 as
//! long as OPL3 mode and bank-1 registers are never touched, which our
//! straight OPL2 register stream never does).

use anyhow::{ensure, Context, Result};
use nuked_opl3::Opl3Chip;
use std::path::Path;

/// AdLibService() timer interrupt rate when music is playing (game2.c:468).
pub const TICK_HZ: f64 = 560.0;

#[derive(Debug, Clone, Copy)]
pub struct ImfEvent {
    pub register: u8,
    pub value: u8,
    pub delay_ticks: u16,
}

pub fn parse_imf(bytes: &[u8]) -> Result<Vec<ImfEvent>> {
    ensure!(
        bytes.len() % 4 == 0,
        "music data length {} is not a multiple of 4 (not a headerless IMF stream)",
        bytes.len()
    );
    Ok(bytes
        .chunks_exact(4)
        .map(|c| ImfEvent {
            register: c[0],
            value: c[1],
            delay_ticks: u16::from_le_bytes([c[2], c[3]]),
        })
        .collect())
}

/// Total playback duration of one pass through `events`, in ticks.
pub fn duration_ticks(events: &[ImfEvent]) -> u64 {
    events.iter().map(|e| e.delay_ticks as u64).sum()
}

/// Renders one pass through `events` to interleaved stereo i16 PCM.
pub fn render_to_pcm(events: &[ImfEvent], sample_rate: u32) -> Vec<i16> {
    let mut chip = Opl3Chip::new(sample_rate);

    // Mirror the original's one-time AdLib init (DetectAdLib(), game2.c:396-410):
    // zero every register, then enable WSE (waveform select - permits the
    // non-sine OPL2 waveforms most instrument voices rely on) and clear
    // CSM/NOTE_SEL. The emulator already starts zeroed, but WSE defaults off.
    for reg in 0x01u16..=0xf5 {
        chip.write_register(reg, 0);
    }
    chip.write_register(0x01, 0x20);
    chip.write_register(0x08, 0x00);

    let mut out = Vec::new();
    let mut pair = [0i16; 2];
    for ev in events {
        chip.write_register(ev.register as u16, ev.value);
        if ev.delay_ticks == 0 {
            continue;
        }
        let sample_count = ((ev.delay_ticks as f64 / TICK_HZ) * sample_rate as f64).round() as usize;
        for _ in 0..sample_count {
            let _ = chip.generate(&mut pair);
            out.push(pair[0]);
            out.push(pair[1]);
        }
    }
    out
}

pub fn render_to_wav(events: &[ImfEvent], sample_rate: u32, out_path: &Path) -> Result<()> {
    let pcm = render_to_pcm(events, sample_rate);
    let spec = hound::WavSpec {
        channels: 2,
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
