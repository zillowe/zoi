//! `.zdelta` - Zoi's delta patch format.
//!
//! A `.zdelta` is a zstd-compressed bsdiff delta patch. Two shapes are
//! supported so the format can stay useful outside of packages:
//!
//! - Single-target shape: the decompressed payload is a raw bsdiff patch (magic
//!   `BSDIFF40`) transforming exactly one source into one target.
//! - Container shape: the decompressed payload is a tar archive holding a
//!   `meta.json` descriptor plus one or more patch/payload files, used when
//!   several files change together (e.g. pooled package archives).
//!
//! Both shapes are zstd frames (zstd magic `28 B5 2F FD`), mirroring how
//! Zoi's self-update deltas are distributed.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::Value;

/// Magic bytes at the start of a raw bsdiff 4.x patch.
const BSDIFF_MAGIC: &[u8; 8] = b"BSDIFF40";

/// The format identifier stored in a `.zdelta` container's `meta.json`.
pub const ZDELTA_FORMAT_ID: &str = "zoi.zdelta.v1";

/// Generates a raw bsdiff patch transforming `old` into `new`.
///
/// # Errors
///
/// Returns an error if the underlying compressor fails.
pub fn diff_bytes(old: &[u8], new: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut patch = Vec::new();
    zbsdiff::Bsdiff::new(old, new).compare(Cursor::new(&mut patch))?;
    Ok(patch)
}

/// Applies a raw bsdiff patch to `old`, producing the target bytes.
///
/// # Errors
///
/// Returns an error if the patch is malformed or does not fit `old`.
pub fn apply_bytes(old: &[u8], patch: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut target = Vec::with_capacity(
        zbsdiff::Bspatch::new(patch)?.hint_target_size() as usize
    );
    zbsdiff::Bspatch::new(patch)?.apply(old, Cursor::new(&mut target))?;
    Ok(target)
}

/// Returns true when the buffer looks like a tar archive (ustar magic).
#[must_use]
pub fn looks_like_tar(bytes: &[u8]) -> bool {
    bytes.len() > 262 && bytes.get(257..262) == Some(b"ustar")
}

/// Decompresses a zstd frame.
///
/// # Errors
///
/// Returns an error if the data is not a valid zstd stream.
pub fn zstd_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let decoder = zstd::stream::read::Decoder::new(Cursor::new(data))?;
    let mut out = Vec::new();
    decoder
        .take(64 * 1024 * 1024 * 1024)
        .read_to_end(&mut out)?;
    Ok(out)
}

/// Compresses bytes into a zstd frame.
///
/// # Errors
///
/// Returns an error if compression fails.
pub fn zstd_compress(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut out = Vec::new();
    {
        let mut encoder = zstd::stream::write::Encoder::new(&mut out, 0)?;
        encoder.write_all(data)?;
        encoder.finish()?;
    }
    Ok(out)
}

/// Writes a `.zdelta` container: a zstd-compressed tar holding `meta.json`
/// plus arbitrary named payload files.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write_container(
    meta: &Value,
    files: &[(String, Vec<u8>)],
    output_path: &Path
) -> Result<()> {
    let mut builder = tar::Builder::new(Vec::new());

    let meta_bytes = serde_json::to_string_pretty(meta)?;
    append_file(&mut builder, "meta.json", meta_bytes.as_bytes())?;
    for (name, data) in files {
        append_file(&mut builder, name, data)?;
    }

    let tar_bytes = builder.into_inner()?;
    fs::write(output_path, zstd_compress(&tar_bytes)?)?;
    Ok(())
}

/// Appends a single file entry to a tar builder.
///
/// Creates a GNU-tar header with the given name and content, then appends
/// it to the builder.
///
/// # Errors
///
/// Returns an error if the header cannot be appended.
fn append_file(
    builder: &mut tar::Builder<Vec<u8>>,
    name: &str,
    data: &[u8]
) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, Cursor::new(data))?;
    Ok(())
}

/// Reads a `.zdelta` container from decompressed tar bytes, returning its
/// `meta.json` value and the named payload files.
///
/// # Errors
///
/// Returns an error if the data is not a valid container.
pub fn read_container_bytes(
    raw: &[u8]
) -> Result<(Value, BTreeMap<String, Vec<u8>>)> {
    if !looks_like_tar(raw) {
        return Err(anyhow!("data is not a .zdelta container"));
    }
    let mut archive = tar::Archive::new(Cursor::new(raw));
    let mut meta: Option<Value> = None;
    let mut files = BTreeMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().to_string();
        let name = name.trim_start_matches("./").to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        if name == "meta.json" {
            meta = Some(serde_json::from_slice(&data)?);
        } else if !name.is_empty() {
            files.insert(name, data);
        }
    }

    let meta =
        meta.ok_or_else(|| anyhow!("container has no meta.json descriptor"))?;
    Ok((meta, files))
}

/// Reads a `.zdelta` container from a file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or is not a valid container.
pub fn read_container(
    path: &Path
) -> Result<(Value, BTreeMap<String, Vec<u8>>)> {
    let raw = zstd_decompress(&fs::read(path)?)?;
    read_container_bytes(&raw)
}

/// Reads any `.zdelta` file. Returns `(Some(meta), Some(files), patches)`
/// for containers, or the raw decompressed bsdiff patch bytes for the
/// single-target shape.
///
/// # Errors
///
/// Returns an error if the file cannot be read or decoded.
pub enum ZDelta {
    /// A multi-file container with a `meta.json` descriptor.
    Container(Value, BTreeMap<String, Vec<u8>>),
    /// A single raw bsdiff patch.
    Single(Vec<u8>)
}

/// Loads a `.zdelta` file and detects which shape it has.
///
/// # Errors
///
/// Returns an error if reading or decompression fails.
pub fn load(path: &Path) -> Result<ZDelta> {
    let decompressed = zstd_decompress(&fs::read(path)?)?;
    if looks_like_tar(&decompressed) {
        let (meta, files) = read_container_bytes(&decompressed)?;
        return Ok(ZDelta::Container(meta, files));
    }
    if decompressed.starts_with(BSDIFF_MAGIC) {
        return Ok(ZDelta::Single(decompressed));
    }
    Err(anyhow!(
        "{} is neither a .zdelta container nor a bsdiff patch",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_patch() {
        let old = b"hello world, this is the old content";
        let new = b"hello world, this is the NEW content!";
        let patch = diff_bytes(old, new).expect("diff should succeed");
        let rebuilt = apply_bytes(old, &patch).expect("apply should succeed");
        assert_eq!(rebuilt, new);
    }

    #[test]
    fn container_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.zdelta");
        let meta = serde_json::json!({
            "format": ZDELTA_FORMAT_ID,
            "entries": ["a.bsdiff"]
        });
        write_container(&meta, &[("a.bsdiff".into(), vec![1, 2, 3])], &path)
            .expect("write should succeed");

        let loaded = load(&path).expect("load should succeed");
        match loaded {
            ZDelta::Container(m, files) => {
                assert_eq!(
                    m.get("format").expect("format key"),
                    ZDELTA_FORMAT_ID
                );
                assert_eq!(
                    files.get("a.bsdiff").expect("a.bsdiff key"),
                    &vec![1, 2, 3]
                );
            }
            ZDelta::Single(_) => panic!("expected container shape")
        }
    }

    #[test]
    fn single_shape_detection() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("single.zdelta");
        let delta = diff_bytes(b"aaa", b"aab").expect("diff should succeed");
        fs::write(&path, zstd_compress(&delta).expect("compress")).ok();
        let loaded = load(&path).expect("load should succeed");
        assert!(matches!(loaded, ZDelta::Single(_)));
    }
}
