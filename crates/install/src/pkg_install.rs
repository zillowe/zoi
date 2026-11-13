use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::hash::{HashAlgorithm, calculate_file_hash};
use zoi_core::types;
use zoi_core::utils::{self, copy_dir_all};
use zoi_db as db;
use zoi_resolver::local;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Returns the root directory where binary shims should be created for a given
/// scope.
fn get_bin_root(scope: types::Scope) -> Result<PathBuf> {
    match scope {
        types::Scope::User => zoi_core::utils::get_user_bin_dir(),
        types::Scope::System => Ok(zoi_core::utils::get_system_bin_dir()),
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

/// Returns the root directory where shell completions should be installed for a
/// given scope and shell.
fn get_completions_root(scope: types::Scope, shell: &str) -> Result<PathBuf> {
    match scope {
        types::Scope::User => zoi_core::utils::get_user_completions_dir(shell),
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(format!(
                    "C:\\ProgramData\\zoi\\pkgs\\shell\\{shell}"
                ))))
            } else if zoi_core::utils::is_zoios() {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/share/zoi/completions"
                };
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(base)))
            } else {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/local/share/zoi/completions"
                };
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(base)))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir
                .join(".zoi")
                .join("pkgs")
                .join("shell")
                .join(shell))
        }
    }
}

/// Creates a symlink for a shell completion file, removing any existing file or
/// link at the destination.
fn create_completion_symlink(source: &Path, link: &Path) -> Result<()> {
    if link.exists() || link.is_symlink() {
        fs::remove_file(link)?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link)
            .map_err(|e| anyhow!("Failed to create completion symlink: {e}"))?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(source, link).map_err(|e| {
            anyhow!("Failed to create completion symlink: {}", e)
        })?;
    }
    Ok(())
}

/// A destination path that an incoming file would overwrite, and who, if
/// anyone, already claims it.
struct FileConflict {
    /// The absolute path the incoming file would be written to.
    path: PathBuf,
    /// The packages that already record this path in `package_files`.
    ///
    /// Empty means the path exists on disk but no installed package claims
    /// it, which is a stray file rather than a package overlap.
    owners: Vec<String>
}

/// Identifies the package being installed, so that the ownership lookup can
/// exclude its own earlier claims.
///
/// A reinstall legitimately rewrites files the package already owns, and those
/// must not be reported as conflicts against another package. Without the
/// identity, every package would appear to conflict with itself.
struct InstallingPackage<'a> {
    /// Package name as recorded in the `packages` table.
    name: &'a str,
    /// Sub-package name for a split package, `None` otherwise.
    sub_package: Option<&'a str>,
    /// Repository tier the package was installed from.
    repo: &'a str,
    /// Registry handle the package belongs to.
    registry: &'a str,
    /// Scope the package is being installed into.
    scope: types::Scope
}

/// An incoming file destination, in both the placeholder form recorded in
/// manifests and the `package_files` index (e.g. `${usrroot}/usr/bin/zoi`)
/// and the absolute path it expands to.
struct IncomingFile {
    /// Placeholder form, as recorded in `installed_files`.
    tracked: String,
    /// Absolute filesystem path the file is written to.
    path: PathBuf
}

/// Lists the files under `source_dir` as they would land under `dest_dir`,
/// tracked under `placeholder_root` (`${usrroot}` or `${usrhome}`).
fn incoming_files_from_dir(
    source_dir: &Path,
    dest_dir: &Path,
    placeholder_root: &str
) -> Result<Vec<IncomingFile>> {
    let mut incoming = Vec::new();
    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .skip(1)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative_path = entry.path().strip_prefix(source_dir)?;
        incoming.push(IncomingFile {
            tracked: format!(
                "{placeholder_root}/{}",
                relative_path.to_string_lossy().replace('\\', "/")
            ),
            path: dest_dir.join(relative_path)
        });
    }
    Ok(incoming)
}

/// Fails unless the caller may write `${usrroot}` files.
///
/// Writing to the real system root needs administrator rights. With a
/// sysroot configured the files land in that tree instead, which the caller
/// may legitimately own, so the filesystem's own permission checks decide.
fn ensure_can_write_usrroot() -> Result<()> {
    if zoi_core::sysroot::get_sysroot().is_none() && !utils::is_admin() {
        return Err(anyhow!(
            "Administrator privileges required to install system-wide files. \
             Please run with sudo or as an administrator."
        ));
    }
    Ok(())
}

/// Checks incoming files against what already exists on disk, prompting the
/// user for confirmation if conflicts are found.
///
/// The check consults the global `package_files` index rather than only the
/// incoming package's own previous manifest. That distinction is what lets a
/// clobber be reported as a clobber: an unowned file is a stray the user may
/// reasonably want replaced, while a file another package owns is a conflict
/// between two packages that should not be resolved silently by whoever
/// installed last.
///
/// `owned_files` is the caller's existing footprint for this package. It is
/// still needed because the ownership index is only populated once a package
/// has been installed at least once, and it is also the only record for a
/// package installed before this index existed.
fn check_and_handle_file_conflicts(
    incoming: &[IncomingFile],
    owned_files: &HashSet<String>,
    package: Option<&InstallingPackage<'_>>,
    yes: bool,
    pb: Option<&indicatif::ProgressBar>
) -> Result<()> {
    let mut stray_files = Vec::new();
    let mut owned_by_others: Vec<FileConflict> = Vec::new();

    // `owned_files` holds placeholder-form paths such as
    // `${usrroot}/usr/bin/zoi` (older manifests may hold absolute ones).
    // Incoming files are compared both by their placeholder form and by
    // their absolute path against the expanded records, so either form of
    // a previous claim is recognized.
    let scope = package.map_or(types::Scope::User, |pkg| pkg.scope);
    let owned_expanded: HashSet<String> = owned_files
        .iter()
        .filter_map(|file| {
            utils::expand_placeholders(file, Path::new(""), scope).ok()
        })
        .collect();

    // Resolved once rather than per file: this runs for every incoming file,
    // and a system-scope package contributes thousands of them.
    //
    // Installed packages and their files are indexed in the `local`
    // database (keyed by the registry handle they came from), so that is
    // the one consulted here. A database that cannot be opened, or a package
    // with no row yet, both degrade to "no owners known" rather than failing
    // the install: losing the ownership index turns a cross-package clobber
    // back into the plain stray-file prompt, so the failure mode is a weaker
    // check and never a broken install.
    let (conn, current_pkg_id) = match package {
        Some(pkg) => match db::open_connection("local") {
            Ok(conn) => {
                let id = db::get_package_id_scoped(
                    &conn,
                    pkg.name,
                    pkg.sub_package,
                    pkg.repo,
                    pkg.registry,
                    Some(pkg.scope)
                )
                .ok();
                (Some(conn), id)
            }
            Err(_) => (None, None)
        },
        None => (None, None)
    };

    for file in incoming {
        if !file.path.exists() && !file.path.is_symlink() {
            continue;
        }

        let absolute = file.path.to_string_lossy().to_string();
        let is_self_owned = owned_files.contains(&file.tracked)
            || owned_files.contains(&absolute)
            || owned_expanded.contains(&absolute);
        if is_self_owned {
            continue;
        }

        // `package_files` stores the placeholder form; legacy rows may hold
        // the absolute path, so that is consulted as a fallback.
        let owners = conn
            .as_ref()
            .map(|c| {
                let mut owners =
                    db::get_other_owners(c, &file.tracked, current_pkg_id)
                        .unwrap_or_default();
                if owners.is_empty() {
                    owners = db::get_other_owners(c, &absolute, current_pkg_id)
                        .unwrap_or_default();
                }
                owners
            })
            .unwrap_or_default();

        if owners.is_empty() {
            stray_files.push(file.path.clone());
        } else {
            owned_by_others.push(FileConflict {
                path: file.path.clone(),
                owners
            });
        }
    }

    if stray_files.is_empty() && owned_by_others.is_empty() {
        return Ok(());
    }

    // The report and the question are printed with every progress bar
    // paused: installs run in parallel and their spinners would otherwise
    // redraw over both, leaving an invisible prompt waiting for an answer.
    let report_and_confirm = || -> Result<()> {
        if !stray_files.is_empty() {
            println!();
            println!("{}", "File Conflict Detected:".red().bold());
            println!(
                "The following files that this package wants to install \
                 already exist on your system:"
            );
            for file in &stray_files {
                println!("- {}", file.display());
            }
            println!();

            if !utils::ask_for_confirmation(
                "Do you want to overwrite these files and continue with the \
                 installation?",
                yes
            ) {
                return Err(anyhow!(
                    "Installation aborted by user due to file conflicts."
                ));
            }
        }

        if !owned_by_others.is_empty() {
            println!();
            println!("{}", "File Conflict With Another Package:".red().bold());
            println!(
                "The following files are owned by other installed packages. \
                 Installing this package would replace them, leaving the \
                 other package broken:"
            );
            for conflict in &owned_by_others {
                println!(
                    "- {} (owned by {})",
                    conflict.path.display(),
                    conflict.owners.join(", ")
                );
            }
            println!();

            if !utils::ask_for_confirmation(
                "Do you want to overwrite these files anyway? The packages \
                 listed above may stop working correctly.",
                yes
            ) {
                return Err(anyhow!(
                    "Installation aborted by user due to file conflicts with \
                     other packages."
                ));
            }
        }
        Ok(())
    };
    match pb {
        Some(pb) => pb.suspend(report_and_confirm),
        None => report_and_confirm()
    }
}

/// Performs the low-level extraction and staging of a package archive.
///
/// Atomic Staging Pattern:
/// - The archive is unpacked into a temporary system `temp_dir`.
/// - Files are then moved into a `.tmp-install-` subdirectory within the target
///   store.
/// - Only after ALL files are staged and shims are verified does Zoi move the
///   staging folder to its final versioned path (`{version}/`).
///
/// This ensures that a crash, power loss, or network failure during extraction
/// never leaves a partially-installed or broken package in the Zoi store.
///
/// # Errors
///
/// Returns an error if:
/// - The archive file cannot be opened or read.
/// - The archive format is invalid.
/// - Extraction fails.
/// - Metadata parsing fails.
/// - Required permissions are missing.
/// - Filesystem operations (copying, directory creation) fail.
pub fn run(
    package_file: &Path,
    scope_override: Option<types::Scope>,
    registry_handle: &str,
    version_override: Option<&str>,
    yes: bool,
    sub_packages: Option<Vec<String>>,
    link_bins: bool,
    pb: Option<&indicatif::ProgressBar>
) -> Result<(Vec<String>, BTreeMap<String, String>)> {
    let scope = scope_override.unwrap_or(types::Scope::User);
    // The sub-packages whose install manifests this run produces: the
    // installer records one manifest per requested sub-package, or the main
    // manifest when none was requested.
    let own_subs: Vec<Option<String>> = match &sub_packages {
        Some(subs) => subs
            .iter()
            .map(|s| (!s.is_empty()).then(|| s.clone()))
            .collect(),
        None => vec![None]
    };

    // Handle meta-packages with no archive
    if package_file.as_os_str().is_empty() {
        if pb.is_none() {
            println!("Initializing meta-package...");
        }
        return Ok((Vec::new(), BTreeMap::new()));
    }

    if pb.is_none() {
        println!(
            "Installing from package archive: {}",
            package_file.display()
        );
    }

    let file_metadata = fs::metadata(package_file)
        .map_err(|e| anyhow!("Failed to get archive metadata: {e}"))?;
    let file_size = file_metadata.len();

    if pb.is_none() {
        println!("Archive size: {}", zoi_core::utils::format_bytes(file_size));
    }

    let mut file = File::open(package_file)
        .map_err(|e| anyhow!("Failed to open package archive: {e}"))?;

    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_ok() && magic != [0x28, 0xB5, 0x2F, 0xFD]
    {
        return Err(anyhow!(
            "Invalid archive format: expected zstd magic number 28 B5 2F FD, \
             but found {magic:02X?}. This file is likely not a valid .zst \
             archive."
        ));
    }

    file.rewind()
        .map_err(|e| anyhow!("Failed to rewind archive file: {e}"))?;

    let decoder = ZstdDecoder::new(file)
        .map_err(|e| anyhow!("Failed to initialize zstd decoder: {e}"))?;
    let mut archive = Archive::new(decoder);

    #[cfg(target_os = "linux")]
    archive.set_unpack_xattrs(true);

    let temp_dir = Builder::new().prefix("zoi-install-").tempdir()?;
    let unpack_path = temp_dir.path().to_path_buf();

    // Later entries must never replace earlier ones. Built archives never
    // repeat a path, and allowing it would let an archive show one
    // `manifest.json` or `.pkg.lua` to the signature verifier and install
    // another (directly, or through a directory symlink).
    archive.set_overwrite(false);
    let mut trust_scan = crate::archive_trust::TrustScan::default();

    for entry_res in archive
        .entries()
        .map_err(|e| anyhow!("Failed to read archive entries: {e}"))?
    {
        let mut entry = entry_res.map_err(|e| {
            anyhow!(
                "Failed to process archive entry: {e}. The archive may be \
                 truncated or corrupted."
            )
        })?;
        let path = entry
            .path()
            .map_err(|e| anyhow!("Failed to get entry path: {e}"))?
            .to_path_buf();
        let is_regular_file = entry.header().entry_type().is_file();
        let trust_entry = crate::archive_trust::classify_entry(&path);
        entry.unpack_in(&unpack_path).map_err(|e| {
            anyhow!("Failed to unpack file '{}': {}", path.display(), e)
        })?;
        if let Some((kind, top_level)) = trust_entry {
            // Root trust entries are read back right after they are written.
            // With overwriting disabled nothing later in the stream can
            // replace them, so these are exactly the bytes the signature
            // verifier (which applies the same rules) saw.
            let data = match &top_level {
                Some(name) if is_regular_file => {
                    Some(fs::read(unpack_path.join(name))?)
                }
                _ => None
            };
            trust_scan.record(
                kind,
                top_level.as_deref(),
                is_regular_file,
                data
            )?;
        }
    }
    let trust = trust_scan.finish().map_err(|e| {
        anyhow!("Refusing to install '{}': {e}", package_file.display())
    })?;

    // The package definition is parsed (and so executed) before anything is
    // installed. When the manifest records its digest, which signed
    // manifests from current builds do, the definition must match it.
    let pkg_lua_path = if let Some((name, bytes)) = trust.single_pkg_lua()? {
        if let Some(manifest) = &trust.manifest {
            crate::archive_trust::check_pkg_lua_binding(manifest, Some(bytes))?;
        }
        unpack_path.join(name)
    } else {
        if let Some(manifest) = &trust.manifest {
            crate::archive_trust::check_pkg_lua_binding(manifest, None)?;
        }
        // Legacy archives may keep the definition below the root.
        let mut pkg_lua_path = None;
        for entry in WalkDir::new(temp_dir.path())
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if entry.file_type().is_file()
                && entry.file_name().to_string_lossy().ends_with(".pkg.lua")
            {
                pkg_lua_path = Some(entry.path().to_path_buf());
                break;
            }
        }
        pkg_lua_path.ok_or_else(|| {
            anyhow!(
                "Could not find .pkg.lua file in archive '{}'",
                package_file.display()
            )
        })?
    };

    let platform = utils::get_platform()?;
    let metadata = zoi_lua::parser::parse_lua_package_for_platform(
        pkg_lua_path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {}",
                pkg_lua_path.display()
            )
        })?,
        &platform,
        version_override,
        Some(scope),
        true
    )?;

    // Only the root `manifest.json` recorded during extraction counts; a file
    // that merely ended up at that path some other way is ignored.
    if let Some(manifest) = &trust.manifest
        && let Ok(pooled_manifest) =
            serde_json::from_slice::<types::PooledZpaManifest>(manifest)
    {
        return extract_pooled_zpa(
            &pooled_manifest,
            &unpack_path,
            scope,
            &metadata,
            sub_packages,
            &own_subs,
            link_bins,
            pb,
            yes,
            registry_handle
        );
    }

    let version = metadata.version.as_ref().ok_or_else(|| {
        anyhow!(
            "Package '{}' is missing version field in its metadata.",
            metadata.name
        )
    })?;

    if pb.is_none() {
        println!(
            "Installing package: {} v{}",
            metadata.name.cyan(),
            version.yellow()
        );
    }

    let package_dir = local::get_package_dir(
        scope,
        registry_handle,
        &metadata.repo,
        &metadata.name
    )?;
    fs::create_dir_all(&package_dir)?;

    let staging_dir = tempfile::Builder::new()
        .prefix(".tmp-install-")
        .tempdir_in(&package_dir)?;

    let mut installed_files: Vec<String> = Vec::new();
    let version_dir = package_dir.join(version);

    let data_dir = temp_dir.path().join("data");
    if data_dir.exists() {
        if let Some(p) = pb {
            p.set_message(format!("Installing {}...", metadata.name.cyan()));
        } else {
            println!("Installing {}...", metadata.name.cyan());
        }

        let subs_to_install = if let Some(subs) = sub_packages {
            subs
        } else if let Some(subs) = &metadata.sub_packages {
            if let Some(main_subs) = &metadata.main_subs {
                main_subs.clone()
            } else {
                let mut all = vec![String::new()];
                all.extend(subs.clone());
                all
            }
        } else {
            vec![String::new()]
        };

        for sub in subs_to_install {
            let sub_data_dir = if sub.is_empty() {
                data_dir.clone()
            } else {
                utils::validate_path_component(&sub)?;
                if pb.is_none() {
                    println!("Installing sub-package: {}", sub.bold());
                }
                utils::safe_join(&data_dir, Path::new(&sub))?
            };

            if !sub_data_dir.exists() {
                if pb.is_none() {
                    eprintln!(
                        "Warning: sub-package '{sub}' not found in archive, \
                         skipping."
                    );
                }
                continue;
            }

            let mut owned_files = HashSet::new();
            let sub_opt = if sub.is_empty() {
                None
            } else {
                Some(sub.as_str())
            };
            if let Ok(Some(manifest)) =
                local::is_package_installed(&metadata.name, sub_opt, scope)
            {
                owned_files.extend(manifest.installed_files);
            }

            let pkgstore_src = sub_data_dir.join("pkgstore");
            if pkgstore_src.exists() {
                copy_dir_all(&pkgstore_src, staging_dir.path())?;
            }

            // The database row this install updates is keyed by the
            // requested sub-package, not by each sub-package unpacked here.
            let installing = InstallingPackage {
                name: &metadata.name,
                sub_package: own_subs.first().and_then(Option::as_deref),
                repo: &metadata.repo,
                registry: registry_handle,
                scope
            };

            let usrroot_src = sub_data_dir.join("usrroot");
            if usrroot_src.exists() {
                ensure_can_write_usrroot()?;
                let root_dest =
                    zoi_core::sysroot::apply_sysroot(PathBuf::from("/"));
                check_and_handle_file_conflicts(
                    &incoming_files_from_dir(
                        &usrroot_src,
                        &root_dest,
                        "${usrroot}"
                    )?,
                    &owned_files,
                    Some(&installing),
                    yes,
                    pb
                )?;
                copy_dir_all(&usrroot_src, &root_dest)?;
                for entry in WalkDir::new(&usrroot_src)
                    .into_iter()
                    .filter_map(std::result::Result::ok)
                {
                    // Symlinks are recreated by copy_dir_all and must be
                    // tracked too, otherwise they survive uninstall.
                    if entry.file_type().is_file()
                        || entry.file_type().is_symlink()
                    {
                        let rel_to_root =
                            entry.path().strip_prefix(&usrroot_src)?;
                        installed_files.push(format!(
                            "${{usrroot}}/{}",
                            rel_to_root.to_string_lossy().replace('\\', "/")
                        ));
                    }
                }
            }

            let usrhome_src = sub_data_dir.join("usrhome");
            if usrhome_src.exists() {
                let home_dest = zoi_core::utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"))?;
                check_and_handle_file_conflicts(
                    &incoming_files_from_dir(
                        &usrhome_src,
                        &home_dest,
                        "${usrhome}"
                    )?,
                    &owned_files,
                    Some(&installing),
                    yes,
                    pb
                )?;
                copy_dir_all(&usrhome_src, &home_dest)?;
                for entry in WalkDir::new(&usrhome_src)
                    .into_iter()
                    .filter_map(std::result::Result::ok)
                {
                    // Track symlinks here as well so cleanup stays complete.
                    if entry.file_type().is_file()
                        || entry.file_type().is_symlink()
                    {
                        let rel_to_home =
                            entry.path().strip_prefix(&usrhome_src)?;
                        installed_files.push(format!(
                            "${{usrhome}}/{}",
                            rel_to_home.to_string_lossy().replace('\\', "/")
                        ));
                    }
                }
            }
        }
    }

    if let Some(p) = pb {
        p.set_position(60);
    }

    for entry in WalkDir::new(staging_dir.path())
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        // copy_dir_all preserves symlinks into the store, so they must be
        // recorded here as well.
        if entry.file_type().is_file() || entry.file_type().is_symlink() {
            let rel_path = entry.path().strip_prefix(staging_dir.path())?;
            installed_files.push(format!(
                "${{pkgstore}}/{}",
                rel_path.to_string_lossy().replace('\\', "/")
            ));
        }
    }

    let replaced = place_version_dir(
        staging_dir.path(),
        &version_dir,
        &own_subs,
        metadata
            .sub_packages
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    )?;
    finalize_or_restore(
        &version_dir,
        replaced,
        &metadata,
        scope,
        link_bins,
        pb,
        &mut installed_files
    )?;

    if let Some(p) = pb {
        p.set_position(100);
    } else {
        println!("{} Installation complete.", "Success:".green());
    }
    Ok((installed_files, BTreeMap::new()))
}

/// Finalizes the installation by creating backups, linking binaries,
/// and installing shell completions and application bundles.
fn finalize_installation(
    version_dir: &Path,
    metadata: &types::Package,
    scope: types::Scope,
    link_bins: bool,
    pb: Option<&indicatif::ProgressBar>,
    _installed_files: &mut Vec<String>
) -> Result<()> {
    // Create .zoiorig copies for 3-way merge support
    if let Some(backup_files) = &metadata.backup {
        for backup_file_rel in backup_files {
            let expanded_path = utils::expand_placeholders(
                backup_file_rel,
                version_dir,
                scope
            )?;
            // Backup paths in package metadata are relative to the package
            // store (version dir), so resolve them against it here.
            let backup_src = if Path::new(&expanded_path).is_absolute() {
                PathBuf::from(expanded_path)
            } else {
                version_dir.join(expanded_path)
            };

            if backup_src.exists() && backup_src.is_file() {
                let mut orig_path = backup_src.clone();
                let ext =
                    orig_path.extension().and_then(|s| s.to_str()).map_or_else(
                        || "zoiorig".to_string(),
                        |s| format!("{s}.zoiorig")
                    );
                orig_path.set_extension(ext);

                if let Err(e) = fs::copy(&backup_src, &orig_path)
                    && pb.is_none()
                {
                    eprintln!(
                        "Warning: failed to create .zoiorig for {}: {}",
                        backup_src.display(),
                        e
                    );
                }
            }
        }
    }

    if link_bins && let Some(bins) = &metadata.bins {
        let bin_root = get_bin_root(scope)?;
        fs::create_dir_all(&bin_root)?;

        let mut created_shims = Vec::new();
        let mut link_error: Option<String> = None;

        for bin_name in bins {
            let mut found_bin = false;
            for entry in WalkDir::new(version_dir)
                .into_iter()
                .filter_map(std::result::Result::ok)
            {
                if entry.file_type().is_file()
                    && entry.file_name().to_string_lossy() == *bin_name
                {
                    let link_path = bin_root.join(bin_name);

                    let zoi_exe = std::env::current_exe()?;
                    if let Err(e) =
                        zoi_core::utils::symlink_file(&zoi_exe, &link_path)
                    {
                        link_error = Some(e.to_string());
                        break;
                    }
                    created_shims.push(link_path);

                    if pb.is_none() {
                        println!("Created shim for: {}", bin_name.green());
                    }
                    found_bin = true;
                    break;
                }
            }
            if link_error.is_some() {
                break;
            }
            if !found_bin && pb.is_none() {
                eprintln!(
                    "Warning: could not find binary '{}' to link.",
                    bin_name.yellow()
                );
            }
        }

        if let Some(e) = link_error {
            for shim in created_shims {
                let _ = fs::remove_file(shim);
            }
            return Err(anyhow!("Failed to create shims: {e}"));
        }
    }

    let shell_dir = version_dir.join("shell");
    if shell_dir.exists() {
        for shell_entry in fs::read_dir(&shell_dir).map_err(|e| {
            anyhow!("Failed to read shell completions directory: {e}")
        })? {
            let shell_entry = shell_entry?;
            if !shell_entry.file_type()?.is_dir() {
                continue;
            }
            let shell_name =
                shell_entry.file_name().to_string_lossy().to_string();
            let completions_root = get_completions_root(scope, &shell_name)?;
            let pkg_completions_dir = completions_root.join(&metadata.name);
            fs::create_dir_all(&pkg_completions_dir)?;

            for file_entry in fs::read_dir(shell_entry.path()).map_err(|e| {
                anyhow!("Failed to read shell/{shell_name}/ directory: {e}")
            })? {
                let file_entry = file_entry?;
                if !file_entry.file_type()?.is_file() {
                    continue;
                }
                let filename =
                    file_entry.file_name().to_string_lossy().to_string();
                let store_path = file_entry.path();
                let link_path = pkg_completions_dir.join(&filename);
                create_completion_symlink(&store_path, &link_path)?;
                if pb.is_none() {
                    println!(
                        "Linked {} completion: {}",
                        shell_name.green(),
                        filename.cyan()
                    );
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let applications_dir = match scope {
            types::Scope::System => PathBuf::from("/Applications"),
            types::Scope::User => {
                let home_dir = zoi_core::utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory."))?;
                home_dir.join("Applications")
            }
            types::Scope::Project => {
                std::env::current_dir()?.join("Applications")
            }
        };

        let mut app_bundles = Vec::new();
        for entry in WalkDir::new(version_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir()
                && entry.file_name().to_string_lossy().ends_with(".app")
            {
                app_bundles.push(entry.path().to_path_buf());
            }
        }

        if !app_bundles.is_empty() {
            fs::create_dir_all(&applications_dir)?;
            for app_path in app_bundles {
                if zoi_core::utils::command_exists("xattr") {
                    let _ = std::process::Command::new("xattr")
                        .arg("-r")
                        .arg("-d")
                        .arg("com.apple.quarantine")
                        .arg(&app_path)
                        .status();
                }

                let app_name = app_path.file_name().ok_or_else(|| {
                    anyhow!("App path has no filename: {:?}", app_path)
                })?;
                // `file_name` already yields a single component, but the bundle
                // name comes from inside the package archive, so pin the link
                // to the applications directory explicitly rather than relying
                // on that invariant holding upstream.
                let symlink_path = zoi_core::utils::safe_join(
                    &applications_dir,
                    Path::new(app_name)
                )?;

                if symlink_path.exists() {
                    let _ = fs::remove_file(&symlink_path);
                    let _ = fs::remove_dir_all(&symlink_path);
                }

                if std::os::unix::fs::symlink(&app_path, &symlink_path).is_ok()
                {
                    _installed_files.push(format!(
                        "${{applications}}/{}",
                        app_name.to_string_lossy().replace('\\', "/")
                    ));
                    if pb.is_none() {
                        println!(
                            "Linked {} to {}",
                            app_name.to_string_lossy().green(),
                            applications_dir.display()
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extracts and installs a pooled ZPA package using its manifest
/// to map files from the pool to their final destinations.
fn extract_pooled_zpa(
    pooled_manifest: &types::PooledZpaManifest,
    unpack_path: &Path,
    scope: types::Scope,
    metadata: &types::Package,
    sub_packages: Option<Vec<String>>,
    own_subs: &[Option<String>],
    link_bins: bool,
    pb: Option<&indicatif::ProgressBar>,
    yes: bool,
    registry_handle: &str
) -> Result<(Vec<String>, BTreeMap<String, String>)> {
    let version = metadata.version.as_ref().ok_or_else(|| {
        anyhow!(
            "Package '{}' is missing version field in its metadata.",
            metadata.name
        )
    })?;

    if pb.is_none() {
        println!(
            "Installing pooled package: {} v{} [{:?}]",
            metadata.name.cyan(),
            version.yellow(),
            scope
        );
    }

    let package_dir = local::get_package_dir(
        scope,
        registry_handle,
        &metadata.repo,
        &metadata.name
    )?;
    fs::create_dir_all(&package_dir)?;

    let staging_dir = tempfile::Builder::new()
        .prefix(".tmp-install-")
        .tempdir_in(&package_dir)?;

    let mut installed_files: Vec<String> = Vec::new();
    // Content digests recorded for post-install verification (`zoi package
    // verify`). Keyed by the same tracked path that goes into
    // installed_files.
    let mut file_digests: BTreeMap<String, String> = BTreeMap::new();
    let version_dir = package_dir.join(version);

    let subs_to_install = if let Some(subs) = sub_packages {
        subs
    } else if let Some(subs) = &metadata.sub_packages {
        if let Some(main_subs) = &metadata.main_subs {
            main_subs.clone()
        } else {
            subs.clone()
        }
    } else {
        vec![String::new()]
    };

    let pool_dir = unpack_path.join("pool");

    let mut owned_files = HashSet::new();
    let mut incoming = Vec::new();
    let mut writes_usrroot = false;

    // ${createpkgdir} resolves to the invocation directory, so its absolute
    // location is recorded at install time instead of the placeholder.
    // Otherwise cleanup from a different directory could not find (or worse,
    // could mis-target) these artifacts.
    let track_dest = |dest: &str| -> Result<String> {
        if dest.starts_with("${createpkgdir}") {
            Ok(expand_pooled_path(dest, staging_dir.path(), scope)?
                .to_string_lossy()
                .to_string())
        } else {
            Ok(dest.to_string())
        }
    };

    for sub in &subs_to_install {
        let sub_opt = if sub.is_empty() {
            None
        } else {
            Some(sub.as_str())
        };
        if let Ok(Some(manifest)) =
            local::is_package_installed(&metadata.name, sub_opt, scope)
        {
            owned_files.extend(manifest.installed_files);
        }

        if let Some(sub_mapping) = pooled_manifest.mappings.get(sub)
            && let Some(scope_mapping) = sub_mapping.scopes.get(&scope)
        {
            writes_usrroot |= scope_mapping
                .files
                .iter()
                .map(|f| f.dest.as_str())
                .chain(scope_mapping.symlinks.iter().map(|l| l.link.as_str()))
                .chain(scope_mapping.dirs.iter().map(|d| d.path.as_str()))
                .any(|dest| dest.starts_with("${usrroot}"));

            for mapped_file in &scope_mapping.files {
                // Only files landing outside the Zoi store's staging area
                // can collide with anything.
                if mapped_file.dest.starts_with("${pkgstore}") {
                    continue;
                }
                incoming.push(IncomingFile {
                    tracked: track_dest(&mapped_file.dest)?,
                    path: expand_pooled_path(
                        &mapped_file.dest,
                        staging_dir.path(),
                        scope
                    )?
                });
            }
        }
    }

    // Same admission and ownership checks as the legacy `data/` layout, so
    // both archive formats behave alike.
    if writes_usrroot {
        ensure_can_write_usrroot()?;
    }
    check_and_handle_file_conflicts(
        &incoming,
        &owned_files,
        Some(&InstallingPackage {
            name: &metadata.name,
            sub_package: own_subs.first().and_then(Option::as_deref),
            repo: &metadata.repo,
            registry: registry_handle,
            scope
        }),
        yes,
        pb
    )?;

    for sub in subs_to_install {
        let Some(sub_mapping) = pooled_manifest.mappings.get(&sub) else {
            if pb.is_none() {
                eprintln!(
                    "Warning: mapping for sub-package '{sub}' not found in \
                     archive, skipping."
                );
            }
            continue;
        };

        let Some(scope_mapping) = sub_mapping.scopes.get(&scope) else {
            if pb.is_none() {
                eprintln!(
                    "Warning: mapping for scope {scope:?} not found for \
                     sub-package '{sub}', skipping."
                );
            }
            continue;
        };

        // Step 1: Create directories
        for mapped_dir in &scope_mapping.dirs {
            let dest_path = expand_pooled_path(
                &mapped_dir.path,
                staging_dir.path(),
                scope
            )?;
            // A directory that already exists (`/usr/bin`, `~/.config`, or
            // one another package created) is shared: the archive's
            // recorded mode and owner only apply to directories this install
            // creates. Older archives record every intermediate directory,
            // so this must not rely on the build having filtered them.
            let existed = fs::symlink_metadata(&dest_path).is_ok();
            fs::create_dir_all(&dest_path)?;

            #[cfg(unix)]
            {
                if !existed {
                    if let Some(mode) = mapped_dir.mode {
                        use std::os::unix::fs::PermissionsExt;
                        // setuid/setgid/sticky bits are never taken from an
                        // archive.
                        fs::set_permissions(
                            &dest_path,
                            fs::Permissions::from_mode(mode & 0o777)
                        )?;
                    }
                    if let (Some(owner), Some(group)) =
                        (&mapped_dir.owner, &mapped_dir.group)
                    {
                        let _ = utils::set_path_owner(&dest_path, owner, group);
                    }
                }
            }
            #[cfg(not(unix))]
            let _ = existed;
        }

        // Step 2: Extract files
        for mapped_file in &scope_mapping.files {
            let pool_file =
                utils::safe_join(&pool_dir, Path::new(&mapped_file.hash))?;
            if !pool_file.exists() {
                return Err(anyhow!("Pool file missing: {}", mapped_file.hash));
            }
            let expected_hash =
                mapped_file.hash.strip_prefix("sha256-").ok_or_else(|| {
                    anyhow!(
                        "Unsupported pool hash format: {}",
                        mapped_file.hash
                    )
                })?;
            let pool_entry = pooled_manifest
                .pool
                .get(&mapped_file.hash)
                .ok_or_else(|| {
                    anyhow!("Pool manifest entry missing: {}", mapped_file.hash)
                })?;
            if pool_file.metadata()?.len() != pool_entry.size {
                return Err(anyhow!(
                    "Pool file size does not match manifest: {}",
                    mapped_file.hash
                ));
            }
            let actual_hash =
                calculate_file_hash(&pool_file, HashAlgorithm::Sha256)?;
            if actual_hash != expected_hash {
                return Err(anyhow!(
                    "Pool file hash does not match manifest: {}",
                    mapped_file.hash
                ));
            }

            let dest_path = expand_pooled_path(
                &mapped_file.dest,
                staging_dir.path(),
                scope
            )?;

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Never write through a symlink sitting at the destination: that
            // would modify whatever file it points to instead of replacing
            // the link.
            if dest_path.is_symlink() {
                fs::remove_file(&dest_path)?;
            }
            fs::copy(&pool_file, &dest_path)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                // Builds record `mode & 0o777`; masking again here keeps a
                // crafted manifest from installing setuid/setgid/sticky
                // files, which Zoi packages have no way to request.
                fs::set_permissions(
                    &dest_path,
                    fs::Permissions::from_mode(mapped_file.mode & 0o777)
                )?;
                if let (Some(owner), Some(group)) =
                    (&mapped_file.owner, &mapped_file.group)
                {
                    let _ = utils::set_path_owner(&dest_path, owner, group);
                }
            }

            let tracked = track_dest(&mapped_file.dest)?;
            file_digests.insert(tracked.clone(), mapped_file.hash.clone());
            installed_files.push(tracked);
        }

        // Step 3: Create symlinks
        for mapped_link in &scope_mapping.symlinks {
            let dest_path = expand_pooled_path(
                &mapped_link.link,
                staging_dir.path(),
                scope
            )?;

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if dest_path.exists() || dest_path.is_symlink() {
                fs::remove_file(&dest_path).ok();
            }

            // Resolve target placeholders if any
            let target = mapped_link.target.clone();

            utils::symlink_file(Path::new(&target), &dest_path)?;
            installed_files.push(track_dest(&mapped_link.link)?);
        }
    }

    // Finalize staging-to-store move
    let replaced = place_version_dir(
        staging_dir.path(),
        &version_dir,
        own_subs,
        metadata
            .sub_packages
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    )?;
    finalize_or_restore(
        &version_dir,
        replaced,
        metadata,
        scope,
        link_bins,
        pb,
        &mut installed_files
    )?;

    if let Some(p) = pb {
        p.set_position(100);
    }

    Ok((installed_files, file_digests))
}

/// Runs [`finalize_installation`], putting the previous version directory
/// back if it fails after [`place_version_dir`] replaced it.
fn finalize_or_restore(
    version_dir: &Path,
    replaced: Option<ReplacedVersionDir>,
    metadata: &types::Package,
    scope: types::Scope,
    link_bins: bool,
    pb: Option<&indicatif::ProgressBar>,
    installed_files: &mut Vec<String>
) -> Result<()> {
    if let Err(e) = finalize_installation(
        version_dir,
        metadata,
        scope,
        link_bins,
        pb,
        installed_files
    ) {
        if let Some(previous) = replaced
            && let Err(restore_error) = previous.restore(version_dir)
        {
            eprintln!(
                "Warning: failed to restore the previous '{}': {restore_error}",
                version_dir.display()
            );
        }
        return Err(e);
    }
    Ok(())
}

/// The previous tree of a version directory replaced by
/// [`place_version_dir`], kept until the install completes so it can be put
/// back if a later step fails. Dropping it deletes the previous tree.
struct ReplacedVersionDir {
    /// Holds the previous tree at `holder/previous`.
    holder: tempfile::TempDir
}

impl ReplacedVersionDir {
    /// Puts the previous tree back at `version_dir`.
    fn restore(self, version_dir: &Path) -> Result<()> {
        if fs::symlink_metadata(version_dir).is_ok() {
            fs::remove_dir_all(version_dir)?;
        }
        fs::rename(self.holder.path().join("previous"), version_dir)?;
        Ok(())
    }
}

/// Returns whether a top-level version-directory file name is an install
/// manifest (current JSON or legacy YAML naming).
fn is_store_manifest_name(name: &str) -> bool {
    name.starts_with("manifest")
        && Path::new(name).extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("json")
                || ext.eq_ignore_ascii_case("yaml")
                || ext.eq_ignore_ascii_case("yml")
        })
}

/// Moves a fully staged store tree into place as `version_dir`.
///
/// A fresh install renames the staging directory into place. A reinstall of
/// the same version replaces the existing directory rather than merging into
/// it, so files the previous build shipped but this one does not cannot
/// linger. The previous tree is moved aside first and returned so the caller
/// can restore it if finishing the install fails. Zoi's own records in the
/// old directory (this package's install manifests and the persisted
/// `package.pkg.lua`) are carried over unless the new tree ships them, so
/// they stay in place until the installer rewrites them.
///
/// Split packages (`split`) share one version directory between
/// sub-packages, which may even be installed concurrently within one
/// transaction, so it is never replaced wholesale (the same applies when the
/// directory holds another sub-package's manifest). Instead this install's
/// own previously recorded store files are removed and the new tree is
/// merged in.
///
/// `own_subs` are the sub-packages whose manifests this install writes
/// (`None` for the package's main manifest).
fn place_version_dir(
    staging: &Path,
    version_dir: &Path,
    own_subs: &[Option<String>],
    split: bool
) -> Result<Option<ReplacedVersionDir>> {
    if split {
        fs::create_dir_all(version_dir)?;
    } else if fs::symlink_metadata(version_dir).is_err() {
        fs::rename(staging, version_dir)?;
        return Ok(None);
    }

    let mut own_manifests = HashSet::new();
    for sub in own_subs {
        let json = local::manifest_filename(sub.as_deref());
        let stem = json.trim_end_matches(".json").to_string();
        own_manifests.insert(format!("{stem}.yaml"));
        own_manifests.insert(format!("{stem}.yml"));
        own_manifests.insert(json);
    }

    let mut carried = Vec::new();
    let mut shared = split;
    for entry in fs::read_dir(version_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_store_manifest_name(&name) {
            if own_manifests.contains(&name) {
                carried.push(name);
            } else {
                shared = true;
            }
        } else if name == "package.pkg.lua" {
            carried.push(name);
        }
    }

    if shared {
        for name in carried.iter().filter(|n| is_store_manifest_name(n)) {
            let Ok(previous) =
                local::read_store_manifest(&version_dir.join(name))
            else {
                continue;
            };
            for file in previous.installed_files {
                if let Some(rel) = file.strip_prefix("${pkgstore}/")
                    && let Ok(path) =
                        utils::safe_join(version_dir, Path::new(rel))
                    && fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir())
                {
                    fs::remove_file(&path)?;
                }
            }
        }
        copy_dir_all(staging, version_dir)?;
        return Ok(None);
    }

    for name in &carried {
        let dest = staging.join(name);
        if fs::symlink_metadata(&dest).is_err() {
            fs::copy(version_dir.join(name), &dest)?;
        }
    }

    let parent = version_dir.parent().ok_or_else(|| {
        anyhow!("Version directory has no parent: {}", version_dir.display())
    })?;
    let holder = tempfile::Builder::new()
        .prefix(".tmp-previous-")
        .tempdir_in(parent)?;
    let previous = holder.path().join("previous");
    fs::rename(version_dir, &previous)?;
    if let Err(e) = fs::rename(staging, version_dir) {
        let _ = fs::rename(&previous, version_dir);
        return Err(anyhow!(
            "Failed to move the staged package into '{}': {e}",
            version_dir.display()
        ));
    }
    Ok(Some(ReplacedVersionDir { holder }))
}

/// Expands placeholders in a pooled path to their absolute filesystem paths.
fn expand_pooled_path(
    path: &str,
    staging_path: &Path,
    _scope: types::Scope
) -> Result<PathBuf> {
    let safe_join = |base: PathBuf, relative: &str| -> Result<PathBuf> {
        let relative_path = Path::new(relative);
        if !utils::is_safe_path(&base, relative_path) {
            return Err(anyhow!(
                "Pooled path escapes its destination root: {path}"
            ));
        }
        Ok(base.join(relative_path))
    };

    if let Some(rel) = path.strip_prefix("${pkgstore}/") {
        safe_join(staging_path.to_path_buf(), rel)
    } else if let Some(rel) = path.strip_prefix("${usrroot}/") {
        safe_join(zoi_core::sysroot::apply_sysroot(PathBuf::from("/")), rel)
    } else if let Some(rel) = path.strip_prefix("${usrhome}/") {
        let home_dir = zoi_core::utils::get_user_home()
            .ok_or_else(|| anyhow!("Home dir not found"))?;
        safe_join(home_dir, rel)
    } else if let Some(rel) = path.strip_prefix("${createpkgdir}/") {
        safe_join(std::env::current_dir()?, rel)
    } else {
        Err(anyhow!("Invalid pooled path placeholder: {path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooled_paths_should_not_escape_their_roots() {
        let staging =
            tempfile::tempdir().expect("staging directory should exist");

        assert!(
            expand_pooled_path(
                "${pkgstore}/../outside",
                staging.path(),
                types::Scope::User
            )
            .is_err()
        );
    }
}
