use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

fn pkg_dirs_store() -> &'static RwLock<Vec<PathBuf>> {
    static PKG_DIRS: OnceLock<RwLock<Vec<PathBuf>>> = OnceLock::new();
    PKG_DIRS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Sets the global package search directories.
pub fn set_pkg_dirs(dirs: Vec<PathBuf>) {
    if let Ok(mut guard) = pkg_dirs_store().write() {
        *guard = dirs;
    }
}

/// Returns the list of global package search directories.
pub fn get_pkg_dirs() -> Vec<PathBuf> {
    pkg_dirs_store()
        .read()
        .map(|dirs| dirs.clone())
        .unwrap_or_default()
}

/// Checks if an archive exists in any of the configured pkg-dirs.
/// Returns the path to the archive if found.
pub fn find_in_pkg_dirs(filename: &str) -> Option<PathBuf> {
    for dir in get_pkg_dirs() {
        let path = dir.join(filename);
        if path.exists() && path.is_file() {
            return Some(path);
        }
    }
    None
}
