use std::fs;
use std::path::PathBuf;

use anyhow::Result;

/// Returns the root directory for Zoi's cache.
///
/// # Errors
///
/// Returns an error if the user's home directory cannot be determined.
pub fn get_cache_root() -> Result<PathBuf> {
    crate::utils::get_user_cache_dir()
}

/// Returns the root directory for Zoi's archive cache.
///
/// While installing into a sysroot (`zoi system distro build`, `--root`,
/// `--pure` builds), archives are kept on the host instead of in the target:
/// they are downloads, not state of the system being built, and keeping them
/// lets a failed or repeated build reuse them instead of downloading
/// everything again. Every archive is still verified (hash and signature)
/// each time it is used. As root that is `/var/cache/zoi/archives`, so no
/// root-owned files land in the invoking user's home; otherwise it is the
/// user's own archive cache.
///
/// # Errors
///
/// Returns an error if the cache root directory cannot be determined.
pub fn get_archive_cache_root() -> Result<PathBuf> {
    if crate::sysroot::get_sysroot().is_some() {
        return host_archive_cache_root();
    }
    let cache_root = get_cache_root()?;
    Ok(cache_root.join("archives"))
}

/// The archive cache on the host, used while a sysroot is active.
fn host_archive_cache_root() -> Result<PathBuf> {
    if crate::utils::is_effective_root() {
        let base = if cfg!(target_os = "macos") {
            PathBuf::from("/Library/Caches/zoi")
        } else if cfg!(target_os = "windows") {
            PathBuf::from("C:\\ProgramData\\zoi\\cache")
        } else {
            PathBuf::from("/var/cache/zoi")
        };
        return Ok(base.join("archives"));
    }
    Ok(crate::utils::get_host_user_cache_dir()?.join("archives"))
}

/// Returns the root directory for Zoi's package definition cache.
///
/// # Errors
///
/// Returns an error if the cache root directory cannot be determined.
pub fn get_pkgdef_cache_root() -> Result<PathBuf> {
    let cache_root = get_cache_root()?;
    Ok(cache_root.join("pkgdefs"))
}

/// Returns a list of candidate URLs for a given URL, including configured
/// mirrors.
pub fn mirror_candidate_urls(url: &str) -> Vec<String> {
    let mut urls = vec![url.to_string()];
    let Ok(config) = crate::config::read_config() else {
        return urls;
    };

    let Some(filename) =
        url.split('/').next_back().filter(|part| !part.is_empty())
    else {
        return urls;
    };

    for mirror in config.cache_mirrors {
        urls.push(format!("{}/{}", mirror.trim_end_matches('/'), filename));
    }

    urls
}

/// Clears the entire Zoi cache.
///
/// # Errors
///
/// Returns an error if the cache directory cannot be removed.
pub fn clear(dry_run: bool) -> Result<()> {
    let cache_dir = get_cache_root()?;
    if cache_dir.exists() {
        if dry_run {
            println!(
                "(Dry-run) Would remove cache directory: {}",
                cache_dir.display()
            );
        } else {
            println!("Removing cache directory: {}", cache_dir.display());
            fs::remove_dir_all(cache_dir)?;
        }
    } else {
        println!("Cache directory does not exist. Nothing to clean.");
    }
    Ok(())
}

/// Clears only the archive cache.
///
/// # Errors
///
/// Returns an error if the archive cache directory cannot be removed.
pub fn clear_archives(dry_run: bool) -> Result<()> {
    let archive_cache_dir = get_archive_cache_root()?;
    if archive_cache_dir.exists() {
        if dry_run {
            println!(
                "(Dry-run) Would remove archive cache directory: {}",
                archive_cache_dir.display()
            );
        } else {
            println!(
                "Removing archive cache directory: {}",
                archive_cache_dir.display()
            );
            fs::remove_dir_all(archive_cache_dir)?;
        }
    } else {
        println!("Archive cache directory does not exist. Nothing to clean.");
    }
    Ok(())
}
