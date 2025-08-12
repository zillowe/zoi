use std::path::Path;

fn main() {
    let env_path_str = if Path::new(".env").exists() {
        ".env"
    } else {
        ".env.local"
    };

    if Path::new(env_path_str).exists() {
        println!("cargo:rerun-if-changed={}", env_path_str);

        if let Ok(iter) = dotenvy::from_filename_iter(env_path_str) {
            for item in iter {
                if let Ok((key, val)) = item {
                    println!("cargo:rustc-env={}={}", key, val);
                }
            }
        } else {
            println!("cargo:warning=failed to load env file: {}", env_path_str);
        }
    }
}
