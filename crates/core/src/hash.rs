use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256, Sha512};
use walkdir::WalkDir;

/// Supported hashing algorithms for integrity verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// SHA-512 (128 character hex string).
    Sha512,
    /// SHA-256 (64 character hex string).
    Sha256
}

impl HashAlgorithm {
    /// Returns the hash algorithm based on the length of the hex string.
    pub fn from_len(len: usize) -> Option<Self> {
        match len {
            128 => Some(HashAlgorithm::Sha512),
            64 => Some(HashAlgorithm::Sha256),
            _ => None
        }
    }

    /// Returns the hash algorithm based on its name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "sha512" => Some(HashAlgorithm::Sha512),
            "sha256" => Some(HashAlgorithm::Sha256),
            _ => None
        }
    }

    /// The algorithm's name as written in a digest (`sha512`, `sha256`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            HashAlgorithm::Sha512 => "sha512",
            HashAlgorithm::Sha256 => "sha256"
        }
    }

    /// Length of the algorithm's hex digest.
    #[must_use]
    pub fn hex_len(self) -> usize {
        match self {
            HashAlgorithm::Sha512 => 128,
            HashAlgorithm::Sha256 => 64
        }
    }
}

/// The algorithm for every hash Zoi records itself: pool objects, file
/// digests, archive manifests, sidecars, trust records and caches. SHA-256
/// stays accepted wherever a hash is read, for archives built before and for
/// hashes users write themselves.
pub const DEFAULT_ALGORITHM: HashAlgorithm = HashAlgorithm::Sha512;

/// Splits a digest written as `<algo>-<hex>` (`sha512-...`, `sha256-...`).
///
/// Returns `None` for an unknown algorithm or a hex part of the wrong length
/// or with non-hex characters.
#[must_use]
pub fn parse_digest(digest: &str) -> Option<(HashAlgorithm, &str)> {
    let (name, hex) = digest.split_once('-')?;
    let algo = HashAlgorithm::from_name(name)?;
    (hex.len() == algo.hex_len() && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then_some((algo, hex))
}

/// Writes a digest as `<algo>-<hex>`.
#[must_use]
pub fn format_digest(algo: HashAlgorithm, hex: &str) -> String {
    format!("{}-{hex}", algo.name())
}

/// The `sha512-<hex>` digest of a file, as Zoi records it.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn file_digest(path: &Path) -> Result<String> {
    Ok(format_digest(
        DEFAULT_ALGORITHM,
        &calculate_file_hash(path, DEFAULT_ALGORITHM)?
    ))
}

/// The `sha512-<hex>` digest of a byte buffer, as Zoi records it.
#[must_use]
pub fn bytes_digest(bytes: &[u8]) -> String {
    format_digest(DEFAULT_ALGORITHM, &hex::encode(Sha512::digest(bytes)))
}

/// Whether `bytes` match a bare hex digest, whose algorithm is told by its
/// length (128 characters for SHA-512, 64 for SHA-256).
///
/// For records that store bare digests (plugin and hook trust, the audit
/// log): new ones are SHA-512, and SHA-256 ones written by earlier versions
/// still match.
#[must_use]
pub fn bytes_match_hex(bytes: &[u8], hex: &str) -> bool {
    HashAlgorithm::from_len(hex.len()).is_some_and(|algo| {
        bytes_match_digest(bytes, &format_digest(algo, hex))
    })
}

/// Whether `bytes` match `digest` (`<algo>-<hex>`, either algorithm).
#[must_use]
pub fn bytes_match_digest(bytes: &[u8], digest: &str) -> bool {
    let Some((algo, hex)) = parse_digest(digest) else {
        return false;
    };
    let actual = match algo {
        HashAlgorithm::Sha512 => hex::encode(Sha512::digest(bytes)),
        HashAlgorithm::Sha256 => hex::encode(Sha256::digest(bytes))
    };
    actual.eq_ignore_ascii_case(hex)
}

/// Calculates the cryptographic hash of a single file.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub fn calculate_file_hash(path: &Path, algo: HashAlgorithm) -> Result<String> {
    let mut file = fs::File::open(path)
        .map_err(|e| anyhow!("Failed to open file {}: {e}", path.display()))?;

    match algo {
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            let mut buffer = [0; 8192];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(
                    buffer
                        .get(..bytes_read)
                        .ok_or_else(|| anyhow!("Buffer overflow"))?
                );
            }
            Ok(hex::encode(hasher.finalize()))
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            let mut buffer = [0; 8192];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(
                    buffer
                        .get(..bytes_read)
                        .ok_or_else(|| anyhow!("Buffer overflow"))?
                );
            }
            Ok(hex::encode(hasher.finalize()))
        }
    }
}

/// Calculates the cryptographic hash of any reader, along with the total
/// number of bytes consumed. Useful for hashing entries while streaming an
/// archive without touching the filesystem.
///
/// # Errors
///
/// Returns an error if reading from the reader fails.
pub fn calculate_reader_hash(
    reader: &mut impl Read,
    algo: HashAlgorithm
) -> Result<(u64, String)> {
    let mut buffer = [0u8; 8192];
    let mut len: u64 = 0;

    macro_rules! stream {
        ($hasher:expr) => {{
            loop {
                let bytes_read = reader.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                len += bytes_read as u64;
                $hasher.update(
                    buffer
                        .get(..bytes_read)
                        .ok_or_else(|| anyhow!("Buffer overflow"))?
                );
            }
            hex::encode($hasher.finalize())
        }};
    }

    match algo {
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            let result = stream!(hasher);
            Ok((len, result))
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            let result = stream!(hasher);
            Ok((len, result))
        }
    }
}

/// Calculates the cryptographic hash of a string.
pub fn calculate_string_hash(input: &str, algo: HashAlgorithm) -> String {
    match algo {
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

/// Calculates a recursive hash of an entire directory's contents.
///
/// Deterministic Algorithm:
/// - Collects all files in the directory.
/// - Sorts files by their relative paths to ensure consistency.
/// - Hashes each file's relative path, its metadata length, and finally its raw
///   content.
///
/// This provides a single SHA-512 "Snapshot" hash that represents the exact
/// state of the directory, used for bit-for-bit reproducibility checks in
/// Specification v2.
///
/// # Errors
///
/// Returns an error if:
/// - The path is not a directory.
/// - Any file within the directory cannot be read.
pub fn calculate_dir_hash(path: &Path) -> Result<String> {
    if !path.is_dir() {
        return Err(anyhow!("Path is not a directory"));
    }

    let mut hasher = Sha512::new();
    let mut paths = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_file() {
            paths.push(entry.path().to_path_buf());
        }
    }

    paths.sort();

    for file_path in paths {
        if let Ok(rel_path) = file_path.strip_prefix(path) {
            let rel_path_str =
                rel_path.to_string_lossy().to_string().replace('\\', "/");
            let path_bytes = rel_path_str.as_bytes();
            hasher.update((path_bytes.len() as u64).to_le_bytes());
            hasher.update(path_bytes);
        }

        let mut file = fs::File::open(&file_path)?;
        let metadata = file.metadata()?;
        hasher.update(metadata.len().to_le_bytes());

        let mut buffer = [0; 8192];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(
                buffer
                    .get(..bytes_read)
                    .ok_or_else(|| anyhow!("Buffer overflow"))?
            );
        }
    }

    Ok(hex::encode(hasher.finalize()))
}
