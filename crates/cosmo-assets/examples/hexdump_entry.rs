use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let sh_path = PathBuf::from(std::env::args().nth(1).unwrap());
    let container = std::env::args().nth(2).unwrap();
    let entry_name = std::env::args().nth(3).unwrap();
    let n: usize = std::env::args().nth(4).unwrap_or_else(|| "64".into()).parse()?;
    let mut zip = cosmo_assets::shell::open_installer_zip(&sh_path)?;
    let container_bytes = cosmo_assets::shell::read_zip_entry(&mut zip, &container)?;
    let entries = cosmo_assets::vol::parse(&container_bytes)?;
    let e = entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(&entry_name))
        .unwrap();
    println!("{} : {} bytes", e.name, e.data.len());
    for (i, chunk) in e.data[..n.min(e.data.len())].chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for b in chunk {
            print!("{:02x} ", b);
        }
        println!();
    }
    Ok(())
}
