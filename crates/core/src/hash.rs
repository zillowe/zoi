use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

/// Supported hashing algorithms for integrity verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// SHA-512 (128 character hex string).
    Sha512,
    /// SHA-256 (64 character hex string).
    Sha256,
}

impl HashAlgorithm {
    pub fn from_len(len: usize) -> Option<Self> {
        match len {
            128 => Some(HashAlgorithm::Sha512),
            64 => Some(HashAlgorithm::Sha256),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "sha512" => Some(HashAlgorithm::Sha512),
            "sha256" => Some(HashAlgorithm::Sha256),
            _ => None,
        }
    }
}

/// Calculates the cryptographic hash of a single file.
pub fn calculate_file_hash(path: &Path, algo: HashAlgorithm) -> Result<String> {
    let mut file =
        fs::File::open(path).map_err(|e| anyhow!("Failed to open file {:?}: {}", path, e))?;

    match algo {
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            let mut buffer = [0; 8192];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
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
                hasher.update(&buffer[..bytes_read]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
    }
}

/// Calculates a recursive hash of an entire directory's contents.
///
/// Deterministic Algorithm:
/// - Collects all files in the directory.
/// - Sorts files by their relative paths to ensure consistency.
/// - Hashes each file's relative path, its metadata length, and finally its raw content.
///
/// This provides a single SHA-512 "Snapshot" hash that represents the exact state
/// of the directory, used for bit-for-bit reproducibility checks in Specification v2.
pub fn calculate_dir_hash(path: &Path) -> Result<String> {
    if !path.is_dir() {
        return Err(anyhow!("Path is not a directory"));
    }

    let mut hasher = Sha512::new();
    let mut paths = Vec::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            paths.push(entry.path().to_path_buf());
        }
    }

    paths.sort();

    for file_path in paths {
        if let Ok(rel_path) = file_path.strip_prefix(path) {
            let rel_path_str = rel_path.to_string_lossy().to_string().replace('\\', "/");
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
            hasher.update(&buffer[..bytes_read]);
        }
    }

    Ok(hex::encode(hasher.finalize()))
}
