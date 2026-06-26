use colored::*;
use mlua::{self, Lua, Table};
use std::path::{Path, PathBuf};
use zoi_core::utils;

use ar::Archive as ArArchive;
use flate2::read::GzDecoder;
use sevenz_rust;
use std::fs;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;
pub fn add_extract_util(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let extract_fn =
        lua.create_function(move |lua, (source, out_name): (String, Option<String>)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
            let build_dir = Path::new(&build_dir_str);

            let archive_file = if source.starts_with("http") {
                if source.starts_with("http://") && !quiet {
                    println!("{}: downloading over insecure HTTP: {}", "Warning:".yellow(), source);
                }
                if !quiet {
                    println!("Downloading: {}", source);
                }
                let file_name = source.split('/').next_back().unwrap_or("download.tmp");
                let temp_path = build_dir.join(file_name);
                let client = utils::get_http_client()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let mut attempt = 0u32;
                let mut response = loop {
                    attempt += 1;
                    match client.get(&source).send() {
                        Ok(resp) => break resp,
                        Err(e) => {
                            if attempt < 3 {
                                if !quiet {
                                    eprintln!("Download failed ({}). Retrying...", e);
                                }
                                zoi_core::utils::retry_backoff_sleep(attempt);
                                continue;
                            } else {
                                return Err(mlua::Error::RuntimeError(e.to_string()));
                            }
                        }
                    }
                };

                if !response.status().is_success() {
                    return Err(mlua::Error::RuntimeError(format!("Failed to download {}: {}", source, response.status())));
                }

                let mut temp_file = fs::File::create(&temp_path)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                std::io::copy(&mut response, &mut temp_file)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

                temp_path
            } else {
                PathBuf::from(source)
            };

            let out_dir_name = out_name.unwrap_or_else(|| "extracted".to_string());
            let out_dir = build_dir.join(&out_dir_name);

            if !out_dir.starts_with(build_dir) || out_dir == build_dir {
                return Err(mlua::Error::RuntimeError(format!(
                    "Invalid output directory: {}. Extraction must be into a subdirectory of the build directory.",
                    out_dir_name
                )));
            }

            fs::create_dir_all(&out_dir).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

            if !quiet {
                println!(
                    "Extracting {} to {}",
                    archive_file.display(),
                    out_dir.display()
                );
            }

            let file = fs::File::open(&archive_file)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

            let archive_path_str = archive_file.to_string_lossy();

            if archive_path_str.ends_with(".zip") {
                let mut archive =
                    ZipArchive::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                archive
                    .extract(&out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            } else if archive_path_str.ends_with(".tar.gz") || archive_path_str.ends_with(".tgz") {
                let tar_gz = GzDecoder::new(file);
                let mut archive = tar::Archive::new(tar_gz);
                archive
                    .unpack(&out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            } else if archive_path_str.ends_with(".tar.zst") {
                let tar_zst =
                    ZstdDecoder::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let mut archive = tar::Archive::new(tar_zst);
                archive
                    .unpack(&out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            } else if archive_path_str.ends_with(".tar.xz") {
                let tar_xz = XzDecoder::new(file);
                let mut archive = tar::Archive::new(tar_xz);
                archive
                    .unpack(&out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            } else if archive_path_str.ends_with(".7z") {
                sevenz_rust::decompress_file(&archive_file, &out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            } else if archive_path_str.ends_with(".dmg") {
                if !cfg!(target_os = "macos") {
                    return Err(mlua::Error::RuntimeError(
                        "Extracting .dmg files is only supported on macOS.".to_string(),
                    ));
                }
                let output = std::process::Command::new("hdiutil")
                    .arg("attach")
                    .arg("-nobrowse")
                    .arg("-readonly")
                    .arg(&archive_file)
                    .output()
                    .map_err(|e| mlua::Error::RuntimeError(format!("Failed to execute hdiutil: {}", e)))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(mlua::Error::RuntimeError(format!("hdiutil failed: {}", stderr)));
                }
                let output_str = String::from_utf8_lossy(&output.stdout);
                let mut mount_point = None;
                for line in output_str.lines() {
                    if line.contains("/Volumes/")
                        && let Some(idx) = line.find("/Volumes/") {
                            mount_point = Some(line[idx..].trim().to_string());
                            break;
                        }
                }
                let mount_point = mount_point.ok_or_else(|| {
                    mlua::Error::RuntimeError("Failed to parse mount point from hdiutil output.".to_string())
                })?;
                let mount_path = std::path::Path::new(&mount_point);
                if let Err(e) = zoi_core::utils::copy_dir_all(mount_path, &out_dir) {
                    let _ = std::process::Command::new("hdiutil").arg("detach").arg(&mount_point).status();
                    return Err(mlua::Error::RuntimeError(format!("Failed to copy contents from dmg: {}", e)));
                }
                let detach_status = std::process::Command::new("hdiutil")
                    .arg("detach")
                    .arg(&mount_point)
                    .status()
                    .map_err(|e| mlua::Error::RuntimeError(format!("Failed to execute hdiutil detach: {}", e)))?;
                if !detach_status.success() {
                    eprintln!("Warning: failed to detach dmg volume at {}", mount_point);
                }
            } else if archive_path_str.ends_with(".pkg") {
                if !cfg!(target_os = "macos") {
                    return Err(mlua::Error::RuntimeError(
                        "Extracting .pkg files natively is only supported on macOS.".to_string(),
                    ));
                }
                let temp_extract_dir = out_dir.join(".pkg_extract_tmp");
                let status = std::process::Command::new("pkgutil")
                    .arg("--expand-full")
                    .arg(&archive_file)
                    .arg(&temp_extract_dir)
                    .status()
                    .map_err(|e| mlua::Error::RuntimeError(format!("Failed to execute pkgutil: {}", e)))?;
                if !status.success() {
                    return Err(mlua::Error::RuntimeError("pkgutil failed to expand the package.".to_string()));
                }
                zoi_core::utils::copy_dir_all(&temp_extract_dir, &out_dir)
                    .map_err(|e| mlua::Error::RuntimeError(format!("Failed to copy pkg contents: {}", e)))?;
                let _ = fs::remove_dir_all(&temp_extract_dir);

            } else if archive_path_str.ends_with(".rar") {
                if zoi_core::utils::command_exists("unrar") {
                    let status = std::process::Command::new("unrar")
                        .arg("x")
                        .arg("-y")
                        .arg(&archive_file)
                        .arg(&out_dir)
                        .status()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    if !status.success() {
                        return Err(mlua::Error::RuntimeError("unrar failed".to_string()));
                    }
                } else {
                    return Err(mlua::Error::RuntimeError(
                        "unrar command not found. Please install unrar to extract .rar files."
                            .to_string(),
                    ));
                }
            } else if archive_path_str.ends_with(".deb") {
                let mut ar = ArArchive::new(file);
                while let Some(entry_result) = ar.next_entry() {
                    let mut entry =
                        entry_result.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    let name = String::from_utf8_lossy(entry.header().identifier())
                        .trim()
                        .trim_end_matches('/')
                        .to_string();
                    if name.starts_with("data.tar") {
                        let temp_data_path = build_dir.join(&name);
                        let mut temp_file = fs::File::create(&temp_data_path)
                            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to create temp file for {}: {}", name, e)))?;
                        std::io::copy(&mut entry, &mut temp_file)
                            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to copy entry data for {}: {}", name, e)))?;

                        let data_file = fs::File::open(&temp_data_path)
                            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to reopen temp file for {}: {}", name, e)))?;
                        if name.ends_with(".gz") {
                            let mut archive = tar::Archive::new(GzDecoder::new(data_file));
                            archive
                                .unpack(&out_dir)
                                .map_err(|e| mlua::Error::RuntimeError(format!("Failed to unpack {}: {}", name, e)))?;
                        } else if name.ends_with(".xz") {
                            let mut archive = tar::Archive::new(XzDecoder::new(data_file));
                            archive
                                .unpack(&out_dir)
                                .map_err(|e| mlua::Error::RuntimeError(format!("Failed to unpack {}: {}", name, e)))?;
                        } else if name.ends_with(".zst") {
                            let mut archive = tar::Archive::new(
                                ZstdDecoder::new(data_file)
                                    .map_err(|e| mlua::Error::RuntimeError(format!("Failed to initialize zstd for {}: {}", name, e)))?,
                            );
                            archive
                                .unpack(&out_dir)
                                .map_err(|e| mlua::Error::RuntimeError(format!("Failed to unpack {}: {}", name, e)))?;
                        }
                        fs::remove_file(temp_data_path).ok();
                    }
                }
            } else {
                return Err(mlua::Error::RuntimeError(format!(
                    "Unsupported archive format for file: {}",
                    archive_path_str
                )));
            }

            Ok(())
        })?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("EXTRACT", extract_fn)?;

    Ok(())
}

pub fn add_archive_util(lua: &Lua) -> Result<(), mlua::Error> {
    let archive_table = lua.create_table()?;

    let list_fn = lua.create_function(|lua, path: String| {
        let p = Path::new(&path);
        let actual_path = if p.exists() {
            p.to_path_buf()
        } else if let Ok(build_dir) = lua.globals().get::<String>("BUILD_DIR") {
            Path::new(&build_dir).join(p)
        } else {
            p.to_path_buf()
        };

        let file = fs::File::open(&actual_path).map_err(|e| {
            mlua::Error::RuntimeError(format!("Failed to open archive {:?}: {}", actual_path, e))
        })?;
        let mut files = Vec::new();

        if path.ends_with(".zip") {
            let mut archive =
                ZipArchive::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            for i in 0..archive.len() {
                let file = archive
                    .by_index(i)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                files.push(file.name().to_string());
            }
        } else if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
            let tar_gz = GzDecoder::new(file);
            let mut archive = tar::Archive::new(tar_gz);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
            {
                let entry = entry.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path.ends_with(".tar.zst") {
            let tar_zst =
                ZstdDecoder::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut archive = tar::Archive::new(tar_zst);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
            {
                let entry = entry.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path.ends_with(".tar.xz") {
            let tar_xz = XzDecoder::new(file);
            let mut archive = tar::Archive::new(tar_xz);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
            {
                let entry = entry.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path.ends_with(".7z") {
            let file =
                fs::File::open(&path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let len = file
                .metadata()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                .len();
            let reader = sevenz_rust::SevenZReader::new(file, len, sevenz_rust::Password::empty())
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            for entry in &reader.archive().files {
                files.push(entry.name.to_string());
            }
        } else if path.ends_with(".rar") {
            if zoi_core::utils::command_exists("unrar") {
                let output = std::process::Command::new("unrar")
                    .arg("lb")
                    .arg(&path)
                    .output()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                if output.status.success() {
                    let list = String::from_utf8_lossy(&output.stdout);
                    for line in list.lines() {
                        files.push(line.to_string());
                    }
                }
            }
        } else if path.ends_with(".deb") {
            let mut ar = ArArchive::new(file);
            while let Some(entry_result) = ar.next_entry() {
                let entry = entry_result.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let header = entry.header();
                files.push(String::from_utf8_lossy(header.identifier()).to_string());
            }
        } else {
            return Err(mlua::Error::RuntimeError(format!(
                "Unsupported archive format: {}",
                path
            )));
        }

        Ok(files)
    })?;
    archive_table.set("list", list_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("ARCHIVE", archive_table)?;

    Ok(())
}
