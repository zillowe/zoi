use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;
use zoi_core::hash::{HashAlgorithm, calculate_file_hash};
use zoi_core::types::{
    MappedDir, MappedFile, MappedSymlink, PoolFileEntry, ScopeMapping
};

/// Deduplicates and pools files from a staging directory into a central pool.
///
/// This function walks the `virtual_staging_dir`, calculates hashes for all
/// files, copies unique files to the `pool_dir`, and populates the
/// `scope_mapping` with metadata (permissions, ownership, destinations) for
/// files, directories, and symlinks.
///
/// # Errors
///
/// Returns an error if:
/// - The staging directory cannot be walked.
/// - File metadata cannot be retrieved.
/// - A file hash cannot be calculated.
/// - A unique file cannot be copied to the pool.
pub fn pool_files(
    virtual_staging_dir: &Path,
    pool_dir: &Path,
    pool: &mut BTreeMap<String, PoolFileEntry>,
    scope_mapping: &mut ScopeMapping,
    fakeroot: bool
) -> Result<()> {
    #[cfg(not(unix))]
    let _ = fakeroot;

    if !virtual_staging_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(virtual_staging_dir).min_depth(1) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(path)?;
        let rel_path = path.strip_prefix(virtual_staging_dir)?;
        let rel_path_str = rel_path.to_string_lossy().to_string();
        let normalized = rel_path_str.replace('\\', "/");

        // The top-level staging directories themselves (pkgstore, usrroot,
        // usrhome, createpkgdir) represent the destination roots and must not
        // be mapped into scope_mapping.dirs as relative paths inside
        // ${pkgstore}.
        if normalized == "pkgstore"
            || normalized == "usrroot"
            || normalized == "usrhome"
            || normalized == "createpkgdir"
        {
            continue;
        }

        // Determine destination placeholder based on top-level directory
        let (dest_prefix, dest_rel) = if let Some(stripped) =
            normalized.strip_prefix("pkgstore/")
        {
            ("${pkgstore}", stripped)
        } else if let Some(stripped) = normalized.strip_prefix("usrroot/") {
            ("${usrroot}", stripped)
        } else if let Some(stripped) = normalized.strip_prefix("usrhome/") {
            ("${usrhome}", stripped)
        } else if let Some(stripped) = normalized.strip_prefix("createpkgdir/")
        {
            ("${createpkgdir}", stripped)
        } else {
            // Fallback for direct writes to staging dir
            ("${pkgstore}", normalized.as_str())
        };

        let dest = format!("{dest_prefix}/{dest_rel}");

        #[cfg(unix)]
        let (owner, group) = {
            if fakeroot {
                (Some("root".to_string()), Some("root".to_string()))
            } else {
                use std::os::unix::fs::MetadataExt;

                use nix::unistd::{Gid, Group, Uid, User};
                let uid = metadata.uid();
                let gid = metadata.gid();
                let owner = User::from_uid(Uid::from_raw(uid))
                    .ok()
                    .flatten()
                    .map(|u| u.name);
                let group = Group::from_gid(Gid::from_raw(gid))
                    .ok()
                    .flatten()
                    .map(|g| g.name);
                (owner, group)
            }
        };
        #[cfg(not(unix))]
        let (owner, group) = (None, None);

        if metadata.is_dir() {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode() & 0o777)
            };
            #[cfg(not(unix))]
            let mode = None;

            // Outside the package store, a staged directory that merely holds
            // files (`usr`, `usr/bin`, `.config`, ...) is a shared system or
            // home directory, not something the package owns. Recording it
            // would make installation re-apply the builder's mode and owner
            // to it. Only empty directories (created on purpose) and ones
            // given a non-default mode are recorded; installation creates
            // any other parents on demand.
            if dest_prefix != "${pkgstore}"
                && fs::read_dir(path)?.next().is_some()
                && mode.is_none_or(|m| m == 0o755)
            {
                continue;
            }

            scope_mapping.dirs.push(MappedDir {
                path: dest,
                mode,
                owner,
                group
            });
        } else if metadata.is_symlink() {
            let target = fs::read_link(path)?;
            let target_str = target.to_string_lossy().to_string();

            // If target starts with virtual_staging_dir, make it relative to
            // placeholder But usually targets in zln are already
            // relative or use placeholders which we replaced.
            // This needs careful handling.

            scope_mapping.symlinks.push(MappedSymlink {
                link: dest,
                target: target_str.replace('\\', "/")
            });
        } else {
            let hash = calculate_file_hash(path, HashAlgorithm::Sha256)?;
            let hash_key = format!("sha256-{hash}");

            if !pool.contains_key(&hash_key) {
                let pool_path = pool_dir.join(&hash_key);
                if !pool_path.exists() {
                    fs::copy(path, pool_path)?;
                }
                pool.insert(
                    hash_key.clone(),
                    PoolFileEntry {
                        size: metadata.len()
                    }
                );
            }

            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o777
            };
            #[cfg(not(unix))]
            let mode = 0o644;

            scope_mapping.files.push(MappedFile {
                dest,
                hash: hash_key,
                mode,
                owner,
                group
            });
        }
    }

    Ok(())
}
