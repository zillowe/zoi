use crate::pkg::{local, lua, types};
use crate::utils::{self, copy_dir_all};
use anyhow::{Result, anyhow};
use colored::*;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::read::Decoder as ZstdDecoder;

fn get_bin_root(scope: types::Scope) -> Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(home_dir.join(".zoi/pkgs/bin"))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                Ok(PathBuf::from("/usr/local/bin"))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

fn check_and_handle_file_conflicts(source_dir: &Path, dest_dir: &Path, yes: bool) -> Result<()> {
    let mut conflicting_files = Vec::new();

    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .skip(1)
    {
        if entry.file_type().is_file() {
            let relative_path = entry.path().strip_prefix(source_dir)?;
            let dest_path = dest_dir.join(relative_path);
            if dest_path.exists() {
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

pub fn run(
    package_file: &Path,
    scope_override: Option<types::Scope>,
    registry_handle: &str,
    version_override: Option<&str>,
    yes: bool,
    sub_packages: Option<Vec<String>>,
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
        println!("Archive size: {}", crate::utils::format_bytes(file_size));
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
    let metadata = lua::parser::parse_lua_package_for_platform(
        pkg_lua_path.to_str().unwrap(),
        &platform,
        version_override,
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
                let root_dest = PathBuf::from("/");
                check_and_handle_file_conflicts(&usrroot_src, &root_dest, yes)?;
                copy_dir_all(&usrroot_src, &root_dest)?;
                for entry in WalkDir::new(&usrroot_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let dest_path = root_dest.join(entry.path().strip_prefix(&usrroot_src)?);
                        installed_files.push(dest_path.to_string_lossy().to_string());
                    }
                }
            }

            let usrhome_src = sub_data_dir.join("usrhome");
            if usrhome_src.exists() {
                let home_dest =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                check_and_handle_file_conflicts(&usrhome_src, &home_dest, yes)?;
                copy_dir_all(&usrhome_src, &home_dest)?;
                for entry in WalkDir::new(&usrhome_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let dest_path = home_dest.join(entry.path().strip_prefix(&usrhome_src)?);
                        installed_files.push(dest_path.to_string_lossy().to_string());
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
            installed_files.push(version_dir.join(rel_path).to_string_lossy().to_string());
        }
    }

    fs::create_dir_all(&version_dir)?;
    copy_dir_all(staging_dir.path(), &version_dir)?;

    if let Some(bins) = &metadata.bins {
        let bin_root = get_bin_root(scope)?;
        fs::create_dir_all(&bin_root)?;

        for bin_name in bins {
            let mut found_bin = false;
            for entry in WalkDir::new(&version_dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() && entry.file_name().to_string_lossy() == *bin_name {
                    let target_path = entry.path();
                    let link_path = bin_root.join(bin_name);

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs as unix_fs;
                        if link_path.exists() || link_path.is_symlink() {
                            fs::remove_file(&link_path)?;
                        }
                        unix_fs::symlink(target_path, &link_path)?;
                    }
                    #[cfg(windows)]
                    {
                        if link_path.exists() {
                            fs::remove_file(&link_path)?;
                        }
                        fs::copy(target_path, &link_path)?;
                    }

                    if pb.is_none() {
                        println!("Linked binary: {}", bin_name.green());
                    }
                    found_bin = true;
                    break;
                }
            }
            if !found_bin && pb.is_none() {
                eprintln!(
                    "Warning: could not find binary '{}' to link.",
                    bin_name.yellow()
                );
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
