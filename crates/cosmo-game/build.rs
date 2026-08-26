//! Converts the owner's original GOG installer into Bevy-ready assets
//! before compiling, caching the result so repeat builds are instant.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

fn find_installer(workspace_root: &std::path::Path) -> Result<PathBuf> {
    if let Ok(p) = std::env::var("COSMO_INSTALLER") {
        return Ok(PathBuf::from(p));
    }
    let original_dir = workspace_root.join("original");
    let candidate = std::fs::read_dir(&original_dir)
        .with_context(|| format!("reading {}", original_dir.display()))?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map(|x| x == "sh").unwrap_or(false));
    match candidate {
        Some(entry) => Ok(entry.path()),
        None => bail!(
            "no GOG installer .sh found in {} (set COSMO_INSTALLER=/path/to/installer.sh, \
             or place the owner's original GOG installer there)",
            original_dir.display()
        ),
    }
}

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .context("expected crates/cosmo-game to be two levels under the workspace root")?;

    let sh_path = find_installer(workspace_root)?;
    println!("cargo:rerun-if-changed={}", sh_path.display());
    println!("cargo:rerun-if-env-changed=COSMO_INSTALLER");

    let out_dir = manifest_dir.join("assets/generated");
    let converted = cosmo_assets::convert::convert_all_episodes_if_stale(&sh_path, &out_dir)?;
    if converted {
        println!(
            "cargo:warning=cosmo-assets: converted all {} episodes into {}",
            cosmo_assets::convert::EPISODES.len(),
            out_dir.display()
        );
    }

    Ok(())
}
