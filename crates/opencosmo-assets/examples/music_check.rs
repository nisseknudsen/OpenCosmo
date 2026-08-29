//! Manual validation for the IMF -> OPL2 -> WAV pipeline: decodes one real
//! track, renders it, and reports structural sanity checks (we can't listen,
//! so silence/duration/pitch-variation checks are the primary signal).

use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let sh_path = PathBuf::from(std::env::args().nth(1).expect("path to installer .sh"));
    let track = std::env::args().nth(2).unwrap_or_else(|| "MCAVES.MNI".to_string());
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(3)
            .unwrap_or_else(|| "/tmp/cosmo-music-check".to_string()),
    );
    std::fs::create_dir_all(&out_dir)?;

    let mut zip = opencosmo_assets::shell::open_installer_zip(&sh_path)?;
    let vol_bytes = opencosmo_assets::shell::read_zip_entry(&mut zip, "COSMO1.VOL")?;
    let entries = opencosmo_assets::vol::parse(&vol_bytes)?;
    let data = entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(&track))
        .unwrap_or_else(|| panic!("no entry {track}"))
        .data;

    println!("{track}: {} raw bytes", data.len());
    let events = opencosmo_assets::music::parse_imf(data)?;
    println!("{} IMF events", events.len());

    let ticks = opencosmo_assets::music::duration_ticks(&events);
    let secs = ticks as f64 / opencosmo_assets::music::TICK_HZ;
    println!(
        "duration: {ticks} ticks @ {}Hz = {:.2}s",
        opencosmo_assets::music::TICK_HZ,
        secs
    );

    let sample_rate = 44100u32;
    let start = std::time::Instant::now();
    let pcm = opencosmo_assets::music::render_to_pcm(&events, sample_rate);
    println!("rendered {} stereo samples in {:?}", pcm.len() / 2, start.elapsed());

    let wav_secs = (pcm.len() / 2) as f64 / sample_rate as f64;
    println!("rendered WAV duration: {wav_secs:.2}s (vs {secs:.2}s predicted from ticks)");

    let out_path = out_dir.join(format!("{}.wav", track.trim_end_matches(".MNI").to_ascii_lowercase()));
    opencosmo_assets::music::render_to_wav(&events, sample_rate, &out_path)?;
    println!("wrote {}", out_path.display());

    // --- Structural validation ---
    let left: Vec<i32> = pcm.iter().step_by(2).map(|&s| s as i32).collect();

    // 1. Non-silence: check RMS in 0.5s windows across the whole track.
    let window = (sample_rate as f64 * 0.5) as usize;
    let mut silent_windows = 0;
    let mut total_windows = 0;
    for chunk in left.chunks(window) {
        total_windows += 1;
        let rms = (chunk.iter().map(|&s| (s * s) as f64).sum::<f64>() / chunk.len() as f64).sqrt();
        if rms < 50.0 {
            silent_windows += 1;
        }
    }
    println!(
        "silence check: {silent_windows}/{total_windows} windows near-silent (RMS<50)"
    );

    // 2. Pitch variation: zero-crossing rate per 0.25s window, report the
    // spread across windows - a constant drone would show near-zero spread.
    let zc_window = (sample_rate as f64 * 0.25) as usize;
    let mut zc_rates = Vec::new();
    for chunk in left.chunks(zc_window) {
        if chunk.len() < 100 {
            continue;
        }
        let crossings = chunk.windows(2).filter(|w| (w[0] >= 0) != (w[1] >= 0)).count();
        zc_rates.push(crossings as f64 / (chunk.len() as f64 / sample_rate as f64));
    }
    let min_zc = zc_rates.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_zc = zc_rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean_zc = zc_rates.iter().sum::<f64>() / zc_rates.len() as f64;
    println!(
        "zero-crossing rate across {} windows: min={min_zc:.0}Hz max={max_zc:.0}Hz mean={mean_zc:.0}Hz (spread indicates pitch movement)",
        zc_rates.len()
    );

    Ok(())
}
