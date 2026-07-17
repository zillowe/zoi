use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::types;
use zoi_core::utils::{self, copy_dir_all};
use zoi_resolver::local;
use zstd::stream::read::Decoder as ZstdDecoder;

fn get_bin_root(scope: types::Scope) -> Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(zoi_core::sysroot::apply_sysroot(
                home_dir.join(".zoi/pkgs/bin"),
            ))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(
                    "C:\\ProgramData\\zoi\\pkgs\\bin",
                )))
            } else {
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(
                    "/usr/local/bin",
                )))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

fn get_completions_root(scope: types::Scope, shell: &str) -> Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(zoi_core::sysroot::apply_sysroot(
                home_dir.join(".zoi/pkgs/shell").join(shell),
            ))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(zoi_core::sysroot::apply_sysroot(PathBuf::from(format!(
                    "C:\\ProgramData\\zoi\\pkgs\\shell\\{}",
                    shell
                ))))
            } else {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/local/share/zoi/completions",
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
            .map_err(|e| anyhow!("Failed to create completion symlink: {}", e))?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(source, link)
            .map_err(|e| anyhow!("Failed to create completion symlink: {}", e))?;
    }
    Ok(())
}

fn check_and_handle_file_conflicts(
    source_dir: &Path,
    dest_dir: &Path,
    owned_files: &HashSet<String>,
    yes: bool,
) -> Result<()> {
    let mut conflicting_files = Vec::new();

    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .skip(1)
    {
        if entry.file_type().is_file() {
            let relative_path = entry.path().strip_prefix(source_dir)?;
            let dest_path = dest_dir.join(relative_path);
            if dest_path.exists() && !owned_files.contains(&dest_path.to_string_lossy().to_string())
            {
                conflicting_files.push(dest_path);
            }
        }
    }

    if !conflicting_files.is_empty() {
        println!();
        println!("{}", "File Conflict Detected:".red().bold());
        println!(
            "The following files that this package wants to install already exist on your system:"
        );
        for file in &conflicting_files {
            println!("- {}", file.display());
        }
        println!();

        if !utils::ask_for_confirmation(
            "Do you want to overwrite these files and continue with the installation?",
            yes,
        ) {
            return Err(anyhow!(
                "Installation aborted by user due to file conflicts."
            ));
        }
    }

    Ok(())
}

/// Performs the low-level extraction and staging of a package archive.
///
/// Atomic Staging Pattern:
/// - The archive is unpacked into a temporary system `temp_dir`.
/// - Files are then moved into a `.tmp-install-` subdirectory within the target store.
/// - Only after ALL files are staged and shims are verified does Zoi move
///   the staging folder to its final versioned path (`{version}/`).
///
/// This ensures that a crash, power loss, or network failure during extraction
/// never leaves a partially-installed or broken package in the Zoi store.
pub fn run(
    package_file: &Path,
    scope_override: Option<types::Scope>,
    registry_handle: &str,
    version_override: Option<&str>,
    yes: bool,
    sub_packages: Option<Vec<String>>,
    link_bins: bool,
    pb: Option<&indicatif::ProgressBar>,
) -> Result<Vec<String>> {
    let scope = scope_override.unwrap_or(types::Scope::User);

    if pb.is_none() {
        println!(
            "Installing from package archive: {}",
            package_file.display()
        );
    }

    let file_metadata =
        fs::metadata(package_file).map_err(|e| anyhow!("Failed to get archive metadata: {}", e))?;
    let file_size = file_metadata.len();

    if pb.is_none() {
        println!("Archive size: {}", zoi_core::utils::format_bytes(file_size));
    }

    let mut file =
        File::open(package_file).map_err(|e| anyhow!("Failed to open package archive: {}", e))?;

    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_ok() && magic != [0x28, 0xB5, 0x2F, 0xFD] {
        return Err(anyhow!(
            "Invalid archive format: expected zstd magic number 28 B5 2F FD, but found {:02X?}. This file is likely not a valid .zst archive.",
            magic
        ));
    }
    use std::io::Seek;
    file.rewind()
        .map_err(|e| anyhow!("Failed to rewind archive file: {}", e))?;

    let decoder =
        ZstdDecoder::new(file).map_err(|e| anyhow!("Failed to initialize zstd decoder: {}", e))?;
    let mut archive = Archive::new(decoder);
    let temp_dir = Builder::new().prefix("zoi-install-").tempdir()?;
    let unpack_path = temp_dir.path().to_path_buf();

    for entry_res in archive
        .entries()
        .map_err(|e| anyhow!("Failed to read archive entries: {}", e))?
    {
        let mut entry = entry_res.map_err(|e| {
            anyhow!(
                "Failed to process archive entry: {}. The archive may be truncated or corrupted.",
                e
            )
        })?;
        let path = entry
            .path()
            .map_err(|e| anyhow!("Failed to get entry path: {}", e))?
            .to_path_buf();
        entry
            .unpack_in(&unpack_path)
            .map_err(|e| anyhow!("Failed to unpack file '{}': {}", path.display(), e))?;
    }

    let mut pkg_lua_path = None;
    for entry in WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name().to_string_lossy().ends_with(".pkg.lua") {
            pkg_lua_path = Some(entry.path().to_path_buf());
            break;
        }
    }
    let pkg_lua_path = pkg_lua_path.ok_or_else(|| {
        anyhow!(
            "Could not find .pkg.lua file in archive '{}'",
            package_file.display()
        )
    })?;

    let platform = utils::get_platform()?;
    let metadata = zoi_lua::parser::parse_lua_package_for_platform(
        pkg_lua_path
            .to_str()
            .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", pkg_lua_path))?,
        &platform,
        version_override,
        Some(scope),
        true,
    )?;
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

    let package_dir =
        local::get_package_dir(scope, registry_handle, &metadata.repo, &metadata.name)?;
    fs::create_dir_all(&package_dir)?;

    let staging_dir = tempfile::Builder::new()
        .prefix(".tmp-install-")
        .tempdir_in(&package_dir)?;

    let mut installed_files: Vec<String> = Vec::new();
    let version_dir = package_dir.join(version);

    let data_dir = temp_dir.path().join("data");
    if data_dir.exists() {
        if let Some(p) = pb {
            p.set_message("Installing package...");
        } else {
            println!("Installing package...");
        }

        let subs_to_install = if let Some(subs) = sub_packages {
            subs
        } else if let Some(subs) = &metadata.sub_packages {
            if let Some(main_subs) = &metadata.main_subs {
                main_subs.clone()
            } else {
                subs.clone()
            }
        } else {
            vec!["".to_string()]
        };

        for sub in subs_to_install {
            let sub_data_dir = if sub.is_empty() {
                data_dir.clone()
            } else {
                if pb.is_none() {
                    println!("Installing sub-package: {}", sub.bold());
                }
                data_dir.join(&sub)
            };

            if !sub_data_dir.exists() {
                if pb.is_none() {
                    eprintln!(
                        "Warning: sub-package '{}' not found in archive, skipping.",
                        sub
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
            if let Ok(Some(manifest)) = local::is_package_installed(&metadata.name, sub_opt, scope)
            {
                owned_files.extend(manifest.installed_files);
            }

            let pkgstore_src = sub_data_dir.join("pkgstore");
            if pkgstore_src.exists() {
                copy_dir_all(&pkgstore_src, staging_dir.path())?;
            }

            let usrroot_src = sub_data_dir.join("usrroot");
            if usrroot_src.exists() {
                if !utils::is_admin() {
                    return Err(anyhow!(
                        "Administrator privileges required to install system-wide files. Please run with sudo or as an administrator."
                    ));
                }
                let root_dest = zoi_core::sysroot::apply_sysroot(PathBuf::from("/"));
                check_and_handle_file_conflicts(&usrroot_src, &root_dest, &owned_files, yes)?;
                copy_dir_all(&usrroot_src, &root_dest)?;
                for entry in WalkDir::new(&usrroot_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let rel_to_root = entry.path().strip_prefix(&usrroot_src)?;
                        installed_files.push(format!(
                            "${{usrroot}}/{}",
                            rel_to_root.to_string_lossy().replace('\\', "/")
                        ));
                    }
                }
            }

            let usrhome_src = sub_data_dir.join("usrhome");
            if usrhome_src.exists() {
                let home_dest =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                check_and_handle_file_conflicts(&usrhome_src, &home_dest, &owned_files, yes)?;
                copy_dir_all(&usrhome_src, &home_dest)?;
                for entry in WalkDir::new(&usrhome_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let rel_to_home = entry.path().strip_prefix(&usrhome_src)?;
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
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel_path = entry.path().strip_prefix(staging_dir.path())?;
            installed_files.push(format!(
                "${{pkgstore}}/{}",
                rel_path.to_string_lossy().replace('\\', "/")
            ));
        }
    }

    fs::create_dir_all(&version_dir)?;
    copy_dir_all(staging_dir.path(), &version_dir)?;

    // Create .zoiorig copies for 3-way merge support
    if let Some(backup_files) = &metadata.backup {
        for backup_file_rel in backup_files {
            let backup_src = version_dir.join(backup_file_rel);
            if backup_src.exists() && backup_src.is_file() {
                let orig_path = backup_src.with_extension(format!(
                    "{}.zoiorig",
                    backup_src
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                ));
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
        let link_error: Option<String> = None;

        for bin_name in bins {
            let mut found_bin = false;
            for entry in WalkDir::new(&version_dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() && entry.file_name().to_string_lossy() == *bin_name {
                    let link_path = bin_root.join(bin_name);

                    let zoi_exe = std::env::current_exe()?;
                    zoi_core::utils::symlink_file(&zoi_exe, &link_path)
                        .map_err(|e| anyhow!("Failed to create shim: {}", e))?;
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
            return Err(anyhow!("Failed to create shims: {}", e));
        }
    }

    let shell_dir = version_dir.join("shell");
    if shell_dir.exists() {
        for shell_entry in fs::read_dir(&shell_dir)
            .map_err(|e| anyhow!("Failed to read shell completions directory: {}", e))?
        {
            let shell_entry = shell_entry?;
            if !shell_entry.file_type()?.is_dir() {
                continue;
            }
            let shell_name = shell_entry.file_name().to_string_lossy().to_string();
            let completions_root = get_completions_root(scope, &shell_name)?;
            let pkg_completions_dir = completions_root.join(&metadata.name);
            fs::create_dir_all(&pkg_completions_dir)?;

            for file_entry in fs::read_dir(shell_entry.path())
                .map_err(|e| anyhow!("Failed to read shell/{}/ directory: {}", shell_name, e))?
            {
                let file_entry = file_entry?;
                if !file_entry.file_type()?.is_file() {
                    continue;
                }
                let filename = file_entry.file_name().to_string_lossy().to_string();
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
                let home_dir =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
                home_dir.join("Applications")
            }
            types::Scope::Project => std::env::current_dir()?.join("Applications"),
        };

        let mut app_bundles = Vec::new();
        for entry in WalkDir::new(&version_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() && entry.file_name().to_string_lossy().ends_with(".app") {
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

                let app_name = app_path
                    .file_name()
                    .ok_or_else(|| anyhow!("App path has no filename: {:?}", app_path))?;
                let symlink_path = applications_dir.join(app_name);

                if symlink_path.exists() {
                    let _ = fs::remove_file(&symlink_path);
                    let _ = fs::remove_dir_all(&symlink_path);
                }

                if std::os::unix::fs::symlink(&app_path, &symlink_path).is_ok() {
                    installed_files.push(format!(
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

    if let Some(p) = pb {
        p.set_position(100);
    } else {
        println!("{} Installation complete.", "Success:".green());
    }
    Ok(installed_files)
}
