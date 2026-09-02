//! Build-time cache: converting the original installer into PNG/JSON/WAV
//! assets is slow-ish and the source data never changes on a dev machine, so
//! we stamp the output directory with a hash of (source bytes, converter
//! version) and skip reconversion when nothing relevant has changed.

use anyhow::Result;
use std::path::Path;

/// Bump this whenever decode/convert *logic* changes. The forced sprite
/// lists are hashed in separately, so adding one of those needs no bump.
pub const CONVERTER_VERSION: u32 = 18;

const STAMP_FILE: &str = ".cache-stamp";

pub fn fingerprint(source_bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(source_bytes);
    hasher.update(&CONVERTER_VERSION.to_le_bytes());
    // The lists of sprites forced into the conversion are part of what the
    // output *is*, so they belong in the fingerprint. Leaving them out
    // meant adding a sprite silently did nothing until someone remembered
    // to bump the version by hand - which twice, nobody did.
    for list in [
        crate::convert::EFFECT_SPRITES,
        crate::convert::RUNTIME_SPAWNED_SPRITES,
    ] {
        for id in list {
            hasher.update(&id.to_le_bytes());
        }
    }
    hasher.finalize().to_hex().to_string()
}

/// Returns true if `out_dir` already holds a cache matching `fingerprint`.
pub fn is_fresh(out_dir: &Path, fingerprint: &str) -> bool {
    let stamp_path = out_dir.join(STAMP_FILE);
    match std::fs::read_to_string(&stamp_path) {
        Ok(existing) => existing.trim() == fingerprint,
        Err(_) => false,
    }
}

pub fn write_stamp(out_dir: &Path, fingerprint: &str) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join(STAMP_FILE), fingerprint)?;
    Ok(())
}
