use std::env;
use std::path::Path;

fn main() {
    let env_path = if Path::new(".env").exists() {
        Some(".env")
    } else if Path::new(".env.local").exists() {
        Some(".env.local")
    } else {
        None
    };

    if let Some(path) = env_path {
        println!("cargo:rerun-if-changed={}", path);
        if dotenvy::from_filename(path).is_err() {
            println!("cargo:warning=failed to load env file: {}", path);
        }
    }

    if let Ok(val) = env::var("POSTHOG_API_KEY") {
        println!("cargo:rustc-env=POSTHOG_API_KEY={}", val);
    }

    if let Ok(val) = env::var("POSTHOG_API_HOST") {
        println!("cargo:rustc-env=POSTHOG_API_HOST={}", val);
    }

    let zoi_registry = env::var("ZOI_DEFAULT_REGISTRY")
        .unwrap_or_else(|_| "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git".to_string());
    println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={}", zoi_registry);
}
