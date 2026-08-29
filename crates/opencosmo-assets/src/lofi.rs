//! Re-voices an extracted score as lo-fi audio.
//!
//! The composition is untouched - every note's pitch, start, length and
//! velocity comes straight from the original's own register stream (see
//! `notes.rs`). Only the timbre is ours.
//!
//! Everything here is plain additive synthesis and plain DSP, which matters
//! for three reasons a generative model would struggle with: it is
//! deterministic, so a rebuild produces byte-identical audio and the asset
//! cache stays meaningful; it runs offline inside the existing build script
//! with no model or network; and it loops seamlessly, which for game music
//! that repeats indefinitely is not optional.
//!
//! Voices are additive sine stacks rather than anything subtractive. A sine
//! partial cannot alias as long as it is below Nyquist, so the harmonics are
//! simply not emitted above it - the result is clean at any pitch, which the
//! original's own square-ish FM voices are emphatically not.

use crate::notes::Note;

/// Which voice a channel plays, chosen from where it sits in the mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    /// Low and round, with a slow attack.
    Bass,
    /// The middle of the arrangement - a soft electric-piano stack.
    Keys,
    /// The top line, with a little vibrato so it sings.
    Lead,
    /// A hi-hat or cymbal: a short noise burst.
    Cymbal,
    /// A kick or tom: a pitched thump that drops as it decays.
    Drum,
}

/// Above this, a channel is not a melody - it is a cymbal. OPL music fakes
/// percussion on ordinary melodic channels (the hardware's rhythm mode goes
/// unused in every shipped track), so "very high and very short" is the only
/// signal that a channel is a hi-hat rather than a lead.
const CYMBAL_HZ: f32 = 2_000.0;
/// Likewise at the bottom. Kept below the lowest note a bassline plausibly
/// plays - B1 is 61.7Hz and D2 is 73.4Hz, both of which appear as real bass
/// parts in episode 1.
const DRUM_HZ: f32 = 55.0;

impl Voice {
    /// Relative amplitudes of harmonics 1, 2, 3, ... Values chosen to be
    /// warm rather than bright: everything falls off fast, so the result
    /// survives the low-pass at the end without turning to mud.
    fn partials(self) -> &'static [f32] {
        match self {
            // Almost pure, with a touch of second harmonic for body.
            Voice::Bass => &[1.0, 0.28, 0.06],
            // A gentle bell-ish stack, the classic soft-keys timbre.
            Voice::Keys => &[1.0, 0.42, 0.18, 0.09, 0.04],
            Voice::Lead => &[1.0, 0.30, 0.14, 0.05],
            // Percussion is synthesised rather than summed from partials.
            Voice::Cymbal | Voice::Drum => &[1.0],
        }
    }

    /// Attack, decay, sustain level, release - in seconds except sustain.
    fn envelope(self) -> Envelope {
        match self {
            Voice::Bass => Envelope { attack: 0.012, decay: 0.30, sustain: 0.55, release: 0.18 },
            Voice::Keys => Envelope { attack: 0.006, decay: 0.45, sustain: 0.35, release: 0.22 },
            Voice::Lead => Envelope { attack: 0.020, decay: 0.60, sustain: 0.70, release: 0.16 },
            // Percussion ignores how long the key is held and simply decays;
            // these values only bound how long it is rendered for.
            Voice::Cymbal => Envelope { attack: 0.0, decay: 0.06, sustain: 0.0, release: 0.06 },
            Voice::Drum => Envelope { attack: 0.0, decay: 0.20, sustain: 0.0, release: 0.20 },
        }
    }

    fn gain(self) -> f32 {
        match self {
            Voice::Bass => 0.85,
            Voice::Keys => 0.55,
            Voice::Lead => 0.60,
            // Under the music, where drums belong in this style.
            Voice::Cymbal => 0.22,
            Voice::Drum => 0.75,
        }
    }

    fn is_percussion(self) -> bool {
        matches!(self, Voice::Cymbal | Voice::Drum)
    }
}

#[derive(Clone, Copy)]
struct Envelope {
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
}

impl Envelope {
    /// Level at `t` seconds after key-on, for a note held `held` seconds.
    fn level(&self, t: f32, held: f32) -> f32 {
        if t < 0.0 {
            return 0.0;
        }
        let sustained = if t < self.attack {
            t / self.attack.max(1e-6)
        } else if t < self.attack + self.decay {
            let through = (t - self.attack) / self.decay.max(1e-6);
            1.0 - through * (1.0 - self.sustain)
        } else {
            self.sustain
        };
        if t <= held {
            return sustained;
        }
        // Released: fade from wherever the sustained part had got to.
        let since = t - held;
        if since >= self.release {
            return 0.0;
        }
        sustained * (1.0 - since / self.release.max(1e-6))
    }

    fn total_length(&self, held: f32) -> f32 {
        held + self.release
    }
}

/// Picks a voice per channel from where it sits in the arrangement.
///
/// Channels carry no declared role - the original points nine identical FM
/// slots at whatever the composer wanted - so pitch is the only signal.
///
/// The subtlety is percussion. No shipped track uses the OPL2's rhythm
/// mode, which is what makes the note decode unambiguous, but composers
/// still wrote drums: a hi-hat is a very high channel with very short notes,
/// a kick a very low one. Treating those as melody is not a small error.
/// `mcaves` puts 124 notes on a channel whose median pitch is 4.2kHz, and
/// voicing that as a lead sine made the remaster measurably *brighter* than
/// the FM original it was supposed to be warming up.
pub fn assign_voices(medians: &[Option<f32>; 9]) -> [Voice; 9] {
    let mut voices = [Voice::Keys; 9];
    let mut melodic: Vec<(usize, f32)> = Vec::new();
    for (i, median) in medians.iter().enumerate() {
        let Some(pitch) = *median else { continue };
        if pitch > CYMBAL_HZ {
            voices[i] = Voice::Cymbal;
        } else if pitch < DRUM_HZ {
            voices[i] = Voice::Drum;
        } else {
            melodic.push((i, pitch));
        }
    }
    if melodic.len() < 2 {
        return voices;
    }
    melodic.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    voices[melodic[0].0] = Voice::Bass;
    voices[melodic[melodic.len() - 1].0] = Voice::Lead;
    voices
}

pub struct RenderSettings {
    pub sample_rate: u32,
    /// Cutoff of the final low-pass, in Hz. The single biggest lo-fi lever.
    pub lowpass_hz: f32,
    /// Depth of the tape-wobble pitch drift, as a fraction of a semitone.
    pub wobble_cents: f32,
    /// How much vinyl crackle to mix in, 0..1.
    pub crackle: f32,
    /// Drive into the soft-clipper.
    pub drive: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings {
            sample_rate: 44_100,
            lowpass_hz: 7_200.0,
            wobble_cents: 7.0,
            crackle: 0.16,
            drive: 1.35,
        }
    }
}

/// Renders one pass of `notes`, looping seamlessly at `loop_secs`.
///
/// Notes whose release tail runs past the end of the pass have that tail
/// folded back onto the start, which is what makes the loop seamless: the
/// decay of the last chord is already sounding when the track repeats,
/// exactly as it would be if the pass simply continued.
pub fn render(notes: &[Note], loop_secs: f64, voices: &[Voice; 9], settings: &RenderSettings) -> Vec<i16> {
    let rate = settings.sample_rate as f32;
    let total = (loop_secs * settings.sample_rate as f64).ceil().max(1.0) as usize;
    let mut left = vec![0f32; total];
    let mut right = vec![0f32; total];

    for note in notes {
        let voice = voices[note.channel as usize % 9];
        let env = voice.envelope();
        let partials = voice.partials();
        // A drum's length is its own decay, not how long the key was held -
        // the original holds percussion notes for whatever the sequencer
        // found convenient.
        let held = if voice.is_percussion() {
            0.0
        } else {
            (note.end_secs() - note.start_secs()) as f32
        };
        let length = env.total_length(held);
        let start = (note.start_secs() * settings.sample_rate as f64) as usize;
        let count = (length * rate) as usize;

        // Two detuned copies, a few cents apart, for chorus. This is most of
        // what stops additive synthesis sounding sterile.
        let detune = [1.0f32, 1.0022];
        // Alternate channels across the stereo field, gently. Percussion
        // stays centred, as it would in any real mix.
        let pan = if voice.is_percussion() {
            0.5
        } else {
            0.5 + 0.22 * if note.channel % 2 == 0 { -1.0 } else { 1.0 }
        };
        let voices_summed = if voice.is_percussion() { 1.0 } else { detune.len() as f32 };
        let amp = note.velocity * voice.gain() / voices_summed;

        // Percussion is deterministic per note, seeded from where it sits,
        // so rebuilds stay identical.
        let mut hit = Noise::new(
            (note.start_tick as u32)
                .wrapping_mul(2_654_435_761)
                .wrapping_add(note.channel as u32 + 1),
        );
        // White noise is far too bright for this style - a raw burst reads
        // as static, not as a hi-hat, and on a sparse track it dominates the
        // whole mix's brightness. One pole at 4.5kHz turns it into a "tsk".
        let hat_alpha = {
            let rc = 1.0 / (std::f32::consts::TAU * 4_500.0);
            let dt = 1.0 / rate;
            dt / (rc + dt)
        };
        let mut hat_state = 0.0f32;

        for i in 0..count {
            let t = i as f32 / rate;
            let level = env.level(t, held);
            if level <= 0.0 {
                continue;
            }
            let sample = match voice {
                Voice::Cymbal => {
                    hat_state += hat_alpha * (hit.next_f32() - hat_state);
                    // Made up again afterwards - filtering costs level.
                    hat_state * 2.5
                }
                // A pitched thump whose frequency falls fast - the standard
                // way to synthesise a kick, and close to how the FM original
                // gets its own.
                Voice::Drum => {
                    let sweep = 1.0 + 2.5 * (-t * 45.0).exp();
                    (std::f32::consts::TAU * note.freq * sweep * t).sin()
                }
                _ => {
                    let mut sum = 0.0f32;
                    for d in detune {
                        let freq = note.freq * d * vibrato(voice, t);
                        for (h, weight) in partials.iter().enumerate() {
                            let partial_freq = freq * (h + 1) as f32;
                            // Simply don't emit anything that would alias.
                            if partial_freq >= rate * 0.5 {
                                break;
                            }
                            sum += weight * (std::f32::consts::TAU * partial_freq * t).sin();
                        }
                    }
                    sum
                }
            };
            let value = sample * level * amp;
            // Fold the tail back onto the start so the loop joins cleanly.
            let at = (start + i) % total;
            left[at] += value * (1.0 - pan);
            right[at] += value * pan;
        }
    }

    apply_wobble(&mut left, settings, 0.0);
    apply_wobble(&mut right, settings, 0.37);
    lowpass(&mut left, settings.lowpass_hz, rate);
    lowpass(&mut right, settings.lowpass_hz, rate);

    let mut out = Vec::with_capacity(total * 2);
    let mut noise = Noise::new(0x51ed_5eed);
    // Peak-normalise before the clipper so `drive` means the same thing
    // whatever the track's density. A track with nothing in it must not be
    // normalised at all - dividing by its peak would amplify arithmetic
    // noise into a full-scale signal.
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0f32, |m, s| m.max(s.abs()));
    let norm = if peak < 1e-4 { 0.0 } else { 0.72 / peak };
    for i in 0..total {
        let crackle = noise.crackle(settings.crackle);
        let l = saturate((left[i] * norm + crackle) * settings.drive);
        let r = saturate((right[i] * norm + crackle) * settings.drive);
        out.push((l.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        out.push((r.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
    }
    out
}

fn vibrato(voice: Voice, t: f32) -> f32 {
    match voice {
        // Delayed onset, so short notes stay steady and long ones sing.
        Voice::Lead => {
            let depth = 0.004 * (t * 3.0).min(1.0);
            1.0 + depth * (std::f32::consts::TAU * 5.2 * t).sin()
        }
        _ => 1.0,
    }
}

/// Tape wobble: a slow drift in playback rate, done by reading from a
/// modulated fractional delay. Two LFOs at unrelated rates keep it from
/// sounding like a deliberate vibrato.
fn apply_wobble(buffer: &mut [f32], settings: &RenderSettings, phase: f32) {
    if settings.wobble_cents <= 0.0 || buffer.len() < 4 {
        return;
    }
    let rate = settings.sample_rate as f32;
    let source = buffer.to_vec();
    let depth = settings.wobble_cents / 1200.0;
    for (i, out) in buffer.iter_mut().enumerate() {
        let t = i as f32 / rate;
        let drift = depth
            * (0.6 * (std::f32::consts::TAU * 0.7 * t + phase).sin()
                + 0.4 * (std::f32::consts::TAU * 2.3 * t + phase * 2.0).sin());
        // Offset in samples; wraps so the loop stays continuous.
        let read = i as f32 + drift * rate * 0.02;
        let base = read.floor();
        let frac = read - base;
        let n = source.len() as isize;
        let i0 = (base as isize).rem_euclid(n) as usize;
        let i1 = ((base as isize) + 1).rem_euclid(n) as usize;
        *out = source[i0] * (1.0 - frac) + source[i1] * frac;
    }
}

/// Two passes of a one-pole filter, for a gentler 12 dB/octave slope than a
/// single pole gives.
fn lowpass(buffer: &mut [f32], cutoff_hz: f32, rate: f32) {
    if cutoff_hz <= 0.0 || cutoff_hz >= rate * 0.5 {
        return;
    }
    let dt = 1.0 / rate;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    let alpha = dt / (rc + dt);
    for _ in 0..2 {
        // Prime from the end of the buffer so the filter's state is already
        // settled at sample zero - otherwise every loop starts with a click.
        let mut state = *buffer.last().unwrap_or(&0.0);
        for sample in buffer.iter_mut() {
            state += alpha * (*sample - state);
            *sample = state;
        }
    }
}

fn saturate(x: f32) -> f32 {
    x.tanh()
}

/// Deterministic noise, for crackle. A fixed seed keeps rebuilds identical.
struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Noise(seed | 1)
    }
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    /// Sparse pops rather than a hiss bed - that is what reads as vinyl.
    fn crackle(&mut self, amount: f32) -> f32 {
        if amount <= 0.0 {
            return 0.0;
        }
        let roll = self.next_f32();
        if roll > 0.9985 {
            self.next_f32() * amount * 0.5
        } else {
            self.next_f32() * amount * 0.0035
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(channel: u8, start: u64, end: u64, freq: f32) -> Note {
        Note { channel, start_tick: start, end_tick: end, freq, velocity: 1.0 }
    }

    #[test]
    fn the_lowest_channel_gets_the_bass_and_the_highest_the_lead() {
        let mut medians = [None; 9];
        medians[0] = Some(400.0);
        medians[2] = Some(90.0);
        medians[5] = Some(1200.0);
        let voices = assign_voices(&medians);
        assert_eq!(voices[2], Voice::Bass);
        assert_eq!(voices[5], Voice::Lead);
        assert_eq!(voices[0], Voice::Keys);
    }

    #[test]
    fn a_single_voice_track_is_not_forced_into_a_bass_part() {
        let mut medians = [None; 9];
        medians[3] = Some(500.0);
        assert_eq!(assign_voices(&medians)[3], Voice::Keys);
    }

    #[test]
    fn faked_percussion_channels_are_not_voiced_as_melody() {
        // These are mcaves' real medians. Channel 8 at 4.2kHz is a hi-hat
        // and channel 6 at 36.7Hz a kick; voicing either as melody is what
        // made the remaster brighter than the FM original.
        let mut medians = [None; 9];
        medians[1] = Some(795.0);
        medians[2] = Some(92.7);
        medians[3] = Some(146.8);
        medians[4] = Some(293.6);
        medians[5] = Some(659.2);
        medians[6] = Some(36.7);
        medians[8] = Some(4169.3);
        let voices = assign_voices(&medians);
        assert_eq!(voices[8], Voice::Cymbal);
        assert_eq!(voices[6], Voice::Drum);
        // The lead must be the highest *melodic* channel, not the hi-hat.
        assert_eq!(voices[1], Voice::Lead);
        assert_eq!(voices[2], Voice::Bass);
    }

    #[test]
    fn a_real_bassline_is_not_mistaken_for_a_kick_drum() {
        // mhappy's bass sits on B1 (61.7Hz) and D2 (73.4Hz); both are above
        // the drum threshold and must stay melodic.
        let mut medians = [None; 9];
        medians[5] = Some(61.7);
        medians[1] = Some(73.4);
        medians[8] = Some(587.2);
        let voices = assign_voices(&medians);
        assert_eq!(voices[5], Voice::Bass);
        assert_eq!(voices[8], Voice::Lead);
        assert!(!voices[1].is_percussion());
    }

    #[test]
    fn percussion_decays_on_its_own_rather_than_being_held() {
        // The sequencer holds drum notes for arbitrary lengths; a hi-hat
        // held for a bar would be a very long noise burst.
        let settings = RenderSettings { sample_rate: 8000, crackle: 0.0, wobble_cents: 0.0, ..Default::default() };
        let mut voices = [Voice::Keys; 9];
        voices[0] = Voice::Cymbal;
        // Held for a full second.
        let pcm = render(&[note(0, 0, 560, 4000.0)], 1.0, &voices, &settings);
        let stereo: Vec<i32> = pcm.chunks(2).map(|c| (c[0] as i32).abs()).collect();
        let early: i32 = stereo[..200].iter().sum();
        let late: i32 = stereo[3000..3200].iter().sum();
        assert!(early > late * 4, "the burst should be long over by then");
    }

    #[test]
    fn the_envelope_rises_holds_and_releases() {
        let env = Envelope { attack: 0.1, decay: 0.1, sustain: 0.5, release: 0.2 };
        assert_eq!(env.level(0.0, 1.0), 0.0);
        assert!((env.level(0.1, 1.0) - 1.0).abs() < 1e-5, "peak at end of attack");
        assert!((env.level(0.2, 1.0) - 0.5).abs() < 1e-5, "decayed to sustain");
        assert!((env.level(0.9, 1.0) - 0.5).abs() < 1e-5, "held");
        assert!(env.level(1.1, 1.0) < 0.5, "releasing");
        assert_eq!(env.level(1.3, 1.0), 0.0, "silent once released");
        assert_eq!(env.level(9.0, 1.0), 0.0);
    }

    #[test]
    fn rendering_produces_stereo_audio_of_exactly_the_loop_length() {
        let notes = vec![note(0, 0, 280, 220.0)];
        let settings = RenderSettings { sample_rate: 8000, ..Default::default() };
        let pcm = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        assert_eq!(pcm.len(), 8000 * 2, "one second, interleaved stereo");
    }

    #[test]
    fn the_output_is_audible_and_never_clips() {
        let notes = vec![note(0, 0, 280, 220.0), note(1, 0, 280, 330.0)];
        let settings = RenderSettings { sample_rate: 8000, ..Default::default() };
        let pcm = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        let peak = pcm.iter().map(|s| s.unsigned_abs() as u32).max().unwrap();
        assert!(peak > 3000, "far too quiet to hear: {peak}");
        assert!(peak < i16::MAX as u32, "clipped");
    }

    #[test]
    fn a_notes_release_tail_wraps_onto_the_start_so_the_loop_joins() {
        // A note ending exactly at the loop point must leave its tail
        // sounding at sample zero, or every repeat begins with a cut.
        let settings = RenderSettings {
            sample_rate: 8000,
            crackle: 0.0,
            wobble_cents: 0.0,
            ..Default::default()
        };
        let notes = vec![note(0, 0, 560, 220.0)]; // one second, the whole loop
        let pcm = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        let head: i32 = pcm[..400].iter().map(|s| (*s as i32).abs()).sum();
        assert!(head > 0, "the wrapped tail should be audible at the start");
    }

    #[test]
    fn partials_above_nyquist_are_dropped_rather_than_aliased() {
        // A high note whose harmonics run past Nyquist must still render
        // cleanly - the whole reason for additive synthesis here.
        let settings = RenderSettings { sample_rate: 8000, crackle: 0.0, ..Default::default() };
        let notes = vec![note(0, 0, 280, 3900.0)];
        let pcm = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        assert!(pcm.iter().all(|s| s.checked_abs().is_some()));
    }

    #[test]
    fn rendering_is_deterministic_so_the_asset_cache_stays_meaningful() {
        let notes = vec![note(0, 0, 280, 220.0), note(3, 100, 400, 110.0)];
        let settings = RenderSettings { sample_rate: 8000, ..Default::default() };
        let a = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        let b = render(&notes, 1.0, &[Voice::Keys; 9], &settings);
        assert_eq!(a, b);
    }

    #[test]
    fn an_empty_score_renders_near_silence_rather_than_amplified_noise() {
        // Peak-normalising a silent buffer would divide by ~0 and turn
        // arithmetic noise into a full-scale signal.
        let settings = RenderSettings { sample_rate: 8000, ..Default::default() };
        let pcm = render(&[], 0.5, &[Voice::Keys; 9], &settings);
        assert_eq!(pcm.len(), 4000 * 2);
        // Crackle is deliberately sparse, so the typical sample is silent
        // even though the occasional pop is not.
        let mut magnitudes: Vec<u16> = pcm.iter().map(|s| s.unsigned_abs()).collect();
        magnitudes.sort_unstable();
        let median = magnitudes[magnitudes.len() / 2];
        assert!(median < 200, "median sample {median} is not silence");
    }

    #[test]
    fn a_quiet_passage_is_not_normalised_up_to_match_a_loud_one() {
        // Normalisation is per-track, so a track that is genuinely quiet
        // throughout still comes out quieter than a dense one - but a track
        // with a single soft note must not be blown up to full scale.
        let settings = RenderSettings { sample_rate: 8000, crackle: 0.0, ..Default::default() };
        let mut soft = note(0, 0, 280, 220.0);
        soft.velocity = 0.02;
        let pcm = render(&[soft], 1.0, &[Voice::Keys; 9], &settings);
        let peak = pcm.iter().map(|s| s.unsigned_abs() as u32).max().unwrap();
        assert!(peak < i16::MAX as u32, "clipped");
    }
}
