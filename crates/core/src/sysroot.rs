use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

fn sysroot_store() -> &'static RwLock<Option<PathBuf>> {
    static SYSROOT: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    SYSROOT.get_or_init(|| RwLock::new(None))
}

/// Sets or replaces the global sysroot path.
pub fn set_sysroot(path: PathBuf) {
    if let Ok(mut guard) = sysroot_store().write() {
        *guard = Some(path);
    }
}

/// Clears the global sysroot path.
pub fn clear_sysroot() {
    if let Ok(mut guard) = sysroot_store().write() {
        *guard = None;
    }
}

/// Returns the global sysroot path if it has been set.
pub fn get_sysroot() -> Option<PathBuf> {
    sysroot_store().read().ok().and_then(|g| g.clone())
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
