use std::sync::{OnceLock, RwLock};

fn frozen_mode_store() -> &'static RwLock<bool> {
    static FROZEN_MODE: OnceLock<RwLock<bool>> = OnceLock::new();
    FROZEN_MODE.get_or_init(|| RwLock::new(false))
}

/// Sets the global frozen mode.
pub fn set_frozen(frozen: bool) {
    if let Ok(mut guard) = frozen_mode_store().write() {
        *guard = frozen;
    }
}

/// Returns true if Zoi is in frozen mode.
pub fn is_frozen() -> bool {
    frozen_mode_store().read().map(|g| *g).unwrap_or(false)
}
