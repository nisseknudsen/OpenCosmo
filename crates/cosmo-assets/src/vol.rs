//! Parser for Apogee's ".VOL" archive format (also used by Duke Nukem II,
//! Major Stryker, etc.): a fixed directory of 200 x 20-byte entries
//! (12-byte null-padded name + u32 LE offset + u32 LE length), zero-filled
//! for unused slots, followed immediately by the concatenated file bodies.

use anyhow::{Context, Result};

const ENTRY_COUNT: usize = 200;
const ENTRY_SIZE: usize = 20;
const NAME_LEN: usize = 12;

pub struct VolEntry<'a> {
    pub name: String,
    pub data: &'a [u8],
}

pub fn parse(bytes: &[u8]) -> Result<Vec<VolEntry<'_>>> {
    let header_len = ENTRY_COUNT * ENTRY_SIZE;
    anyhow::ensure!(
        bytes.len() >= header_len,
        "file too small to hold VOL directory"
    );

    let mut entries = Vec::new();
    for i in 0..ENTRY_COUNT {
        let rec = &bytes[i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE];
        let raw_name = &rec[0..NAME_LEN];
        if raw_name.iter().all(|&b| b == 0) {
            continue;
        }
        let name_end = raw_name.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        let name = std::str::from_utf8(&raw_name[..name_end])
            .context("VOL entry name is not valid UTF-8")?
            .to_string();
        let offset = u32::from_le_bytes(rec[12..16].try_into().unwrap()) as usize;
        let length = u32::from_le_bytes(rec[16..20].try_into().unwrap()) as usize;
        if length == 0 {
            continue;
        }
        anyhow::ensure!(
            offset + length <= bytes.len(),
            "VOL entry {name} points past end of file"
        );
        entries.push(VolEntry {
            name,
            data: &bytes[offset..offset + length],
        });
    }
    Ok(entries)
}
