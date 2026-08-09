//! Build script for zoi-core.
//!
//! This script is responsible for embedding the default environment variables
//! and generating Rust code that includes the built-in PGP keys for signature verification.

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=ZOI_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=ZOI_DEFAULT_REGISTRY");
    println!("cargo:rerun-if-env-changed=ZOI_BUILTIN_AUTHORITIES");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_1");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_2");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_3");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_4");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_5");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_6");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_7");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_8");
    println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_9");

    let zoi_registry = env::var("ZOI_DEFAULT_REGISTRY")
        .unwrap_or_else(|_| "https://gitlab.com/zillowe/zillwen/zusty/zoidberg.git".into());
    println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={zoi_registry}");

    let mut authorities = Vec::new();
    for i in 1..=9 {
        let key = format!("ZOI_AUTHORITIES_KEY_{i}");
        if let Ok(val) = env::var(&key)
            && !val.is_empty()
        {
            authorities.push(val);
        }
    }
    println!(
        "cargo:rustc-env=ZOI_BUILTIN_AUTHORITIES={}",
        authorities.join(",")
    );

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR should be set by cargo");
    let dest_path = Path::new(&out_dir).join("generated_pgp_keys.rs");
    generate_pgp_keys(&dest_path);
}

/// Reads PGP key files from the `src/builtin/pgp` directory and generates a Rust source
/// file containing the keys as static byte arrays.
fn generate_pgp_keys(dest_path: &Path) {
    use std::fmt::Write;
    let pgp_dir = PathBuf::from("src/builtin/pgp");
    let mut output = String::from("/// A list of built-in PGP keys for registry and package verification.\n///\n/// Each entry is a tuple of (`key_name`, `raw_key_bytes`).\npub static BUILTIN_KEYS: &[(&str, &[u8])] = &[\n");

    if pgp_dir.exists()
        && let Ok(entries) = fs::read_dir(&pgp_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "asc")
                && let Ok(data) = fs::read(&path)
            {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let _ = writeln!(output, "    (\"{name}\", &{data:?}),");
            }
        }
    }

    output.push_str("];\n");
    fs::write(dest_path, output).ok();
}
