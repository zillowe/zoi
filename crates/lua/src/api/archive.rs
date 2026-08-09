//! Archive utilities for Lua scripts.
//!
//! This module provides functions for extracting and creating various archive
//! formats (Zip, Tar, Zstd, Xz, etc.) within the Lua environment.

use std::fs;
use std::path::{Path, PathBuf};

use ar::Archive as ArArchive;
use flate2::read::GzDecoder;
use mlua::{self, Lua, Table};
use sevenz_rust;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Exposes the `UTILS.EXTRACT` function to the Lua environment.
///
/// This utility provides a unified interface for downloading and extracting
/// various archive formats. It handles:
/// - Remote Fetching: If the source starts with http(s), it downloads the file
///   to `BUILD_DIR`.
/// - Format Detection: Dispatches to the appropriate decoder (Zip, Tar, Zstd,
///   Xz, 7z, etc.).
/// - Error Propagation: Any failure (network, filesystem, or corruption) is
///   converted into an `mlua::Error::RuntimeError`, which halts the Lua
///   execution and is caught by the Rust build engine to trigger a rollback.
///
/// # Errors
///
/// Returns an `mlua::Error` if:
/// - The `UTILS` table cannot be found.
/// - The output directory is invalid (not a subdirectory of `BUILD_DIR`).
/// - Filesystem operations (create dir, open file, copy, remove) fail.
/// - Network download fails.
/// - Archive extraction fails.
/// - The archive format is unsupported.
///
/// # Panics
///
/// This function may panic if:
/// - Parsing the source URL fails to yield a filename.
/// - Executing external commands (`hdiutil`, `pkgutil`, `unrar`) fails or their
///   output cannot be parsed.
pub fn add_extract_util(lua: &Lua, quiet: bool,) -> Result<(), mlua::Error,> {
    let extract_fn = lua.create_function(
        move |lua, (source, out_name,): (String, Option<String,>,)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR",)?;
            let build_dir = Path::new(&build_dir_str,);

            let archive_file = if source.starts_with("http",) {
                let file_name =
                    source.split('/',).next_back().unwrap_or("download.tmp",);
                let temp_path = build_dir.join(file_name,);
                super::download::download_with_progress(
                    &source, &temp_path, quiet,
                )?;

                temp_path
            } else {
                PathBuf::from(source,)
            };

            let out_dir_name =
                out_name.unwrap_or_else(|| "extracted".to_string(),);
            let out_dir = build_dir.join(&out_dir_name,);

            if !out_dir.starts_with(build_dir,) || out_dir == build_dir {
                return Err(mlua::Error::RuntimeError(format!(
                    "Invalid output directory: {out_dir_name}. Extraction \
                     must be into a subdirectory of the build directory."
                ),),);
            }

            fs::create_dir_all(&out_dir,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;

            if !quiet {
                println!(
                    "Extracting {} to {}",
                    archive_file.display(),
                    out_dir.display()
                );
            }

            let file = fs::File::open(&archive_file,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;

            let archive_path = Path::new(&archive_file,);
            let archive_path_str = archive_file.to_string_lossy();

            if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip",),)
            {
                let mut archive = ZipArchive::new(file,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                archive
                    .extract(&out_dir,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else if archive_path_str.ends_with(".tar.gz",)
                || archive_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz",),)
            {
                let tar_gz = GzDecoder::new(file,);
                let mut archive = tar::Archive::new(tar_gz,);
                archive
                    .unpack(&out_dir,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else if archive_path_str.ends_with(".tar.zst",)
                || archive_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("zpa",),)
                || archive_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("zsa",),)
            {
                let tar_zst = ZstdDecoder::new(file,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                let mut archive = tar::Archive::new(tar_zst,);
                archive
                    .unpack(&out_dir,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else if archive_path_str.ends_with(".tar.xz",) {
                let tar_xz = XzDecoder::new(file,);
                let mut archive = tar::Archive::new(tar_xz,);
                archive
                    .unpack(&out_dir,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("7z",),)
            {
                sevenz_rust::decompress_file(&archive_file, &out_dir,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dmg",),)
            {
                if !cfg!(target_os = "macos") {
                    return Err(mlua::Error::RuntimeError(
                        "Extracting .dmg files is only supported on macOS."
                            .to_string(),
                    ),);
                }
                let output = std::process::Command::new("hdiutil",)
                    .arg("attach",)
                    .arg("-nobrowse",)
                    .arg("-readonly",)
                    .arg(&archive_file,)
                    .output()
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!(
                            "Failed to execute hdiutil: {e}"
                        ),)
                    },)?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr,);
                    return Err(mlua::Error::RuntimeError(format!(
                        "hdiutil failed: {stderr}"
                    ),),);
                }
                let output_str = String::from_utf8_lossy(&output.stdout,);
                let mut mount_point = None;
                for line in output_str.lines() {
                    if line.contains("/Volumes/",)
                        && let Some(idx,) = line.find("/Volumes/",)
                    {
                        mount_point = Some(line[idx..].trim().to_string(),);
                        break;
                    }
                }
                let mount_point = mount_point.ok_or_else(|| {
                    mlua::Error::RuntimeError(
                        "Failed to parse mount point from hdiutil output."
                            .to_string(),
                    )
                },)?;
                let mount_path = std::path::Path::new(&mount_point,);
                if let Err(e,) =
                    zoi_core::utils::copy_dir_all(mount_path, &out_dir,)
                {
                    let _ = std::process::Command::new("hdiutil",)
                        .arg("detach",)
                        .arg(&mount_point,)
                        .status();
                    return Err(mlua::Error::RuntimeError(format!(
                        "Failed to copy contents from dmg: {e}"
                    ),),);
                }
                let detach_status = std::process::Command::new("hdiutil",)
                    .arg("detach",)
                    .arg(&mount_point,)
                    .status()
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!(
                            "Failed to execute hdiutil detach: {e}"
                        ),)
                    },)?;
                if !detach_status.success() {
                    eprintln!(
                        "Warning: failed to detach dmg volume at {mount_point}"
                    );
                }
            } else if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pkg",),)
            {
                if !cfg!(target_os = "macos") {
                    return Err(mlua::Error::RuntimeError(
                        "Extracting .pkg files natively is only supported on \
                         macOS."
                            .to_string(),
                    ),);
                }
                let temp_extract_dir = out_dir.join(".pkg_extract_tmp",);
                let status = std::process::Command::new("pkgutil",)
                    .arg("--expand-full",)
                    .arg(&archive_file,)
                    .arg(&temp_extract_dir,)
                    .status()
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!(
                            "Failed to execute pkgutil: {e}"
                        ),)
                    },)?;
                if !status.success() {
                    return Err(mlua::Error::RuntimeError(
                        "pkgutil failed to expand the package.".to_string(),
                    ),);
                }
                zoi_core::utils::copy_dir_all(&temp_extract_dir, &out_dir,)
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!(
                            "Failed to copy pkg contents: {e}"
                        ),)
                    },)?;
                let _ = fs::remove_dir_all(&temp_extract_dir,);
            } else if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rar",),)
            {
                if zoi_core::utils::command_exists("unrar",) {
                    let status = std::process::Command::new("unrar",)
                        .arg("x",)
                        .arg("-y",)
                        .arg(&archive_file,)
                        .arg(&out_dir,)
                        .status()
                        .map_err(|e| {
                            mlua::Error::RuntimeError(e.to_string(),)
                        },)?;
                    if !status.success() {
                        return Err(mlua::Error::RuntimeError(
                            "unrar failed".to_string(),
                        ),);
                    }
                } else {
                    return Err(mlua::Error::RuntimeError(
                        "unrar command not found. Please install unrar to \
                         extract .rar files."
                            .to_string(),
                    ),);
                }
            } else if archive_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("deb",),)
            {
                let mut ar = ArArchive::new(file,);
                while let Some(entry_result,) = ar.next_entry() {
                    let mut entry = entry_result.map_err(|e| {
                        mlua::Error::RuntimeError(e.to_string(),)
                    },)?;
                    let name =
                        String::from_utf8_lossy(entry.header().identifier(),)
                            .trim()
                            .trim_end_matches('/',)
                            .to_string();
                    if name.starts_with("data.tar",) {
                        let temp_data_path = build_dir.join(&name,);
                        let mut temp_file = fs::File::create(&temp_data_path,)
                            .map_err(|e| {
                                mlua::Error::RuntimeError(format!(
                                    "Failed to create temp file for {name}: \
                                     {e}"
                                ),)
                            },)?;
                        std::io::copy(&mut entry, &mut temp_file,).map_err(
                            |e| {
                                mlua::Error::RuntimeError(format!(
                                    "Failed to copy entry data for {name}: {e}"
                                ),)
                            },
                        )?;

                        let data_file = fs::File::open(&temp_data_path,)
                            .map_err(|e| {
                                mlua::Error::RuntimeError(format!(
                                    "Failed to reopen temp file for {name}: \
                                     {e}"
                                ),)
                            },)?;
                        let data_path = Path::new(&name,);
                        if data_path.extension().is_some_and(|ext| {
                                ext.eq_ignore_ascii_case("gz",)
                            },)
                            {
                                let mut archive = tar::Archive::new(
                                    GzDecoder::new(data_file,),
                                );
                                archive.unpack(&out_dir,).map_err(|e| {
                                    mlua::Error::RuntimeError(format!(
                                        "Failed to unpack {name}: {e}"
                                    ),)
                                },)?;
                            } else if data_path.extension().is_some_and(|ext| {
                                ext.eq_ignore_ascii_case("xz",)
                            },)
                            {
                                let mut archive = tar::Archive::new(
                                    XzDecoder::new(data_file,),
                                );
                                archive.unpack(&out_dir,).map_err(|e| {
                                    mlua::Error::RuntimeError(format!(
                                        "Failed to unpack {name}: {e}"
                                    ),)
                                },)?;
                            } else if data_path.extension().is_some_and(|ext| {
                                ext.eq_ignore_ascii_case("zst",)
                            },)
                            {
                                let mut archive = tar::Archive::new(
                                    ZstdDecoder::new(data_file,).map_err(
                                        |e| {
                                            mlua::Error::RuntimeError(format!(
                                                "Failed to initialize zstd \
                                                 for {name}: {e}"
                                            ),)
                                        },
                                    )?,
                                );
                                archive.unpack(&out_dir,).map_err(|e| {
                                    mlua::Error::RuntimeError(format!(
                                        "Failed to unpack {name}: {e}"
                                    ),)
                                },)?;
                            }
                        fs::remove_file(temp_data_path,).ok();
                    }
                }
            } else {
                return Err(mlua::Error::RuntimeError(format!(
                    "Unsupported archive format for file: {archive_path_str}"
                ),),);
            }

            Ok((),)
        },
    )?;

    let utils_table: Table = lua.globals().get("UTILS",)?;
    utils_table.set("EXTRACT", extract_fn,)?;

    Ok((),)
}

/// Exposes the `UTILS.ARCHIVE` table and `UTILS.MAKE_ARCHIVE` function to the
/// Lua environment.
///
/// `UTILS.ARCHIVE` includes:
/// - `list(path)`: Lists the contents of an archive.
///
/// `UTILS.MAKE_ARCHIVE(source, output, algorithm)`: Creates an archive from a
/// source path.
///
/// # Errors
///
/// Returns an `mlua::Error` if:
/// - The `UTILS` table cannot be found.
/// - Filesystem operations (open, create, metadata, read, write) fail.
/// - Archive processing (zip, tar, etc.) fails.
/// - Unsupported archive format or algorithm is provided.
///
/// # Panics
///
/// This function may panic if:
/// - Stripping path prefixes fails during ZIP creation.
/// - The parent of a source path cannot be determined.
pub fn add_archive_util(lua: &Lua,) -> Result<(), mlua::Error,> {
    let archive_table = lua.create_table()?;

    let list_fn = lua.create_function(|lua, path: String| {
        let p = Path::new(&path,);
        let actual_path = if p.exists() {
            p.to_path_buf()
        } else if let Ok(build_dir,) = lua.globals().get::<String>("BUILD_DIR",)
        {
            Path::new(&build_dir,).join(p,)
        } else {
            p.to_path_buf()
        };

        let file = fs::File::open(&actual_path,).map_err(|e| {
            mlua::Error::RuntimeError(format!(
                "Failed to open archive {}: {e}",
                actual_path.display()
            ),)
        },)?;
        let mut files = Vec::new();

        let path_obj = Path::new(&path,);
        if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip",),)
        {
            let mut archive = ZipArchive::new(file,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            for i in 0..archive.len() {
                let file = archive
                    .by_index(i,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                files.push(file.name().to_string(),);
            }
        } else if path.ends_with(".tar.gz",)
            || path_obj
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz",),)
        {
            let tar_gz = GzDecoder::new(file,);
            let mut archive = tar::Archive::new(tar_gz,);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?
            {
                let entry = entry
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path.ends_with(".tar.zst",)
            || path_obj
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zpa",),)
            || path_obj
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zsa",),)
        {
            let tar_zst = ZstdDecoder::new(file,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            let mut archive = tar::Archive::new(tar_zst,);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?
            {
                let entry = entry
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path.ends_with(".tar.xz",) {
            let tar_xz = XzDecoder::new(file,);
            let mut archive = tar::Archive::new(tar_xz,);
            for entry in archive
                .entries()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?
            {
                let entry = entry
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                files.push(
                    entry
                        .path()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        } else if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("7z",),)
        {
            let file = fs::File::open(&path,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            let len = file
                .metadata()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?
                .len();
            let reader = sevenz_rust::SevenZReader::new(
                file,
                len,
                sevenz_rust::Password::empty(),
            )
            .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            for entry in &reader.archive().files {
                files.push(entry.name.clone(),);
            }
        } else if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rar",),)
        {
            if zoi_core::utils::command_exists("unrar",) {
                let output = std::process::Command::new("unrar",)
                    .arg("lb",)
                    .arg(&path,)
                    .output()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                if output.status.success() {
                    let list = String::from_utf8_lossy(&output.stdout,);
                    for line in list.lines() {
                        files.push(line.to_string(),);
                    }
                }
            }
        } else if path_obj
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("deb",),)
        {
            let mut ar = ArArchive::new(file,);
            while let Some(entry_result,) = ar.next_entry() {
                let entry = entry_result
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                let header = entry.header();
                files.push(
                    String::from_utf8_lossy(header.identifier(),).to_string(),
                );
            }
        } else {
            return Err(mlua::Error::RuntimeError(format!(
                "Unsupported archive format: {path}"
            ),),);
        }

        Ok(files,)
    },)?;
    archive_table.set("list", list_fn,)?;

    let make_archive_fn = lua.create_function(
        move |lua, (source, output, algorithm): (mlua::Value, String, Option<String>)| {
            let algo = algorithm
                .unwrap_or_else(|| "zst".to_string())
                .to_lowercase();
            let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
            let build_dir = Path::new(&build_dir_str);

            let output_path = if Path::new(&output).is_absolute() {
                PathBuf::from(&output)
            } else {
                build_dir.join(&output)
            };

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            }

            let mut source_paths = Vec::new();
            match source {
                mlua::Value::String(s) => {
                    let s_borrowed = s.to_str()?;
                    let s_str_ref = s_borrowed.as_ref();
                    let p = build_dir.join(s_str_ref);
                    if p.exists() {
                        source_paths.push((p, s_str_ref.to_string()));
                    } else if Path::new(s_str_ref).exists() {
                        source_paths.push((PathBuf::from(s_str_ref), s_str_ref.to_string()));
                    } else {
                        return Err(mlua::Error::RuntimeError(format!(
                            "MAKE_ARCHIVE: source path does not exist: {s_str_ref}"
                        )));
                    }
                }
                mlua::Value::Table(t) => {
                    for val in t.sequence_values::<String>() {
                        let s_str = val?;
                        let p = build_dir.join(&s_str);
                        if p.exists() {
                            source_paths.push((p, s_str.clone()));
                        } else if Path::new(&s_str).exists() {
                            source_paths.push((PathBuf::from(&s_str), s_str.clone()));
                        } else {
                            return Err(mlua::Error::RuntimeError(format!(
                                "MAKE_ARCHIVE: source path does not exist: {s_str}"
                            )));
                        }
                    }
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "MAKE_ARCHIVE: source must be string or table".to_string(),
                    ));
                }
            }

            let file = fs::File::create(&output_path)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

            match algo.as_str() {
                "gz" => {
                    let mut encoder =
                        flate2::write::GzEncoder::new(file, flate2::Compression::default());
                    for (path, _) in source_paths {
                        let mut f = fs::File::open(path)
                            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                        std::io::copy(&mut f, &mut encoder)
                            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    }
                    encoder
                        .finish()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                }
                "zip" => {
                    let mut zip = zip::ZipWriter::new(file);
                    let options = zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated);

                    for (path, rel_name) in source_paths {
                        if path.is_dir() {
                            let parent = path.parent().expect("source path should have a parent");
                            for entry in walkdir::WalkDir::new(&path)
                                .into_iter()
                                .filter_map(Result::ok)
                            {
                                let rel =
                                    entry.path().strip_prefix(parent).expect("entry path should be within source path");
                                if entry.file_type().is_dir() {
                                    zip.add_directory(rel.to_string_lossy(), options)
                                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                                } else {
                                    zip.start_file(rel.to_string_lossy(), options)
                                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                                    let mut f = fs::File::open(entry.path())
                                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                                    std::io::copy(&mut f, &mut zip)
                                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                                }
                            }
                        } else {
                            zip.start_file(rel_name, options)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                            let mut f = fs::File::open(path)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                            std::io::copy(&mut f, &mut zip)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                        }
                    }
                    zip.finish()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                }
                "tar" | "tar.gz" | "tar.xz" | "tar.zst" | "zst" => {
                    let writer: Box<dyn std::io::Write> = match algo.as_str() {
                        "tar" => Box::new(file),
                        "tar.gz" => Box::new(flate2::write::GzEncoder::new(
                            file,
                            flate2::Compression::default(),
                        )),
                        "tar.xz" => Box::new(xz2::write::XzEncoder::new(file, 6)),
                        "tar.zst" | "zst" => Box::new(
                            zstd::stream::write::Encoder::new(file, 0)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
                                .auto_finish(),
                        ),
                        _ => unreachable!(),
                    };

                    let mut tar = tar::Builder::new(writer);
                    for (path, rel_name) in source_paths {
                        if path.is_dir() {
                            tar.append_dir_all(rel_name, path)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                        } else {
                            tar.append_path_with_name(path, rel_name)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                        }
                    }
                    tar.finish()
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "MAKE_ARCHIVE: unsupported algorithm: {algo}"
                    )));
                }
            }

            Ok(())
        },
    )?;

    let utils_table: Table = lua.globals().get("UTILS",)?;
    utils_table.set("ARCHIVE", archive_table,)?;
    utils_table.set("MAKE_ARCHIVE", make_archive_fn,)?;

    Ok((),)
}
