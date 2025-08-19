use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub fn get_cache_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("cache"))
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
