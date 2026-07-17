use anyhow::{Result, anyhow};
use colored::*;
use elb::{DynamicTag, Elf, ElfPatcher};
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

/// Automatically identifies ELF binaries and shared libraries in the staging area
/// and rewrites their RUNPATH/RPATH to use `$ORIGIN` relative paths.
///
/// This ensures that Zoi packages are "Relocatable": they can find their internal
/// shared libraries regardless of where they are symlinked (e.g. system vs user scope).
pub fn relocate_elfs(staging_dir: &Path, quiet: bool) -> Result<()> {
    if !quiet {
        println!("{} Relocating ELF binaries...", "::".bold().blue());
    }

    let mut relocated_count = 0;

    for entry in WalkDir::new(staging_dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if is_elf(path)? {
            match relocate_file(path, staging_dir) {
                Ok(true) => {
                    relocated_count += 1;
                    if !quiet {
                        println!("  Relocated: {}", path.strip_prefix(staging_dir)?.display());
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!(
                        "{} Failed to relocate {}: {}",
                        "Warning:".yellow(),
                        path.display(),
                        e
                    );
                }
            }
        }
    }

    if !quiet && relocated_count > 0 {
        println!(
            "{} Successfully relocated {} ELF file(s).",
            "::".bold().green(),
            relocated_count
        );
    }

    Ok(())
}

fn is_elf(path: &Path) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return Ok(false);
    }
    // \x7fELF
    Ok(magic == [0x7f, 0x45, 0x4c, 0x46])
}

fn relocate_file(path: &Path, staging_dir: &Path) -> Result<bool> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| anyhow!("Failed to open ELF for writing: {}", e))?;

    let page_size = 4096;
    let elf = Elf::read(&mut file, page_size).map_err(|e| anyhow!("Failed to read ELF: {}", e))?;
    let mut patcher = ElfPatcher::new(elf, file);

    let rpaths = calculate_rpaths(path, staging_dir);
    let rpath_string = rpaths.join(":");
    let c_rpath = CString::new(rpath_string).map_err(|e| anyhow!("Invalid RPATH string: {}", e))?;

    // MODERN: We set RUNPATH (DT_RUNPATH) which is preferred over RPATH.
    patcher
        .set_dynamic_tag(DynamicTag::Runpath, c_rpath.as_c_str())
        .map_err(|e| anyhow!("Failed to set RUNPATH: {}", e))?;

    patcher
        .finish()
        .map_err(|e| anyhow!("Failed to write relocated ELF: {}", e))?;

    Ok(true)
}

fn calculate_rpaths(path: &Path, staging_dir: &Path) -> Vec<String> {
    // Determine common Zoi relative paths based on where the file is in the staging area.
    // We look for the 'pkgstore' directory which is the root of the package's isolated files.
    let mut rpaths = Vec::new();
    rpaths.push("$ORIGIN".to_string());

    // Calculate depth to pkgstore to add ../lib etc.
    if let Some(rel_path) = find_rel_to_pkgstore(path, staging_dir) {
        let depth = rel_path.components().count();
        if depth > 1 {
            // If it's in a subdirectory (like bin/ or lib/), we add the parent's lib dirs.
            // e.g. bin/foo -> $ORIGIN/../lib
            rpaths.push("$ORIGIN/../lib".to_string());
            rpaths.push("$ORIGIN/../lib64".to_string());
        }
        if depth > 2 {
            // e.g. lib/plugins/foo.so -> $ORIGIN/../..
            rpaths.push("$ORIGIN/..".to_string());
            rpaths.push("$ORIGIN/../../lib".to_string());
            rpaths.push("$ORIGIN/../../lib64".to_string());
        }
    }
    rpaths
}

fn find_rel_to_pkgstore(path: &Path, staging_dir: &Path) -> Option<std::path::PathBuf> {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if parent.file_name().and_then(|s| s.to_str()) == Some("pkgstore") {
            return path.strip_prefix(parent).ok().map(|p| p.to_path_buf());
        }
        if parent == staging_dir {
            break;
        }
        current = parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_calculate_rpaths_root() {
        let staging = PathBuf::from("/tmp/staging");
        let pkgstore = staging.join("data/pkgstore");
        let bin = pkgstore.join("my-bin");

        let rpaths = calculate_rpaths(&bin, &staging);
        assert!(rpaths.contains(&"$ORIGIN".to_string()));
        assert_eq!(rpaths.len(), 1);
    }

    #[test]
    fn test_calculate_rpaths_bin() {
        let staging = PathBuf::from("/tmp/staging");
        let pkgstore = staging.join("data/pkgstore");
        let bin = pkgstore.join("bin/my-bin");

        let rpaths = calculate_rpaths(&bin, &staging);
        assert!(rpaths.contains(&"$ORIGIN".to_string()));
        assert!(rpaths.contains(&"$ORIGIN/../lib".to_string()));
    }
}
