use crate::pkg::{
    install::{
        util::{download_file_with_progress, get_filename_from_url},
        verification::{verify_checksum, verify_signatures},
    },
    library, types,
};
use crate::utils;
use anyhow::Result;
use colored::*;
use flate2::read::GzDecoder;
use std::error::Error;
use std::fs;
use std::io::Cursor;
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn handle_com_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let platform = utils::get_platform()?;
    let target_os = platform.split('-').next().unwrap_or("");
    let os = std::env::consts::OS;

    let com_ext = method
        .platform_com_ext
        .as_ref()
        .and_then(|ext_map| ext_map.get(os))
        .map(|s| s.as_str())
        .unwrap_or(if os == "windows" { "zip" } else { "tar.zst" });

    let url = &method.url;

    let downloaded_bytes = download_file_with_progress(url)?;

    let file_to_verify = get_filename_from_url(url);
    verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

    let temp_dir = Builder::new().prefix("zoi-com-binary").tempdir()?;

    if com_ext == "zip" {
        let mut archive = ZipArchive::new(Cursor::new(downloaded_bytes))?;
        archive.extract(temp_dir.path())?;
    } else if com_ext == "tar.zst" {
        let tar = ZstdDecoder::new(Cursor::new(downloaded_bytes))?;
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else if com_ext == "tar.xz" {
        let tar = XzDecoder::new(Cursor::new(downloaded_bytes));
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else if com_ext == "tar.gz" {
        let tar = GzDecoder::new(Cursor::new(downloaded_bytes));
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else {
        return Err(format!("Unsupported compression format: {}", com_ext).into());
    }

    let store_dir = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name)
        .join("bin");
    fs::create_dir_all(&store_dir)?;
    let mut dest_filename = pkg.name.clone();
    if let Some(bp) = &method.binary_path
        && bp.ends_with(".exe")
    {
        dest_filename = format!("{}.exe", pkg.name);
    }
    let mut bin_path = store_dir.join(&dest_filename);

    if pkg.package_type == types::PackageType::Library {
        library::install_files(temp_dir.path(), pkg)?;
        println!("{}", "Library files installed successfully.".green());
        return Ok(());
    }

    let binary_name = &pkg.name;
    let binary_name_with_ext = format!("{}.exe", pkg.name);
    let declared_binary_path_normalized: Option<String> = method.binary_path.as_ref().map(|bp| {
        if target_os == "windows" && !bp.ends_with(".exe") {
            format!("{bp}.exe")
        } else {
            bp.clone()
        }
    });
    let declared_binary_path = declared_binary_path_normalized.as_deref();
    let mut found_binary_path = None;
    let mut files_in_archive = Vec::new();

    for entry in WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        files_in_archive.push(path.to_path_buf());
        if let Some(bp) = declared_binary_path {
            let rel = path
                .strip_prefix(temp_dir.path())
                .unwrap_or(path)
                .to_path_buf();
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let bp_norm = bp.replace('\\', "/");
            let file_name = path.file_name().and_then(|o| o.to_str()).unwrap_or("");
            let mut matched = rel_str == bp_norm;
            if !matched && !bp_norm.contains('/') {
                matched = file_name == bp_norm;
                if !matched && bp_norm == binary_name.as_str() {
                    matched = file_name == binary_name_with_ext.as_str();
                }
            }
            if matched {
                found_binary_path = Some(path.to_path_buf());
            }
        } else {
            let file_name = path.file_name().unwrap_or_default();
            if file_name == binary_name.as_str()
                || (target_os == "windows" && file_name == binary_name_with_ext.as_str())
            {
                found_binary_path = Some(path.to_path_buf());
            }
        }
    }

    if let Some(found_path) = found_binary_path {
        if found_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".exe"))
            .unwrap_or(false)
        {
            dest_filename = format!("{}.exe", pkg.name);
            bin_path = store_dir.join(&dest_filename);
        }
        fs::copy(found_path, &bin_path)?;
    } else if files_in_archive.len() == 1 {
        println!(
            "{}",
            "Could not find binary by package name. Found one file, assuming it's the correct one."
                .yellow()
        );
        let only = &files_in_archive[0];
        if only
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".exe"))
            .unwrap_or(false)
        {
            dest_filename = format!("{}.exe", pkg.name);
            bin_path = store_dir.join(&dest_filename);
        }
        fs::copy(only, &bin_path)?;
    } else {
        eprintln!(
            "Error: Could not find binary '{}' in the extracted archive.",
            binary_name
        );
        eprintln!("Listing contents of the extracted archive:");
        for path in files_in_archive {
            eprintln!("- {}", path.display());
        }
        return Err(format!(
            "Could not find binary '{}' in the extracted archive.",
            binary_name
        )
        .into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
    }

    #[cfg(unix)]
    {
        let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
        fs::create_dir_all(&symlink_dir)?;
        let symlink_path = symlink_dir.join(&pkg.name);

        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        std::os::unix::fs::symlink(&bin_path, symlink_path)?;
    }

    #[cfg(windows)]
    {
        println!(
            "{}",
            "Binary installed. Please add ~/.zoi/pkgs/bin to your PATH manually.".yellow()
        );
    }

    println!("{}", "Compressed binary installed successfully.".green());
    Ok(())
}
