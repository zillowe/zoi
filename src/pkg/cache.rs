use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub fn get_cache_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("cache"))
}

pub fn get_alt_cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let cache_root = get_cache_root()?;
    let alt_dir = cache_root.join("alt");
    fs::create_dir_all(&alt_dir)?;
    Ok(alt_dir)
}

fn get_cached_alt_path(url: &str) -> Result<PathBuf, Box<dyn Error>> {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let result = hasher.finalize();
    let hash = format!("{:x}", result);
    let alt_cache_dir = get_alt_cache_dir()?;
    Ok(alt_cache_dir.join(format!("{}.pkg.yaml", hash)))
}

pub fn cache_alt_source(url: &str, content: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = get_cached_alt_path(url)?;
    fs::write(&path, content)?;
    Ok(path)
}

pub fn get_cached_alt_source_path(url: &str) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let path = get_cached_alt_path(url)?;
    if path.exists() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

pub fn clear() -> Result<(), Box<dyn Error>> {
    let cache_dir = get_cache_root()?;
    if cache_dir.exists() {
        println!("Removing cache directory: {}", cache_dir.display());
        fs::remove_dir_all(cache_dir)?;
    } else {
        println!("Cache directory does not exist. Nothing to clean.");
    }
    Ok(())
}
