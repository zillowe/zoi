use anyhow::{Result, anyhow};
use sha2::{Digest, Sha512};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

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
        let file_content = fs::read(&file_path)?;
        hasher.update(&file_content);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
