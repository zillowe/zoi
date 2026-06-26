use std::env;
use std::error::Error;
use std::path::Path;

const BUILD_ENV_VARS: &[&str] = &[
    "ZOI_COMMIT_HASH",
    "POSTHOG_API_KEY",
    "POSTHOG_API_HOST",
    "ZOI_ABOUT_PACKAGER_AUTHOR",
    "ZOI_ABOUT_PACKAGER_EMAIL",
    "ZOI_ABOUT_PACKAGER_HOMEPAGE",
    "ZOI_DEFAULT_REGISTRY",
];
const DEFAULT_REGISTRY: &str = "https://gitlab.com/zillowe/zillwen/zusty/zoidberg.git";
const AUTHORITIES_KEY_RANGE: std::ops::Range<usize> = 1..10;

fn forward_env_var(var: &str) {
    if let Ok(val) = env::var(var) {
        println!("cargo:rustc-env={var}={val}");
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    for var in BUILD_ENV_VARS {
        println!("cargo:rerun-if-env-changed={var}");
    }
    for i in AUTHORITIES_KEY_RANGE {
        println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_{i}");
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

    for var in BUILD_ENV_VARS
        .iter()
        .copied()
        .filter(|var| *var != "ZOI_DEFAULT_REGISTRY")
    {
        forward_env_var(var);
    }

    let zoi_registry = env::var("ZOI_DEFAULT_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY.into());
    println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={zoi_registry}");

    let mut authorities = Vec::new();
    for i in AUTHORITIES_KEY_RANGE {
        if let Ok(val) = env::var(format!("ZOI_AUTHORITIES_KEY_{i}"))
            && !val.is_empty()
        {
            authorities.push(val);
        }
    }
    println!(
        "cargo:rustc-env=ZOI_BUILTIN_AUTHORITIES={}",
        authorities.join(",")
    );

    Ok(())
}
