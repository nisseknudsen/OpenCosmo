pub mod actor_sprite_map;
pub mod cache;
pub mod convert;
pub mod level;
pub mod music;
pub mod palette;
pub mod shell;
pub mod sprite;
pub mod tile;
pub mod vol;

pub mod episode {
    /// The three episode short-names as they appear in file names
    /// (COSMO1.VOL, COSMO2.VOL, COSMO3.VOL, ...).
    pub const ALL: [&str; 3] = ["COSMO1", "COSMO2", "COSMO3"];
}
