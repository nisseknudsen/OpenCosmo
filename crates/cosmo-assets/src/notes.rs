//! Recovering the *score* from an IMF register stream.
//!
//! The music files are a log of writes to an OPL2's registers, which is
//! usually treated as an opaque thing to replay into an emulator. It isn't:
//! the registers that matter for pitch and timing are documented hardware,
//! so the stream can be read back as notes.
//!
//! - `0xA0..0xA8` hold the low 8 bits of a channel's F-number.
//! - `0xB0..0xB8` hold the top 2 bits (0-1), the block/octave (2-4), and the
//!   key-on bit (5).
//! - Frequency is `fnum * 49716 / 2^(20 - block)`, where 49716 Hz is the
//!   OPL2's own sample rate (14.318181 MHz / 288).
//! - `0x40..0x55` hold each operator's total level, 0.75 dB per step with 0
//!   the loudest. The *carrier's* level is what sets a two-operator voice's
//!   output volume, so it stands in for velocity.
//!
//! One thing could have made this ambiguous: OPL2's rhythm mode
//! (`0xBD` bit 5) reassigns channels 6-8 to five percussion instruments,
//! which would need unpicking separately. Every track in all three episodes
//! leaves `0xBD` at zero, so every channel is a straightforward melodic
//! voice and the decode is unambiguous.

use crate::music::{ImfEvent, TICK_HZ};

/// The OPL2's internal sample rate, which sets the F-number scale.
const OPL_RATE: f64 = 49716.0;

/// Carrier operator offsets for channels 0..8 (the OPL2's non-contiguous
/// slot numbering).
const CARRIER_SLOT: [usize; 9] = [0x03, 0x04, 0x05, 0x0b, 0x0c, 0x0d, 0x13, 0x14, 0x15];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub channel: u8,
    /// In IMF ticks (1/560 s).
    pub start_tick: u64,
    pub end_tick: u64,
    pub freq: f32,
    /// 0..1, from the carrier's total level.
    pub velocity: f32,
}

impl Note {
    pub fn start_secs(&self) -> f64 {
        self.start_tick as f64 / TICK_HZ
    }
    pub fn end_secs(&self) -> f64 {
        self.end_tick as f64 / TICK_HZ
    }
}

/// True if any track in this stream switches the OPL2 into rhythm mode, in
/// which case channels 6-8 are percussion and this decode does not apply.
pub fn uses_rhythm_mode(events: &[ImfEvent]) -> bool {
    events
        .iter()
        .any(|e| e.register == 0xbd && e.value & 0x20 != 0)
}

pub fn extract_notes(events: &[ImfEvent]) -> Vec<Note> {
    let mut fnum_low = [0u16; 9];
    let mut fnum_high = [0u16; 9];
    let mut block = [0u32; 9];
    let mut total_level = [0u8; 0x16];
    let mut sounding: [Option<Note>; 9] = [None; 9];

    let mut notes = Vec::new();
    let mut tick: u64 = 0;

    for ev in events {
        let reg = ev.register as usize;
        match reg {
            0x40..=0x55 => total_level[reg - 0x40] = ev.value & 0x3f,
            0xa0..=0xa8 => fnum_low[reg - 0xa0] = ev.value as u16,
            0xb0..=0xb8 => {
                let ch = reg - 0xb0;
                fnum_high[ch] = (ev.value & 0x03) as u16;
                block[ch] = ((ev.value >> 2) & 0x07) as u32;
                let key_on = ev.value & 0x20 != 0;

                // A key-on while already sounding is a re-trigger: end the
                // old note here and start a new one, which is what the chip
                // does and what keeps repeated notes from merging into one.
                if let Some(mut note) = sounding[ch].take() {
                    note.end_tick = tick;
                    if note.end_tick > note.start_tick {
                        notes.push(note);
                    }
                }
                if key_on {
                    let fnum = (fnum_high[ch] << 8) | fnum_low[ch];
                    let freq = fnum as f64 * OPL_RATE / (1u64 << (20 - block[ch])) as f64;
                    // Nothing musical lives outside this range; a stray
                    // register write mid-reconfiguration can produce one.
                    if (20.0..8000.0).contains(&freq) {
                        sounding[ch] = Some(Note {
                            channel: ch as u8,
                            start_tick: tick,
                            end_tick: tick,
                            freq: freq as f32,
                            velocity: level_to_velocity(total_level[CARRIER_SLOT[ch]]),
                        });
                    }
                }
            }
            _ => {}
        }
        tick += ev.delay_ticks as u64;
    }

    // Anything still held when the log ends stops there.
    for note in sounding.into_iter().flatten() {
        let mut note = note;
        note.end_tick = tick;
        if note.end_tick > note.start_tick {
            notes.push(note);
        }
    }

    notes.sort_by_key(|n| (n.start_tick, n.channel));
    notes
}

/// Total level is attenuation: 0 is loudest, each step is -0.75 dB.
fn level_to_velocity(level: u8) -> f32 {
    let db = -0.75 * level as f32;
    10f32.powf(db / 20.0)
}

/// Median pitch per channel, used to decide which voice a channel gets.
/// Returns `None` for channels that never sound.
pub fn channel_median_pitch(notes: &[Note]) -> [Option<f32>; 9] {
    let mut out = [None; 9];
    for ch in 0..9u8 {
        let mut pitches: Vec<f32> = notes
            .iter()
            .filter(|n| n.channel == ch)
            .map(|n| n.freq)
            .collect();
        if pitches.is_empty() {
            continue;
        }
        pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out[ch as usize] = Some(pitches[pitches.len() / 2]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(register: u8, value: u8, delay_ticks: u16) -> ImfEvent {
        ImfEvent {
            register,
            value,
            delay_ticks,
        }
    }

    /// Middle-ish A: block 4, fnum 577 lands near 440 Hz.
    #[test]
    fn a_key_on_and_key_off_becomes_one_note_at_the_right_pitch() {
        let fnum: u16 = 577;
        let block: u8 = 4;
        let events = vec![
            ev(0x43, 0, 0), // carrier at full level
            ev(0xa0, (fnum & 0xff) as u8, 0),
            ev(0xb0, 0x20 | (block << 2) | (fnum >> 8) as u8, 560),
            ev(0xb0, (block << 2) | (fnum >> 8) as u8, 0),
        ];
        let notes = extract_notes(&events);
        assert_eq!(notes.len(), 1);
        let n = notes[0];
        assert_eq!(n.channel, 0);
        assert_eq!(n.start_tick, 0);
        assert_eq!(n.end_tick, 560);
        assert!((n.end_secs() - n.start_secs() - 1.0).abs() < 1e-6, "one second");
        assert!(
            (n.freq - 440.0).abs() < 5.0,
            "expected ~440Hz, got {}",
            n.freq
        );
        assert!((n.velocity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn re_triggering_without_a_key_off_splits_into_two_notes() {
        // Repeated notes are keyed on again while still sounding; merging
        // them would turn a bassline into one long drone.
        let on = 0x20 | (4 << 2);
        let events = vec![
            ev(0xa0, 0x41, 0),
            ev(0xb0, on, 100),
            ev(0xb0, on, 100),
            ev(0xb0, 4 << 2, 0),
        ];
        let notes = extract_notes(&events);
        assert_eq!(notes.len(), 2);
        assert_eq!((notes[0].start_tick, notes[0].end_tick), (0, 100));
        assert_eq!((notes[1].start_tick, notes[1].end_tick), (100, 200));
    }

    #[test]
    fn channels_are_independent() {
        let on = 0x20 | (4 << 2);
        let events = vec![
            ev(0xa0, 0x41, 0),
            ev(0xa3, 0x41, 0),
            ev(0xb0, on, 0),
            ev(0xb3, on, 50),
            ev(0xb0, 4 << 2, 50),
            ev(0xb3, 4 << 2, 0),
        ];
        let notes = extract_notes(&events);
        assert_eq!(notes.len(), 2);
        let ch0 = notes.iter().find(|n| n.channel == 0).unwrap();
        let ch3 = notes.iter().find(|n| n.channel == 3).unwrap();
        assert_eq!(ch0.end_tick, 50);
        assert_eq!(ch3.end_tick, 100);
    }

    #[test]
    fn a_note_still_held_when_the_log_ends_is_closed_out() {
        let events = vec![ev(0xa0, 0x41, 0), ev(0xb0, 0x20 | (4 << 2), 300)];
        let notes = extract_notes(&events);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].end_tick, 300);
    }

    #[test]
    fn quieter_carrier_levels_give_quieter_notes() {
        let mk = |level: u8| {
            let events = vec![
                ev(0x43, level, 0),
                ev(0xa0, 0x41, 0),
                ev(0xb0, 0x20 | (4 << 2), 100),
                ev(0xb0, 4 << 2, 0),
            ];
            extract_notes(&events)[0].velocity
        };
        assert!(mk(0) > mk(16));
        assert!(mk(16) > mk(40));
        // -0.75 dB per step: 16 steps is -12 dB, a quarter of the amplitude.
        assert!((mk(16) / mk(0) - 0.251).abs() < 0.01);
    }

    #[test]
    fn nonsense_pitches_from_mid_reconfiguration_writes_are_dropped() {
        // fnum 0 with the key-on bit set is silence, not a 0 Hz note.
        let events = vec![ev(0xa0, 0, 0), ev(0xb0, 0x20, 100), ev(0xb0, 0, 0)];
        assert!(extract_notes(&events).is_empty());
    }

    #[test]
    fn rhythm_mode_is_detected_when_present() {
        assert!(!uses_rhythm_mode(&[ev(0xbd, 0x00, 0)]));
        assert!(uses_rhythm_mode(&[ev(0xbd, 0x20, 0)]));
    }

    #[test]
    fn median_pitch_separates_a_bass_channel_from_a_lead() {
        let notes = vec![
            Note { channel: 0, start_tick: 0, end_tick: 1, freq: 100.0, velocity: 1.0 },
            Note { channel: 0, start_tick: 1, end_tick: 2, freq: 120.0, velocity: 1.0 },
            Note { channel: 1, start_tick: 0, end_tick: 1, freq: 900.0, velocity: 1.0 },
        ];
        let medians = channel_median_pitch(&notes);
        assert!(medians[0].unwrap() < medians[1].unwrap());
        assert!(medians[2].is_none());
    }
}
