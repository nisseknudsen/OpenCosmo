//! Converts the owner's original GOG installer into Bevy-ready assets
//! before compiling, caching the result so repeat builds are instant.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

/// Locates the installer to convert, or `None` if there isn't one.
///
/// A missing installer is not an error. The game needs converted assets to
/// *run*, but the code compiles and the unit tests pass without them - the
/// behaviour tests are pure functions over game state, not fixtures read
/// from disk. Failing the build here would mean nobody could compile or
/// test this project, or run CI on it, without owning a copy of a 1992
/// game. The failure is deferred to startup, where it can say so plainly.
fn find_installer(workspace_root: &std::path::Path) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("COSMO_INSTALLER") {
        return Some(PathBuf::from(p));
    }
    std::fs::read_dir(workspace_root.join("original"))
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map(|x| x == "sh").unwrap_or(false))
        .map(|e| e.path())
}

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .context("expected crates/opencosmo-game to be two levels under the workspace root")?;

    println!("cargo:rerun-if-env-changed=COSMO_INSTALLER");
    let Some(sh_path) = find_installer(workspace_root) else {
        println!(
            "cargo:warning=no game installer found in ./original (or COSMO_INSTALLER); \
             building without assets - the code compiles and the tests run, but the \
             game needs your own copy of Cosmo's Cosmic Adventure to play"
        );
        return Ok(());
    };
    println!("cargo:rerun-if-changed={}", sh_path.display());

    let out_dir = manifest_dir.join("assets/generated");
    let converted = opencosmo_assets::convert::convert_all_episodes_if_stale(&sh_path, &out_dir)?;
    if converted {
        println!(
            "cargo:warning=opencosmo-assets: converted all {} episodes into {}",
            opencosmo_assets::convert::EPISODES.len(),
            out_dir.display()
        );
    }

    Ok(())
}
