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
    // Kept to undo a patch that produces an invalid file (see below).
    let original = fs::read(path)?;
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

    // The patcher can lay out a new segment whose file offset and address
    // disagree modulo its alignment. Every Linux loader rejects such a file
    // ("ELF load command address/offset not page-aligned"), so a binary
    // relocated that way could not run at all. Keep the original instead.
    if !load_segments_aligned(&fs::read(path)?) {
        fs::write(path, &original)?;
        return Err(anyhow!(
            "relocation produced a misaligned load segment; left unchanged"
        ));
    }

    Ok(true)
}

/// Whether every `PT_LOAD` segment of an ELF image satisfies
/// `p_offset % p_align == p_vaddr % p_align`, which the kernel and the
/// dynamic loader require. A file whose headers cannot be read counts as
/// invalid.
fn load_segments_aligned(data: &[u8]) -> bool {
    /// Program header type of a loadable segment.
    const PT_LOAD: u32 = 1;
    let read = |at: usize, len: usize| -> Option<u64> {
        let bytes = data.get(at..at.checked_add(len)?)?;
        let big_endian = data.get(5) == Some(&2);
        let mut value = 0u64;
        for i in 0..len {
            let byte = if big_endian {
                bytes.get(i)
            } else {
                bytes.get(len - 1 - i)
            };
            value = (value << 8) | u64::from(*byte?);
        }
        Some(value)
    };
    let check = || -> Option<bool> {
        let is_64 = match data.get(4)? {
            1 => false,
            2 => true,
            _ => return None
        };
        let (ph_offset, base) = if is_64 {
            (read(32, 8)?, 54)
        } else {
            (read(28, 4)?, 42)
        };
        let entry_size = usize::try_from(read(base, 2)?).ok()?;
        let count = usize::try_from(read(base + 2, 2)?).ok()?;
        let ph_offset = usize::try_from(ph_offset).ok()?;
        for i in 0..count {
            let at = ph_offset.checked_add(i.checked_mul(entry_size)?)?;
            if read(at, 4)? != u64::from(PT_LOAD) {
                continue;
            }
            let (offset, vaddr, align) = if is_64 {
                (read(at + 8, 8)?, read(at + 16, 8)?, read(at + 48, 8)?)
            } else {
                (read(at + 4, 4)?, read(at + 8, 4)?, read(at + 28, 4)?)
            };
            if align > 1 && offset % align != vaddr % align {
                return Some(false);
            }
        }
        Some(true)
    };
    check().unwrap_or(false)
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

    /// A little-endian ELF64 image with one `PT_LOAD` segment at `offset`
    /// mapped to `vaddr`, page-aligned.
    fn elf_with_load(offset: u64, vaddr: u64) -> Vec<u8> {
        let mut header = Vec::with_capacity(64);
        header.extend_from_slice(b"\x7fELF");
        header.extend_from_slice(&[2, 1, 1, 0]);
        header.resize(32, 0);
        header.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
        header.resize(54, 0);
        header.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize
        header.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
        header.resize(64, 0);

        header.extend_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        header.extend_from_slice(&0u32.to_le_bytes()); // p_flags
        header.extend_from_slice(&offset.to_le_bytes());
        header.extend_from_slice(&vaddr.to_le_bytes());
        header.extend_from_slice(&vaddr.to_le_bytes()); // p_paddr
        header.extend_from_slice(&0u64.to_le_bytes()); // p_filesz
        header.extend_from_slice(&0u64.to_le_bytes()); // p_memsz
        header.extend_from_slice(&0x1000u64.to_le_bytes()); // p_align
        header
    }

    #[test]
    fn aligned_load_segments_are_accepted() {
        assert!(load_segments_aligned(&elf_with_load(0x2000, 0x5000)));
        assert!(load_segments_aligned(&elf_with_load(0x1220, 0x4220)));
    }

    #[test]
    fn misaligned_load_segments_are_rejected() {
        // The layout the patcher produced for Zoidberg's `xz`.
        assert!(!load_segments_aligned(&elf_with_load(0x1b1c0, 0x21000)));
        assert!(!load_segments_aligned(b"\x7fELF"));
    }

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
