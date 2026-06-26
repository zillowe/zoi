use std::env;
use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};

fn read_sorted_paths(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some(extension) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let hooks_dir = manifest_dir.join("../core/src/builtin/hooks");
    println!("cargo:rerun-if-changed={}", hooks_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let dest_path = out_dir.join("generated_builtin_hooks.rs");
    let mut file = std::fs::File::create(dest_path)?;

    writeln!(&mut file, "pub static BUILTIN_HOOKS: &[(&str, &str)] = &[")?;
    for path in read_sorted_paths(&hooks_dir, "yaml")? {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let content = std::fs::read_to_string(&path)?;
        writeln!(&mut file, "    ({:?}, {:?}),", name, content)?;
    }
    writeln!(&mut file, "];")?;

    Ok(())
}
