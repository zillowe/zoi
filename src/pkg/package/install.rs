use super::structs::FinalMetadata;
use crate::pkg::{local, types};
use crate::utils;
use chrono::Utc;
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

pub fn run(
    package_file: &Path,
    scope_override: Option<types::Scope>,
) -> Result<(), Box<dyn Error>> {
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

    let metadata_path = temp_dir.path().join("metadata.json");
    if !metadata_path.exists() {
        return Err("metadata.json not found in package archive.".into());
    }
    let metadata_content = fs::read_to_string(metadata_path)?;
    let metadata: FinalMetadata = serde_json::from_str(&metadata_content)?;

    println!(
        "Installing package: {} v{}",
        metadata.name.cyan(),
        metadata.version.yellow()
    );

    let registry_handle = "local";
    let version_dir = local::get_package_version_dir(
        scope,
        registry_handle,
        &metadata.repo,
        &metadata.name,
        &metadata.version,
    )?;

    if version_dir.exists() {
        println!(
            "Removing existing installation at version {}...",
            metadata.version
        );
        fs::remove_dir_all(&version_dir)?;
    }
    fs::create_dir_all(&version_dir)?;

    let mut installed_files: Vec<String> = Vec::new();

    let data_dir = temp_dir.path().join("data");
    if data_dir.exists() {
        println!("Copying package files...");
        for entry in WalkDir::new(&data_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .skip(1)
        {
            let dest_path = version_dir.join(entry.path().strip_prefix(&data_dir)?);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                fs::copy(entry.path(), &dest_path)?;
                installed_files.push(dest_path.to_string_lossy().to_string());
            }
        }
    }

    let man_md_path = temp_dir.path().join("man.md");
    if man_md_path.exists() {
        let dest = version_dir.join("man.md");
        fs::copy(man_md_path, &dest)?;
        installed_files.push(dest.to_string_lossy().to_string());
        println!("Installed manual (man.md).");
    }

    let man_txt_path = temp_dir.path().join("man.txt");
    if man_txt_path.exists() {
        let dest = version_dir.join("man.txt");
        fs::copy(man_txt_path, &dest)?;
        installed_files.push(dest.to_string_lossy().to_string());
        println!("Installed manual (man.txt).");
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

    let files_dir = temp_dir.path().join("files");
    if files_dir.exists()
        && let Some(file_groups) = &metadata.installation.files
    {
        println!("Copying additional files...");
        let platform = crate::utils::get_platform()?;

        for group in file_groups {
            if crate::utils::is_platform_compatible(&platform, &group.platforms) {
                for file_copy in &group.files {
                    let source_in_archive = files_dir.join(&file_copy.source);

                    if !source_in_archive.exists() {
                        eprintln!(
                            "{} File specified in metadata not found in archive: {}",
                            "Warning:".yellow(),
                            source_in_archive.display()
                        );
                        continue;
                    }

                    let mut dest_path_str = file_copy.destination.clone();
                    if dest_path_str.starts_with("~/") {
                        if scope == types::Scope::System {
                            eprintln!(
                                "{} Cannot use home directory ('~') destination for a system-wide package install. Skipping '{}'",
                                "Warning:".yellow(),
                                file_copy.destination
                            );
                            continue;
                        }
                        if let Some(home) = home::home_dir() {
                            dest_path_str = dest_path_str.replacen("~/", home.to_str().unwrap(), 1);
                        } else {
                            eprintln!(
                                "{} Could not determine home directory. Skipping '{}'",
                                "Warning:".yellow(),
                                file_copy.destination
                            );
                            continue;
                        }
                    }

                    let dest_path = PathBuf::from(&dest_path_str);

                    let home = home::home_dir();
                    let is_system_path = if let Some(ref h) = home {
                        !dest_path.starts_with(h)
                    } else {
                        true
                    };

                    if is_system_path && !utils::is_admin() {
                        return Err(format!("Administrator privileges required to write to {}. Please run with sudo or as an administrator.", dest_path.display()).into());
                    }

                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if source_in_archive.is_dir() {
                        fs::create_dir_all(&dest_path)?;
                        for entry in WalkDir::new(&source_in_archive)
                            .into_iter()
                            .filter_map(|e| e.ok())
                        {
                            let target_path =
                                dest_path.join(entry.path().strip_prefix(&source_in_archive)?);
                            if entry.file_type().is_dir() {
                                fs::create_dir_all(&target_path)?;
                            } else {
                                fs::copy(entry.path(), &target_path)?;
                                installed_files.push(target_path.to_string_lossy().to_string());
                            }
                        }
                    } else {
                        fs::copy(&source_in_archive, &dest_path)?;
                        installed_files.push(dest_path.to_string_lossy().to_string());
                    }
                    println!(
                        "Copied {} to {}",
                        file_copy.source.cyan(),
                        dest_path.display()
                    );
                }
            }
        }
    }

    let manifest = types::InstallManifest {
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        repo: metadata.repo.clone(),
        registry_handle: registry_handle.to_string(),
        package_type: types::PackageType::Package,
        installed_at: Utc::now().to_rfc3339(),
        reason: types::InstallReason::Direct,
        scope,
        bins: metadata.bins.clone(),
        conflicts: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("prebuilt-archive".to_string()),
        installed_files,
    };

    local::write_manifest(&manifest)?;

    println!("{} Installation complete.", "Success:".green());
    Ok(())
}
