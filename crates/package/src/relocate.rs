//! ELF binary relocation for portability.
//!
//! This module provides utilities for rewriting the RPATH/RUNPATH of ELF
//! binaries to use relative paths (using `$ORIGIN`). This makes packages
//! relocatable, allowing them to find their shared libraries regardless of
//! their install location.

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::Path;

use anyhow::{Result, anyhow};
use colored::Colorize;
use elb::{DynamicTag, Elf, ElfPatcher};
use walkdir::WalkDir;

/// Automatically identifies ELF binaries and shared libraries in the staging
/// area and rewrites their RUNPATH/RPATH to use `$ORIGIN` relative paths.
///
/// This ensures that Zoi packages are "Relocatable": they can find their
/// internal shared libraries regardless of where they are symlinked (e.g.
/// system vs user scope).
///
/// # Errors
///
/// Returns an error if the staging directory cannot be walked or if ELF magic
/// bytes cannot be read.
pub fn relocate_elfs(staging_dir: &Path, quiet: bool) -> Result<()> {
    if !quiet {
        println!("{} Relocating ELF binaries...", "::".bold().blue());
    }

    let mut relocated_count = 0;

    for entry in WalkDir::new(staging_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !in_package_store(path, staging_dir) {
            continue;
        }
        if is_elf(path)? {
            match relocate_file(path, staging_dir) {
                Ok(true) => {
                    relocated_count += 1;
                    if !quiet {
                        println!(
                            "  Relocated: {}",
                            path.strip_prefix(staging_dir)?.display()
                        );
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!(
                        "{} Failed to relocate {}: {e}",
                        "Warning:".yellow(),
                        path.display(),
                    );
                }
            }
        }
    }

    if !quiet && relocated_count > 0 {
        println!(
            "{} Successfully relocated {relocated_count} ELF file(s).",
            "::".bold().green(),
        );
    }

    Ok(())
}

/// Whether a staged file installs into the package store, the only place
/// `$ORIGIN` RUNPATHs are for.
///
/// Files staged for the system root (`usrroot`), a home (`usrhome`) or a
/// template directory land in standard locations such as `/usr/lib` and
/// already find their libraries through the system's search paths.
/// Rewriting them gained nothing and could break them: a new dynamic entry
/// moves the dynamic section into a fresh segment, which the dynamic loader
/// (`ld-linux-*.so`) cannot survive, so glibc's loader crashed on start and
/// took every program in the system with it.
fn in_package_store(path: &Path, staging_dir: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(staging_dir) else {
        return false;
    };
    !rel.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("usrroot" | "usrhome" | "createpkgdir")
        )
    })
}

/// Checks if a file is an ELF binary by looking for the `\x7fELF` magic bytes.
fn is_elf(path: &Path) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return Ok(false);
    }
    // \x7fELF
    Ok(magic == [0x7f, 0x45, 0x4c, 0x46])
}

/// Relocates a single ELF file by rewriting its RUNPATH.
fn relocate_file(path: &Path, staging_dir: &Path) -> Result<bool> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| anyhow!("Failed to open ELF for writing: {e}"))?;

    let page_size = 4096;
    let elf = Elf::read(&mut file, page_size)
        .map_err(|e| anyhow!("Failed to read ELF: {e}"))?;
    let mut patcher = ElfPatcher::new(elf, file);

    // Without DT_NEEDED entries the file loads no libraries, so a RUNPATH
    // has nothing to do. That covers the dynamic loader itself and static
    // PIE executables, whose layout must not be touched.
    let Some(table) = patcher
        .read_dynamic_table()
        .map_err(|e| anyhow!("Failed to read dynamic table: {e}"))?
    else {
        return Ok(false);
    };
    if !table.iter().any(|(tag, _)| *tag == DynamicTag::Needed) {
        return Ok(false);
    }

    // Keep any RUNPATH/RPATH the package's build set, after ours: replacing
    // it broke programs that rely on a specific library directory.
    let existing = existing_runpath(&mut patcher, &table);

    let mut rpaths = calculate_rpaths(path, staging_dir);
    for entry in existing.split(':').filter(|e| !e.is_empty()) {
        if !rpaths.iter().any(|r| r == entry) {
            rpaths.push(entry.to_string());
        }
    }
    let rpath_string = rpaths.join(":");
    let c_rpath = CString::new(rpath_string)
        .map_err(|e| anyhow!("Invalid RPATH string: {e}"))?;

    // MODERN: We set RUNPATH (DT_RUNPATH) which is preferred over RPATH.
    patcher
        .set_dynamic_tag(DynamicTag::Runpath, c_rpath.as_c_str())
        .map_err(|e| anyhow!("Failed to set RUNPATH: {e}"))?;

    patcher
        .finish()
        .map_err(|e| anyhow!("Failed to write relocated ELF: {e}"))?;

    Ok(true)
}

/// Returns the file's current RUNPATH (or legacy RPATH), or an empty string.
fn existing_runpath(
    patcher: &mut ElfPatcher<fs::File>,
    table: &elb::DynamicTable
) -> String {
    let Some(offset) = table
        .get(DynamicTag::Runpath)
        .or_else(|| table.get(DynamicTag::Rpath))
    else {
        return String::new();
    };
    let Ok(Some(strings)) = patcher.read_dynamic_string_table() else {
        return String::new();
    };
    usize::try_from(offset)
        .ok()
        .and_then(|offset| strings.get_string(offset))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Calculates the appropriate relative RPATHs for an ELF file based on its
/// location.
fn calculate_rpaths(path: &Path, staging_dir: &Path) -> Vec<String> {
    // Determine common Zoi relative paths based on where the file is in the
    // staging area. We look for the 'pkgstore' directory which is the root
    // of the package's isolated files.
    let mut rpaths = Vec::new();
    rpaths.push("$ORIGIN".to_string());

    // Calculate depth to pkgstore to add ../lib etc.
    if let Some(rel_path) = find_rel_to_pkgstore(path, staging_dir) {
        let depth = rel_path.components().count();
        if depth > 1 {
            // If it's in a subdirectory (like bin/ or lib/), we add the
            // parent's lib dirs. e.g. bin/foo -> $ORIGIN/../lib
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

/// Finds the path of a file relative to the nearest `pkgstore` directory.
fn find_rel_to_pkgstore(
    path: &Path,
    staging_dir: &Path
) -> Option<std::path::PathBuf> {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if parent.file_name().and_then(|s| s.to_str()) == Some("pkgstore") {
            return path
                .strip_prefix(parent)
                .ok()
                .map(std::path::Path::to_path_buf);
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
    use std::path::PathBuf;

    use super::*;

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
