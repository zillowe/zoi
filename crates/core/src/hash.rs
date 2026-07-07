use anyhow::{Result, anyhow};
use md5;
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha512,
    Sha256,
    Md5,
}

impl HashAlgorithm {
    pub fn from_len(len: usize) -> Option<Self> {
        match len {
            128 => Some(HashAlgorithm::Sha512),
            64 => Some(HashAlgorithm::Sha256),
            32 => Some(HashAlgorithm::Md5),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "sha512" => Some(HashAlgorithm::Sha512),
            "sha256" => Some(HashAlgorithm::Sha256),
            "md5" => Some(HashAlgorithm::Md5),
            _ => None,
        }
    }
}

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
        HashAlgorithm::Md5 => {
            let mut hasher = md5::Context::new();
            let mut buffer = [0; 8192];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.consume(&buffer[..bytes_read]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
    }
}

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
