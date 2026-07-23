use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use zoi_core::hash::{HashAlgorithm, calculate_file_hash};
use zoi_core::types::{MappedDir, MappedFile, MappedSymlink, PoolFileEntry, ScopeMapping};

pub fn pool_files(
    virtual_staging_dir: &Path,
    pool_dir: &Path,
    pool: &mut BTreeMap<String, PoolFileEntry>,
    scope_mapping: &mut ScopeMapping,
    fakeroot: bool,
) -> Result<()> {
    if !virtual_staging_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(virtual_staging_dir).min_depth(1) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(path)?;
        let rel_path = path.strip_prefix(virtual_staging_dir)?;
        let rel_path_str = rel_path.to_string_lossy().to_string();

        // Determine destination placeholder based on top-level directory
        let (dest_prefix, dest_rel) = if let Some(stripped) = rel_path_str.strip_prefix("pkgstore/")
        {
            ("${pkgstore}", stripped)
        } else if let Some(stripped) = rel_path_str.strip_prefix("usrroot/") {
            ("${usrroot}", stripped)
        } else if let Some(stripped) = rel_path_str.strip_prefix("usrhome/") {
            ("${usrhome}", stripped)
        } else if let Some(stripped) = rel_path_str.strip_prefix("createpkgdir/") {
            ("${createpkgdir}", stripped)
        } else {
            // Fallback for direct writes to staging dir
            ("${pkgstore}", rel_path_str.as_str())
        };

        let dest = format!("{}/{}", dest_prefix, dest_rel.replace('\\', "/"));

        #[cfg(unix)]
        let (owner, group) = {
            if fakeroot {
                (Some("root".to_string()), Some("root".to_string()))
            } else {
                use nix::unistd::{Gid, Group, Uid, User};
                use std::os::unix::fs::MetadataExt;
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

            scope_mapping.dirs.push(MappedDir {
                path: dest,
                mode,
                owner,
                group,
            });
        } else if metadata.is_symlink() {
            let target = fs::read_link(path)?;
            let target_str = target.to_string_lossy().to_string();

            // If target starts with virtual_staging_dir, make it relative to placeholder
            // But usually targets in zln are already relative or use placeholders which we replaced.
            // This needs careful handling.

            scope_mapping.symlinks.push(MappedSymlink {
                link: dest,
                target: target_str.replace('\\', "/"),
            });
        } else {
            let hash = calculate_file_hash(path, HashAlgorithm::Sha256)?;
            let hash_key = format!("sha256-{}", hash);

            if !pool.contains_key(&hash_key) {
                let pool_path = pool_dir.join(&hash_key);
                if !pool_path.exists() {
                    fs::copy(path, pool_path)?;
                }
                pool.insert(
                    hash_key.clone(),
                    PoolFileEntry {
                        size: metadata.len(),
                    },
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
                group,
            });
        }
    }

    Ok(())
}
