use std::sync::{OnceLock, RwLock};

fn offline_mode_store() -> &'static RwLock<bool> {
    static OFFLINE_MODE: OnceLock<RwLock<bool>> = OnceLock::new();
    OFFLINE_MODE.get_or_init(|| RwLock::new(false))
}

/// Sets the global offline mode.
pub fn set_offline(offline: bool) {
    if let Ok(mut guard) = offline_mode_store().write() {
        *guard = offline;
    }
}

/// Returns true if Zoi is in offline mode.
pub fn is_offline() -> bool {
    offline_mode_store().read().map(|g| *g).unwrap_or(false)
}
