use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
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
        if let Ok(val) = env::var(&key) {
            if !val.is_empty() {
                authorities.push(val);
            }
        }
    }
    println!(
        "cargo:rustc-env=ZOI_BUILTIN_AUTHORITIES={}",
        authorities.join(",")
    );

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_pgp_keys.rs");
    generate_pgp_keys(&dest_path);

    Ok(())
}

fn generate_pgp_keys(dest_path: &Path) {
    let pgp_dir = PathBuf::from("src/builtin/pgp");
    let mut output = String::from("pub static BUILTIN_KEYS: &[(&str, &[u8])] = &[\n");

    if pgp_dir.exists() {
        if let Ok(entries) = fs::read_dir(&pgp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "asc") {
                    if let Ok(data) = fs::read(&path) {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        output.push_str(&format!("    (\"{}\", &{:?}),\n", name, data));
                    }
                }
            }
        }
    }

    output.push_str("];\n");
    fs::write(dest_path, output).ok();
}
