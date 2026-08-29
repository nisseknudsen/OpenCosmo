//! Unwraps a GOG "makeself + mojosetup" `.sh` installer to reach the embedded
//! zip payload. The header is a shell script whose line count (N) is baked in
//! by the makeself packager as `head -n N "$0" | wc -c`; everything after
//! that byte offset is a mojosetup binary blob followed by a zip archive.
//! The `zip` crate locates entries via the end-of-central-directory record,
//! so it doesn't care that the mojosetup binary precedes the real zip data.

use anyhow::{bail, Context, Result};
use std::io::Cursor;
use std::path::Path;
use zip::ZipArchive;

fn header_line_count(bytes: &[u8]) -> Result<usize> {
    const NEEDLE: &[u8] = b"head -n ";
    let pos = bytes
        .windows(NEEDLE.len())
        .position(|w| w == NEEDLE)
        .context("could not find makeself 'head -n <N>' offset marker")?;
    let digits_start = pos + NEEDLE.len();
    let digits_end = bytes[digits_start..]
        .iter()
        .position(|b| !b.is_ascii_digit())
        .map(|i| digits_start + i)
        .context("malformed offset marker")?;
    let n: usize = std::str::from_utf8(&bytes[digits_start..digits_end])?.parse()?;
    Ok(n)
}

fn header_byte_offset(bytes: &[u8], line_count: usize) -> Result<usize> {
    let mut seen = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            seen += 1;
            if seen == line_count {
                return Ok(i + 1);
            }
        }
    }
    bail!("installer script shorter than declared header line count");
}

/// Opens the embedded zip payload of a GOG makeself installer.
pub fn open_installer_zip(sh_path: &Path) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    let bytes = std::fs::read(sh_path)
        .with_context(|| format!("reading installer {}", sh_path.display()))?;
    let n = header_line_count(&bytes)?;
    let offset = header_byte_offset(&bytes, n)?;
    let payload = bytes[offset..].to_vec();
    let archive = ZipArchive::new(Cursor::new(payload)).context("payload is not a valid zip")?;
    Ok(archive)
}

/// Reads a single named file out of the installer's zip payload.
pub fn read_zip_entry(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    name_suffix: &str,
) -> Result<Vec<u8>> {
    let idx = (0..archive.len())
        .find(|&i| {
            archive
                .by_index(i)
                .map(|f| f.name().ends_with(name_suffix))
                .unwrap_or(false)
        })
        .with_context(|| format!("no zip entry ending in {name_suffix}"))?;
    let mut file = archive.by_index(idx)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::Read::read_to_end(&mut file, &mut buf)?;
    Ok(buf)
}
