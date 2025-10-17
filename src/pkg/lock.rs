use anyhow::{Result, anyhow};
use colored::*;
use std::fs;
use std::path::PathBuf;

fn get_lock_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("lock"))
}

pub fn acquire_lock() -> Result<LockGuard> {
    let lock_path = match get_lock_path() {
        Ok(p) => p,
        Err(_) => {
            return Ok(LockGuard { path: None });
        }
    };

    if lock_path.exists() {
        eprintln!(
            "{}: Another Zoi process may be running.",
            "Error".red().bold()
        );
        eprintln!(
            "If you are sure no other Zoi process is running, you can remove the lock file at:"
        );
        eprintln!("  {}", lock_path.display());
        eprintln!("This can happen if a previous operation was interrupted.");
        return Err(anyhow!("Could not acquire lock."));
    }

    if let Some(parent) = lock_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!(
            "Warning: could not create lock directory {}: {}",
            parent.display(),
            e
        );
        return Ok(LockGuard { path: None });
    }

    match fs::File::create(&lock_path) {
        Ok(_) => Ok(LockGuard {
            path: Some(lock_path),
        }),
        Err(e) => {
            eprintln!("Warning: could not create lock file: {}", e);
            Ok(LockGuard { path: None })
        }
    }
}

pub struct LockGuard {
    path: Option<PathBuf>,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.path
            && path.exists()
            && let Err(e) = fs::remove_file(path)
        {
            eprintln!("Warning: Failed to remove lock file: {}", e);
        }
    }
}
