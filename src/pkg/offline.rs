use std::sync::OnceLock;

static OFFLINE_MODE: OnceLock<bool> = OnceLock::new();

/// Sets the global offline mode.
pub fn set_offline(offline: bool) {
    let _ = OFFLINE_MODE.set(offline);
}

/// Returns true if Zoi is in offline mode.
pub fn is_offline() -> bool {
    *OFFLINE_MODE.get().unwrap_or(&false)
}
