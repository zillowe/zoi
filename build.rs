use std::fs;
use std::path::Path;

fn main() {
    let env_path_str = if Path::new(".env").exists() {
        ".env"
    } else {
        ".env.local"
    };

    let env_path = Path::new(env_path_str);

    if env_path.exists() {
        println!("cargo:rerun-if-changed={}", env_path_str);

        let content = fs::read_to_string(env_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", env_path_str));

        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                if !key.starts_with('#') && !key.is_empty() {
                    let value = value.trim_matches('"');
                    println!("cargo:rustc-env={}={}", key, value);
                }
            }
        }
    }
}
