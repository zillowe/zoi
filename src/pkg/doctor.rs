use crate::pkg::types::Scope;
use anyhow::{Result, anyhow};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

fn get_bin_root(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(home_dir.join(".zoi/pkgs/bin"))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                Ok(PathBuf::from("/usr/local/bin"))
            }
        }
        Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

pub fn check_broken_symlinks() -> Result<Vec<PathBuf>> {
    let mut broken_links = Vec::new();
    let scopes = [Scope::User, Scope::System, Scope::Project];

    for &scope in &scopes {
        let root = get_bin_root(scope)?;
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_symlink() && !path.exists() {
                broken_links.push(path);
            }
        }
    }
    Ok(broken_links)
}

pub fn check_path_configuration() -> Result<Option<String>> {
    if let Some(home) = home::home_dir() {
        let zoi_bin_dir = home.join(".zoi").join("pkgs").join("bin");
        if !zoi_bin_dir.exists() {
            return Ok(None);
        }

        if let Ok(path_var) = std::env::var("PATH")
            && !std::env::split_paths(&path_var).any(|p| p == zoi_bin_dir)
        {
            return Ok(Some(format!(
                "Zoi's user binary directory ({}) is not in your PATH.",
                zoi_bin_dir.display()
            )));
        }
    }
    Ok(None)
}

pub fn check_outdated_repos() -> Result<Option<String>> {
    let db_root = crate::pkg::resolve::get_db_root()?;
    let config = crate::pkg::config::read_config()?;

    if let Some(default_reg) = config.default_registry
        && !default_reg.handle.is_empty()
    {
        let repo_path = db_root.join(default_reg.handle);
        let fetch_head = repo_path.join(".git/FETCH_HEAD");
        if fetch_head.exists() {
            let metadata = fs::metadata(fetch_head)?;
            if let Ok(modified) = metadata.modified()
                && let Ok(since_modified) = SystemTime::now().duration_since(modified)
                && since_modified.as_secs() > 60 * 60 * 24 * 7
            {
                return Ok(Some(format!(
                    "Default repository has not been synced in over a week (last sync: {} days ago).",
                    since_modified.as_secs() / (60 * 60 * 24)
                )));
            }
        } else if repo_path.join(".git").exists() {
            return Ok(Some(
                "Default repository has never been synced.".to_string(),
            ));
        }
    }

    Ok(None)
}
