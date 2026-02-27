use std::path::PathBuf;
use std::sync::OnceLock;

static PKG_DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Sets the global package search directories.
pub fn set_pkg_dirs(dirs: Vec<PathBuf>) {
    let _ = PKG_DIRS.set(dirs);
}

/// Returns the list of global package search directories.
pub fn get_pkg_dirs() -> &'static [PathBuf] {
    PKG_DIRS.get().map(|d| d.as_slice()).unwrap_or(&[])
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
