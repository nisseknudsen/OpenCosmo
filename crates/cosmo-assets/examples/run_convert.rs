use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let sh_path = PathBuf::from(std::env::args().nth(1).expect("path to installer .sh"));
    let out_dir = PathBuf::from(std::env::args().nth(2).expect("out dir"));
    let start = std::time::Instant::now();
    let did_convert = cosmo_assets::convert::convert_all_episodes_if_stale(&sh_path, &out_dir)?;
    println!("converted={did_convert} in {:?}", start.elapsed());
    let start = std::time::Instant::now();
    let did_convert2 = cosmo_assets::convert::convert_all_episodes_if_stale(&sh_path, &out_dir)?;
    println!("second run converted={did_convert2} in {:?} (should be false/fast)", start.elapsed());
    Ok(())
}
