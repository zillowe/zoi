use std::env;
use std::path::Path;

fn main() {
    let env_path_str = if Path::new(".env").exists() {
        ".env"
    } else {
        ".env.local"
    };

    let mut registry_is_set = false;
    if Path::new(env_path_str).exists() {
        println!("cargo:rerun-if-changed={}", env_path_str);

        if let Ok(iter) = dotenvy::from_filename_iter(env_path_str) {
            for (key, val) in iter.flatten() {
                if key == "ZOI_DEFAULT_REGISTRY" {
                    registry_is_set = true;
                }
                println!("cargo:rustc-env={}={}", key, val);
            }
        } else {
            println!("cargo:warning=failed to load env file: {}", env_path_str);
        }
    }

    if let Ok(val) = env::var("ZOI_DEFAULT_REGISTRY") {
        if !registry_is_set {
            println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={}", val);
        }
        registry_is_set = true;
    }

    if !registry_is_set {
        println!(
            "cargo:rustc-env=ZOI_DEFAULT_REGISTRY=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git"
        );
    }
}
