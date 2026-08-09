use std::fs;
use std::path::Path;

use mlua::{self, Lua, Table};
use walkdir::WalkDir;
use zoi_core::utils;
/// Exposes filesystem and staging utilities to the Lua environment.
///
/// This module provides the "Staging Engine" for Zoi packages. Key functions
/// include:
/// - `zcp`: Stages files and directories into the `STAGING_DIR` using
///   origin-aware placeholders.
/// - `zln`: Records symbolic link creation to be performed during the final
///   installation.
/// - `zmkdir`: Records directory creation.
/// - `zchmod`/`zchown`: Records metadata changes for staged files.
///
/// These functions do not always perform immediate actions; instead, they often
/// record operations into `__ZoiBuildOperations` for the Rust engine to execute
/// atomically during the staging-to-store move.
/// Adds file downloading utilities to the Lua environment.
///
/// # Errors
///
/// Returns an error if the `UTILS` table cannot be found or if setting the
/// `FILE` function fails.
pub fn add_file_util(lua: &Lua, quiet: bool,) -> Result<(), mlua::Error,> {
    let file_fn = lua.create_function(
        move |_, (url, path,): (String, String,)| -> Result<(), mlua::Error,> {
            super::download::download_with_progress(
                &url,
                Path::new(&path,),
                quiet,
            )
        },
    )?;

    let utils_table: Table = lua.globals().get("UTILS",)?;
    utils_table.set("FILE", file_fn,)?;

    Ok((),)
}

/// Adds the `zcp` function to the Lua environment for staging files.
///
/// # Errors
///
/// Returns an error if the `zcp` function cannot be set in the global
/// environment.
pub fn add_zcp(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zcp_fn = lua.create_function(
        |lua, (source, destination,): (String, String,)| {
            let ops_table: Table =
                if let Ok(t,) = lua.globals().get("__ZoiBuildOperations",) {
                    t
                } else {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiBuildOperations", new_t.clone(),)?;
                    new_t
                };
            let op = lua.create_table()?;
            op.set("op", "zcp",)?;
            op.set("source", source,)?;
            op.set("destination", destination,)?;
            ops_table.push(op,)?;
            Ok((),)
        },
    )?;
    lua.globals().set("zcp", zcp_fn,)?;
    Ok((),)
}

/// Adds the `zlicense` function to the Lua environment for staging licenses.
///
/// # Errors
///
/// Returns an error if the `zlicense` function cannot be set in the global
/// environment.
pub fn add_zlicense(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zlicense_fn = lua.create_function(|lua, source: String| {
        let zoi_table: Table = lua.globals().get("ZOI",)?;
        let scope: String = zoi_table
            .get("scope",)
            .unwrap_or_else(|_| "user".to_string(),);
        let pkg_table: Table = lua.globals().get("PKG",)?;
        let pkg_name: String = pkg_table
            .get("name",)
            .unwrap_or_else(|_| "unknown".to_string(),);

        let filename = Path::new(&source,)
            .file_name()
            .and_then(|n| n.to_str(),)
            .unwrap_or("LICENSE",);

        let destination = if scope == "system" {
            format!("${{usrroot}}/usr/share/licenses/{pkg_name}/{filename}")
        } else {
            format!("${{pkgstore}}/{filename}")
        };

        let zcp: mlua::Function = lua.globals().get("zcp",)?;
        zcp.call::<()>((source, destination,),)?;
        Ok((),)
    },)?;
    lua.globals().set("zlicense", zlicense_fn,)?;
    Ok((),)
}

/// Adds the `zdoc` function to the Lua environment for staging documentation
/// files.
///
/// # Errors
///
/// Returns an error if the `zdoc` function cannot be set in the global
/// environment.
pub fn add_zdoc(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zdoc_fn = lua.create_function(|lua, source: String| {
        let zoi_table: Table = lua.globals().get("ZOI",)?;
        let scope: String = zoi_table
            .get("scope",)
            .unwrap_or_else(|_| "user".to_string(),);
        let pkg_table: Table = lua.globals().get("PKG",)?;
        let pkg_name: String = pkg_table
            .get("name",)
            .unwrap_or_else(|_| "unknown".to_string(),);

        let filename = Path::new(&source,)
            .file_name()
            .and_then(|n| n.to_str(),)
            .ok_or_else(|| {
                mlua::Error::RuntimeError("Invalid source path".to_string(),)
            },)?;

        let destination = if scope == "system" {
            format!("${{usrroot}}/usr/share/doc/{pkg_name}/{filename}")
        } else {
            format!("${{pkgstore}}/doc/{filename}")
        };

        let zcp: mlua::Function = lua.globals().get("zcp",)?;
        zcp.call::<()>((source, destination,),)?;
        Ok((),)
    },)?;
    lua.globals().set("zdoc", zdoc_fn,)?;
    Ok((),)
}

/// Adds the `zman` function to the Lua environment for staging manual pages.
///
/// # Errors
///
/// Returns an error if the `zman` function cannot be set in the global
/// environment.
pub fn add_zman(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zman_fn = lua.create_function(
        |lua, (source, section,): (String, Option<String,>,)| {
            let zoi_table: Table = lua.globals().get("ZOI",)?;
            let scope: String = zoi_table
                .get("scope",)
                .unwrap_or_else(|_| "user".to_string(),);

            let path = Path::new(&source,);
            let mut source_paths = Vec::new();

            if path.is_dir() {
                // Read the directory and collect all files
                if let Ok(entries,) = fs::read_dir(path,) {
                    for entry in entries.flatten() {
                        if entry.file_type().is_ok_and(|t| t.is_file(),) {
                            source_paths.push(entry.path(),);
                        }
                    }
                }
            } else {
                source_paths.push(path.to_path_buf(),);
            }

            for p in source_paths {
                let filename = p
                    .file_name()
                    .and_then(|n| n.to_str(),)
                    .ok_or_else(|| {
                        mlua::Error::RuntimeError(
                            "Invalid source path for zman".to_string(),
                        )
                    },)?;

                let inferred_section = if let Some(ref s,) = section {
                    s.clone()
                } else {
                    // Try to infer from extension (e.g. .1, .5, .1.gz, .5.bz2)
                    let stem =
                        p.file_stem().and_then(|s| s.to_str(),).unwrap_or("",);
                    let ext =
                        p.extension().and_then(|e| e.to_str(),).unwrap_or("",);

                    if ext.parse::<u8>().is_ok() {
                        ext.to_string()
                    } else if (ext == "gz" || ext == "bz2" || ext == "xz")
                        && !stem.is_empty()
                    {
                        let inner_ext = Path::new(stem,)
                            .extension()
                            .and_then(|e| e.to_str(),)
                            .unwrap_or("",);
                        if inner_ext.parse::<u8>().is_ok() {
                            inner_ext.to_string()
                        } else {
                            "1".to_string()
                        }
                    } else {
                        "1".to_string()
                    }
                };

                let destination = if scope == "system" {
                    format!(
                        "${{usrroot}}/usr/share/man/man{inferred_section}/\
                         {filename}"
                    )
                } else {
                    format!(
                        "${{pkgstore}}/man/man{inferred_section}/{filename}"
                    )
                };

                let zcp: mlua::Function = lua.globals().get("zcp",)?;
                zcp.call::<()>(
                    (p.to_string_lossy().to_string(), destination,),
                )?;
            }
            Ok((),)
        },
    )?;
    lua.globals().set("zman", zman_fn,)?;
    Ok((),)
}

/// Adds the `zshell` function to the Lua environment for staging shell
/// completions.
///
/// # Errors
///
/// Returns an error if the `zshell` function cannot be set in the global
/// environment.
pub fn add_zshell(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zshell_fn =
        lua.create_function(|lua, (source, shell,): (String, String,)| {
            let filename = Path::new(&source,)
                .file_name()
                .and_then(|n| n.to_str(),)
                .ok_or_else(|| {
                    mlua::Error::RuntimeError(
                        "Invalid source path for zshell".to_string(),
                    )
                },)?
                .to_string();

            let destination = format!("${{pkgstore}}/shell/{shell}/{filename}");
            let zcp: mlua::Function = lua.globals().get("zcp",)?;
            zcp.call::<()>((source, destination.clone(),),)?;

            let shells_table: Table =
                if let Ok(t,) = lua.globals().get("__ZoiPackageShells",) {
                    t
                } else {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiPackageShells", new_t.clone(),)?;
                    new_t
                };

            let shell_files: Vec<String,> =
                shells_table.get(&*shell,).unwrap_or_default();
            let mut shell_files = shell_files;
            shell_files.push(filename,);
            shells_table.set(shell, shell_files,)?;

            Ok((),)
        },)?;
    lua.globals().set("zshell", zshell_fn,)?;
    Ok((),)
}

/// Adds the `zsed` function to the Lua environment for text replacements in
/// files.
///
/// # Errors
///
/// Returns an error if the `zsed` function cannot be set in the global
/// environment.
pub fn add_zsed(lua: &Lua, quiet: bool,) -> Result<(), mlua::Error,> {
    let zsed_fn = lua.create_function(
        move |lua, (pattern, replacement, file,): (String, String, String,)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR",)?;
            let path = Path::new(&build_dir_str,).join(&file,);

            let content =
                std::fs::read_to_string(&path,).map_err(|e| {
                    mlua::Error::RuntimeError(format!(
                        "Failed to read {file}: {e}"
                    ),)
                },)?;

            let re = regex::Regex::new(&pattern,).map_err(|e| {
                mlua::Error::RuntimeError(format!(
                    "Invalid regex '{pattern}': {e}"
                ),)
            },)?;

            let new_content = re.replace_all(&content, replacement.as_str(),);

            std::fs::write(&path, new_content.as_bytes(),).map_err(|e| {
                mlua::Error::RuntimeError(format!(
                    "Failed to write {file}: {e}"
                ),)
            },)?;

            if !quiet {
                println!("Applied sed replacement to {file}");
            }

            Ok((),)
        },
    )?;
    lua.globals().set("zsed", zsed_fn,)?;
    Ok((),)
}

/// Adds the `zln` function to the Lua environment for creating symbolic links.
///
/// # Errors
///
/// Returns an error if the `zln` function cannot be set in the global
/// environment.
pub fn add_zln(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zln_fn =
        lua.create_function(|lua, (target, link,): (String, String,)| {
            let ops_table: Table =
                if let Ok(t,) = lua.globals().get("__ZoiBuildOperations",) {
                    t
                } else {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiBuildOperations", new_t.clone(),)?;
                    new_t
                };
            let op = lua.create_table()?;
            op.set("op", "zln",)?;
            op.set("target", target,)?;
            op.set("link", link,)?;
            ops_table.push(op,)?;
            Ok((),)
        },)?;
    lua.globals().set("zln", zln_fn,)?;
    Ok((),)
}

/// Adds the `zchmod` function to the Lua environment for changing file
/// permissions.
///
/// # Errors
///
/// Returns an error if the `zchmod` function cannot be set in the global
/// environment.
pub fn add_zchmod(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zchmod_fn =
        lua.create_function(|lua, (path, mode,): (String, u32,)| {
            let ops_table: Table =
                if let Ok(t,) = lua.globals().get("__ZoiBuildOperations",) {
                    t
                } else {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiBuildOperations", new_t.clone(),)?;
                    new_t
                };
            let op = lua.create_table()?;
            op.set("op", "zchmod",)?;
            op.set("path", path,)?;
            op.set("mode", mode,)?;
            ops_table.push(op,)?;
            Ok((),)
        },)?;
    lua.globals().set("zchmod", zchmod_fn,)?;
    Ok((),)
}

/// Adds the `zchown` function to the Lua environment for changing file
/// ownership.
///
/// # Errors
///
/// Returns an error if the `zchown` function cannot be set in the global
/// environment.
pub fn add_zchown(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zchown_fn = lua.create_function(
        |lua, (path, owner, group,): (String, String, String,)| {
            let ops_table: Table =
                if let Ok(t,) = lua.globals().get("__ZoiBuildOperations",) {
                    t
                } else {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiBuildOperations", new_t.clone(),)?;
                    new_t
                };
            let op = lua.create_table()?;
            op.set("op", "zchown",)?;
            op.set("path", path,)?;
            op.set("owner", owner,)?;
            op.set("group", group,)?;
            ops_table.push(op,)?;
            Ok((),)
        },
    )?;
    lua.globals().set("zchown", zchown_fn,)?;
    Ok((),)
}

/// Adds the `zmkdir` function to the Lua environment for creating directories.
///
/// # Errors
///
/// Returns an error if the `zmkdir` function cannot be set in the global
/// environment.
pub fn add_zmkdir(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zmkdir_fn = lua.create_function(|lua, path: String| {
        let ops_table: Table =
            if let Ok(t,) = lua.globals().get("__ZoiBuildOperations",) {
                t
            } else {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone(),)?;
                new_t
            };
        let op = lua.create_table()?;
        op.set("op", "zmkdir",)?;
        op.set("path", path,)?;
        ops_table.push(op,)?;
        Ok((),)
    },)?;
    lua.globals().set("zmkdir", zmkdir_fn,)?;
    Ok((),)
}

/// Adds the `zrm` function to the Lua environment for removing files during
/// uninstallation.
///
/// # Errors
///
/// Returns an error if the `zrm` function cannot be set in the global
/// environment.
pub fn add_zrm(lua: &Lua,) -> Result<(), mlua::Error,> {
    let zrm_fn = lua.create_function(|lua, path: String| {
        let ops_table: Table =
            if let Ok(t,) = lua.globals().get("__ZoiUninstallOperations",) {
                t
            } else {
                let new_t = lua.create_table()?;
                lua.globals()
                    .set("__ZoiUninstallOperations", new_t.clone(),)?;
                new_t
            };
        let op = lua.create_table()?;
        op.set("op", "zrm",)?;
        op.set("path", path,)?;
        ops_table.push(op,)?;
        Ok((),)
    },)?;
    lua.globals().set("zrm", zrm_fn,)?;
    Ok((),)
}

/// Adds general filesystem utilities to the `UTILS.FS` table.
///
/// # Errors
///
/// Returns an error if the `UTILS` table cannot be found or if setting the `FS`
/// table fails.
pub fn add_fs_util(lua: &Lua,) -> Result<(), mlua::Error,> {
    let fs_table = lua.create_table()?;

    let exists_fn = lua.create_function(|lua, path: String| {
        let p = Path::new(&path,);
        if p.exists() {
            return Ok(true,);
        }
        if let Ok(build_dir,) = lua.globals().get::<String>("BUILD_DIR",)
            && Path::new(&build_dir,).join(p,).exists()
        {
            return Ok(true,);
        }
        Ok(false,)
    },)?;
    fs_table.set("exists", exists_fn,)?;

    let copy_fn =
        lua.create_function(|_, (src, dest,): (String, String,)| {
            let src_path = Path::new(&src,);
            let dest_path = Path::new(&dest,);
            if src_path.is_dir() {
                utils::copy_dir_all(src_path, dest_path,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            } else {
                fs::copy(src_path, dest_path,)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            }
            Ok(true,)
        },)?;
    fs_table.set("copy", copy_fn,)?;

    let move_fn =
        lua.create_function(|_, (src, dest,): (String, String,)| {
            fs::rename(src, dest,)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            Ok(true,)
        },)?;
    fs_table.set("move", move_fn,)?;

    let chmod_fn =
        lua.create_function(|_, (path, mode,): (String, u32,)| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(mode,),)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
            }
            #[cfg(windows)]
            {
                let _ = (path, mode,);
            }
            Ok(true,)
        },)?;
    fs_table.set("chmod", chmod_fn,)?;

    let utils_table: Table = lua.globals().get("UTILS",)?;
    utils_table.set("FS", fs_table,)?;

    Ok((),)
}

/// Adds file finding utilities to the `UTILS.FIND` table.
///
/// # Errors
///
/// Returns an error if the `UTILS` table cannot be found or if setting the
/// `FIND` table fails.
pub fn add_find_util(lua: &Lua,) -> Result<(), mlua::Error,> {
    let find_table = lua.create_table()?;

    let find_file_fn =
        lua.create_function(|lua, (dir, name,): (String, String,)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR",)?;
            let search_dir = Path::new(&build_dir_str,).join(dir,);
            for entry in WalkDir::new(search_dir,) {
                let entry = entry
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                if entry.file_name().to_string_lossy() == name {
                    let path = entry.path();
                    let relative_path = path
                        .strip_prefix(Path::new(&build_dir_str,),)
                        .map_err(|e| {
                            mlua::Error::RuntimeError(format!(
                                "Failed to determine relative path for {}: {e}",
                                path.display()
                            ),)
                        },)?;
                    return Ok(Some(
                        relative_path.to_string_lossy().to_string(),
                    ),);
                }
            }
            Ok(None,)
        },)?;
    find_table.set("file", find_file_fn,)?;

    let utils_table: Table = lua.globals().get("UTILS",)?;
    utils_table.set("FIND", find_table,)?;

    Ok((),)
}
