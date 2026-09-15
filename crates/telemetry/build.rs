//! Build script for zoi-telemetry.
//!
//! Forwards opt-in telemetry configuration (`PostHog` + Sentry) from the build
//! environment / `.env` file into the compiled crate via `option_env!`.

use std::env;
use std::path::Path;

/// Telemetry-related variables embedded at compile time.
const TELEMETRY_ENV_VARS: &[&str] = &[
    "POSTHOG_API_KEY",
    "POSTHOG_API_HOST",
    "SENTRY_DSN",
    "SENTRY_ENVIRONMENT"
];

/// Forwards an environment variable to `rustc` if set and non-empty.
fn forward_env_var(var: &str) {
    if let Ok(val) = env::var(var) {
        println!("cargo:rustc-env={var}={val}");
    }
}

fn main() {
    for var in TELEMETRY_ENV_VARS {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let env_path = if Path::new(".env").exists() {
        Some(".env")
    } else if Path::new(".env.local").exists() {
        Some(".env.local")
    } else {
        None
    };

    if let Some(path) = env_path {
        println!("cargo:rerun-if-changed={path}");
        if dotenvy::from_filename(path).is_err() {
            println!("cargo:warning=failed to load env file: {path}");
        }
    }

    for var in TELEMETRY_ENV_VARS {
        forward_env_var(var);
    }
}
