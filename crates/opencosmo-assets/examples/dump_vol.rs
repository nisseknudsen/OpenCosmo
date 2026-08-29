use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let sh_path = PathBuf::from(std::env::args().nth(1).expect("path to installer .sh"));
    let entry_name = std::env::args().nth(2).unwrap_or_else(|| "COSMO1.VOL".to_string());
    let mut zip = opencosmo_assets::shell::open_installer_zip(&sh_path)?;
    let vol_bytes = opencosmo_assets::shell::read_zip_entry(&mut zip, &entry_name)?;
    println!("{entry_name}: {} bytes", vol_bytes.len());
    let entries = opencosmo_assets::vol::parse(&vol_bytes)?;
    println!("{} entries:", entries.len());
    for e in &entries {
        println!("  {:16} {:>8} bytes", e.name, e.data.len());
    }
    Ok(())
}
