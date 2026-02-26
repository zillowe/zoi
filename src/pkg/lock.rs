use anyhow::{Result, anyhow};
use colored::*;
use fs2::FileExt;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

fn get_lock_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("lock"))
}

pub fn acquire_lock() -> Result<LockGuard> {
    let lock_path = get_lock_path()?;

    if let Some(parent) = lock_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!(
            "Warning: could not create lock directory {}: {}",
            parent.display(),
            e
        );
    }

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;

    if file.try_lock_exclusive().is_err() {
        let mut content = String::new();
        if let Ok(mut f) = fs::File::open(&lock_path) {
            let _ = f.read_to_string(&mut content);
        }

        let pid_info = if !content.trim().is_empty() {
            format!(" (held by PID {})", content.trim())
        } else {
            String::new()
        };

        eprintln!(
            "{}: Another Zoi process{} may be running.",
            "Error".red().bold(),
            pid_info
        );
        eprintln!(
            "If you are absolutely sure no other Zoi process is running, you can manually remove the lock file:"
        );
        eprintln!("  {}", lock_path.display());
        return Err(anyhow!("Could not acquire lock."));
    }

    let mut file = file;
    let _ = file.set_len(0);
    let _ = file.seek(SeekFrom::Start(0));
    let _ = write!(file, "{}", std::process::id());
    let _ = file.flush();

    Ok(LockGuard {
        path: Some(lock_path),
        _file: Some(file),
    })
}

pub fn release_lock() -> Result<()> {
    let lock_path = get_lock_path()?;
    if lock_path.exists() {
        let _ = fs::remove_file(lock_path);
    }
    Ok(())
}

pub struct LockGuard {
    path: Option<PathBuf>,
    _file: Option<fs::File>,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            self._file.take();

            if path.exists()
                && let Err(e) = fs::remove_file(&path)
            {
                debug_assert!(false, "Failed to remove lock file: {}", e);
            }
        }
    }
}
