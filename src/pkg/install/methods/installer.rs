use crate::pkg::{
    install::{
        util::{download_file_with_progress, get_filename_from_url},
        verification::{verify_checksum, verify_signatures},
    },
    types,
};
use anyhow::Result;
use colored::*;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::Builder;

pub fn handle_installer_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
    installed_files: &mut Vec<String>,
    version_dir: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let os = std::env::consts::OS;

    let installer_type = if os == "macos" {
        "dmg"
    } else if os == "windows" {
        "msi"
    } else if os == "linux" {
        "appimage"
    } else {
        return Err(format!("Installer method not supported on this OS: {}", os).into());
    };

    if pkg.package_type == types::PackageType::Library {
        return Err(format!(
            "{} installers are not supported for libraries.",
            installer_type.to_uppercase()
        )
        .into());
    }
    let url_string;
    let url: &str = if !method.url.ends_with(installer_type) {
        url_string = format!("{}.{}", &method.url, installer_type);
        &url_string
    } else {
        &method.url
    };

    let downloaded_bytes = download_file_with_progress(url)?;

    let file_to_verify = get_filename_from_url(url);
    verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

    let temp_dir = Builder::new()
        .prefix(&format!("zoi-install-{}", pkg.name))
        .tempdir()?;
    let file_name = get_filename_from_url(url);
    let temp_file_path = temp_dir.path().join(file_name);
    fs::write(&temp_file_path, &downloaded_bytes)?;

    println!("Installing {}...", file_name.cyan());

    if installer_type == "dmg" {
        let output = Command::new("hdiutil")
            .arg("attach")
            .arg(&temp_file_path)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "Failed to mount DMG: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mount_path_line = stdout.lines().last().unwrap_or("");
        let mount_path_parts: Vec<&str> = mount_path_line.split('\t').collect();
        let mount_path = PathBuf::from(mount_path_parts.last().unwrap_or(&"").trim());

        if mount_path.as_os_str().is_empty() {
            return Err("Could not determine mount path for DMG.".into());
        }

        let app_path = fs::read_dir(&mount_path)?
            .filter_map(Result::ok)
            .find(|entry| entry.path().extension().is_some_and(|ext| ext == "app"))
            .map(|entry| entry.path());

        if let Some(app_path) = app_path {
            let app_name = app_path.file_name().unwrap().to_str().unwrap();
            let app_dest_dir = if pkg.scope == types::Scope::System {
                PathBuf::from("/Applications")
            } else {
                home::home_dir().ok_or("No home dir")?.join("Applications")
            };
            fs::create_dir_all(&app_dest_dir)?;
            let dest_path = app_dest_dir.join(app_name);

            println!(
                "Copying {} to {}...",
                app_name.cyan(),
                app_dest_dir.display()
            );
            let cp_status = Command::new("cp")
                .arg("-R")
                .arg(&app_path)
                .arg(&app_dest_dir)
                .status()?;
            if !cp_status.success() {
                return Err("Failed to copy .app from DMG.".into());
            }

            installed_files.push(dest_path.to_str().unwrap().to_string());

            let bin_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&bin_dir)?;
            let symlink_path = bin_dir.join(&pkg.name);

            let executable_name = app_path.file_stem().unwrap().to_str().unwrap();
            let app_executable = dest_path.join("Contents/MacOS").join(executable_name);

            if app_executable.exists() {
                if symlink_path.exists() {
                    fs::remove_file(&symlink_path)?;
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(&app_executable, &symlink_path)?;
            } else {
                println!(
                    "{} Could not find executable inside .app bundle to create a symlink.",
                    "Warning:".yellow()
                );
            }
        } else {
            return Err("Could not find an .app file in the mounted DMG.".into());
        }

        Command::new("hdiutil")
            .arg("detach")
            .arg(&mount_path)
            .status()?;
    } else if installer_type == "msi" {
        let store_dir = version_dir;
        fs::create_dir_all(store_dir)?;
        let msi_path = store_dir.join(file_name);
        fs::copy(&temp_file_path, &msi_path)?;
        installed_files.push(msi_path.to_str().unwrap().to_string());

        let status = Command::new("msiexec")
            .arg("/i")
            .arg(&temp_file_path)
            .arg("/qn")
            .status()?;
        if !status.success() {
            return Err("Failed to run MSI installer.".into());
        }
    } else if installer_type == "appimage" {
        let store_dir = version_dir.join("bin");
        fs::create_dir_all(&store_dir)?;

        let bin_path = store_dir.join(&pkg.name);
        fs::copy(&temp_file_path, &bin_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
        }

        let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
        fs::create_dir_all(&symlink_dir)?;
        let symlink_path = symlink_dir.join(&pkg.name);

        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&bin_path, symlink_path)?;
    }

    println!("{}", "Installer finished successfully.".green());
    Ok(())
}
