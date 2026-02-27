use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static SYSROOT: OnceLock<PathBuf> = OnceLock::new();

/// Sets the global sysroot path. This should be called once at the start of the program.
pub fn set_sysroot(path: PathBuf) {
    let _ = SYSROOT.set(path);
}

/// Returns the global sysroot path if it has been set.
pub fn get_sysroot() -> Option<&'static Path> {
    SYSROOT.get().map(|p| p.as_path())
}

/// Prepends the sysroot to the given path if a sysroot is set.
/// If the path is absolute, it is made relative to the current root before joining.
pub fn apply_sysroot(path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    if let Some(root) = get_sysroot() {
        if path.is_absolute() {
            let mut components = path.components();
            components.next();
            root.join(components.as_path())
        } else {
            root.join(path)
        }
    } else {
        path
    }
}
