use crate::pkg::{local, lua, types};
use crate::utils;
use colored::*;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::read::Decoder as ZstdDecoder;

fn get_bin_root(scope: types::Scope) -> Result<PathBuf, Box<dyn Error>> {
    match scope {
        types::Scope::User => {
            let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
            Ok(home_dir.join(".zoi/pkgs/bin"))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                Ok(PathBuf::from("/usr/local/bin"))
            }
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn run(
    package_file: &Path,
    scope_override: Option<types::Scope>,
    registry_handle: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let scope = scope_override.unwrap_or(types::Scope::User);

    println!(
        "Installing from package archive: {}",
        package_file.display()
    );

    let file = File::open(package_file)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);
    let temp_dir = Builder::new().prefix("zoi-install-").tempdir()?;
    archive.unpack(temp_dir.path())?;

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
    let pkg_lua_path = pkg_lua_path.ok_or("Could not find .pkg.lua file in archive")?;

    let platform = utils::get_platform()?;
    let metadata = lua::parser::parse_lua_package_for_platform(
        pkg_lua_path.to_str().unwrap(),
        &platform,
        None,
    )?;
    let version = metadata
        .version
        .as_ref()
        .ok_or("Package is missing version")?;

    println!(
        "Installing package: {} v{}",
        metadata.name.cyan(),
        version.yellow()
    );

    let version_dir = local::get_package_version_dir(
        scope,
        registry_handle,
        &metadata.repo,
        &metadata.name,
        version,
    )?;

    if version_dir.exists() {
        println!("Removing existing installation at version {}...", version);
        fs::remove_dir_all(&version_dir)?;
    }
    fs::create_dir_all(&version_dir)?;

    let mut installed_files: Vec<String> = Vec::new();

    let data_dir = temp_dir.path().join("data");
    if data_dir.exists() {
        println!("Copying package files...");

        let pkgstore_src = data_dir.join("pkgstore");
        if pkgstore_src.exists() {
            copy_dir_all(&pkgstore_src, &version_dir)?;
            for entry in WalkDir::new(version_dir.clone())
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    installed_files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }

        let usrroot_src = data_dir.join("usrroot");
        if usrroot_src.exists() {
            if !utils::is_admin() {
                return Err("Administrator privileges required to install system-wide files. Please run with sudo or as an administrator.".into());
            }
            let root_dest = PathBuf::from("/");
            copy_dir_all(&usrroot_src, &root_dest)?;
            for entry in WalkDir::new(&usrroot_src)
                .into_iter()
                .filter_map(|e| e.ok())
                .skip(1)
            {
                let dest_path = root_dest.join(entry.path().strip_prefix(&usrroot_src)?);
                if dest_path.is_file() {
                    installed_files.push(dest_path.to_string_lossy().to_string());
                }
            }
        }

        let usrhome_src = data_dir.join("usrhome");
        if usrhome_src.exists() {
            let home_dest = home::home_dir().ok_or("Could not find home directory")?;
            copy_dir_all(&usrhome_src, &home_dest)?;
            for entry in WalkDir::new(&usrhome_src)
                .into_iter()
                .filter_map(|e| e.ok())
                .skip(1)
            {
                let dest_path = home_dest.join(entry.path().strip_prefix(&usrhome_src)?);
                if dest_path.is_file() {
                    installed_files.push(dest_path.to_string_lossy().to_string());
                }
            }
        }
    }

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

                    println!("Linked binary: {}", bin_name.green());
                    found_bin = true;
                    break;
                }
            }
            if !found_bin {
                eprintln!(
                    "Warning: could not find binary '{}' to link.",
                    bin_name.yellow()
                );
            }
        }
    }

    println!("{} File installation complete.", "Success:".green());
    Ok(installed_files)
}
