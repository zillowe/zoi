use crate::utils;
use mlua::{self, Lua, Table};
use std::path::Path;

use std::fs;
use walkdir::WalkDir;
pub fn add_file_util(lua: &Lua) -> Result<(), mlua::Error> {
    let file_fn = lua.create_function(
        |_, (url, path): (String, String)| -> Result<(), mlua::Error> {
            let client =
                utils::get_http_client().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut attempt = 0u32;
            let response = loop {
                attempt += 1;
                match client.get(&url).send() {
                    Ok(resp) => break resp,
                    Err(e) => {
                        if attempt < 3 {
                            eprintln!("Download failed ({}). Retrying...", e);
                            crate::utils::retry_backoff_sleep(attempt);
                            continue;
                        } else {
                            return Err(mlua::Error::RuntimeError(e.to_string()));
                        }
                    }
                }
            };
            let content = response
                .bytes()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            fs::write(path, content).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(())
        },
    )?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FILE", file_fn)?;

    Ok(())
}

pub fn add_zcp(lua: &Lua) -> Result<(), mlua::Error> {
    let zcp_fn = lua.create_function(|lua, (source, destination): (String, String)| {
        let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zcp")?;
        op.set("source", source)?;
        op.set("destination", destination)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zcp", zcp_fn)?;
    Ok(())
}

pub fn add_zlicense(lua: &Lua) -> Result<(), mlua::Error> {
    let zlicense_fn = lua.create_function(|lua, source: String| {
        let destination = "${pkgstore}/LICENSE".to_string();
        let zcp: mlua::Function = lua.globals().get("zcp")?;
        zcp.call::<()>((source, destination))?;
        Ok(())
    })?;
    lua.globals().set("zlicense", zlicense_fn)?;
    Ok(())
}

pub fn add_zdoc(lua: &Lua) -> Result<(), mlua::Error> {
    let zdoc_fn = lua.create_function(|lua, source: String| {
        let filename = Path::new(&source)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| mlua::Error::RuntimeError("Invalid source path".to_string()))?;
        let destination = format!("${{pkgstore}}/doc/{}", filename);
        let zcp: mlua::Function = lua.globals().get("zcp")?;
        zcp.call::<()>((source, destination))?;
        Ok(())
    })?;
    lua.globals().set("zdoc", zdoc_fn)?;
    Ok(())
}

pub fn add_zsed(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let zsed_fn = lua.create_function(
        move |lua, (pattern, replacement, file): (String, String, String)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
            let path = Path::new(&build_dir_str).join(&file);

            let content = std::fs::read_to_string(&path).map_err(|e| {
                mlua::Error::RuntimeError(format!("Failed to read {}: {}", file, e))
            })?;

            let re = regex::Regex::new(&pattern).map_err(|e| {
                mlua::Error::RuntimeError(format!("Invalid regex '{}': {}", pattern, e))
            })?;

            let new_content = re.replace_all(&content, replacement.as_str());

            std::fs::write(&path, new_content.as_bytes()).map_err(|e| {
                mlua::Error::RuntimeError(format!("Failed to write {}: {}", file, e))
            })?;

            if !quiet {
                println!("Applied sed replacement to {}", file);
            }

            Ok(())
        },
    )?;
    lua.globals().set("zsed", zsed_fn)?;
    Ok(())
}

pub fn add_zln(lua: &Lua) -> Result<(), mlua::Error> {
    let zln_fn = lua.create_function(|lua, (target, link): (String, String)| {
        let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zln")?;
        op.set("target", target)?;
        op.set("link", link)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zln", zln_fn)?;
    Ok(())
}

pub fn add_zchmod(lua: &Lua) -> Result<(), mlua::Error> {
    let zchmod_fn = lua.create_function(|lua, (path, mode): (String, u32)| {
        let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zchmod")?;
        op.set("path", path)?;
        op.set("mode", mode)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zchmod", zchmod_fn)?;
    Ok(())
}

pub fn add_zchown(lua: &Lua) -> Result<(), mlua::Error> {
    let zchown_fn =
        lua.create_function(|lua, (path, owner, group): (String, String, String)| {
            let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
                Ok(t) => t,
                Err(_) => {
                    let new_t = lua.create_table()?;
                    lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                    new_t
                }
            };
            let op = lua.create_table()?;
            op.set("op", "zchown")?;
            op.set("path", path)?;
            op.set("owner", owner)?;
            op.set("group", group)?;
            ops_table.push(op)?;
            Ok(())
        })?;
    lua.globals().set("zchown", zchown_fn)?;
    Ok(())
}

pub fn add_zmkdir(lua: &Lua) -> Result<(), mlua::Error> {
    let zmkdir_fn = lua.create_function(|lua, path: String| {
        let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zmkdir")?;
        op.set("path", path)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zmkdir", zmkdir_fn)?;
    Ok(())
}

pub fn add_zrm(lua: &Lua) -> Result<(), mlua::Error> {
    let zrm_fn = lua.create_function(|lua, path: String| {
        let ops_table: Table = match lua.globals().get("__ZoiUninstallOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals()
                    .set("__ZoiUninstallOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zrm")?;
        op.set("path", path)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zrm", zrm_fn)?;
    Ok(())
}

pub fn add_fs_util(lua: &Lua) -> Result<(), mlua::Error> {
    let fs_table = lua.create_table()?;

    let exists_fn = lua.create_function(|lua, path: String| {
        let p = Path::new(&path);
        if p.exists() {
            return Ok(true);
        }
        if let Ok(build_dir) = lua.globals().get::<String>("BUILD_DIR")
            && Path::new(&build_dir).join(p).exists()
        {
            return Ok(true);
        }
        Ok(false)
    })?;
    fs_table.set("exists", exists_fn)?;

    let copy_fn = lua.create_function(|_, (src, dest): (String, String)| {
        let src_path = Path::new(&src);
        let dest_path = Path::new(&dest);
        if src_path.is_dir() {
            utils::copy_dir_all(src_path, dest_path)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else {
            fs::copy(src_path, dest_path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        }
        Ok(true)
    })?;
    fs_table.set("copy", copy_fn)?;

    let move_fn = lua.create_function(|_, (src, dest): (String, String)| {
        fs::rename(src, dest).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        Ok(true)
    })?;
    fs_table.set("move", move_fn)?;

    let chmod_fn = lua.create_function(|_, (path, mode): (String, u32)| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        }
        #[cfg(windows)]
        {
            let _ = (path, mode);
        }
        Ok(true)
    })?;
    fs_table.set("chmod", chmod_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FS", fs_table)?;

    Ok(())
}

pub fn add_find_util(lua: &Lua) -> Result<(), mlua::Error> {
    let find_table = lua.create_table()?;

    let find_file_fn = lua.create_function(|lua, (dir, name): (String, String)| {
        let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
        let search_dir = Path::new(&build_dir_str).join(dir);
        for entry in WalkDir::new(search_dir) {
            let entry = entry.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            if entry.file_name().to_string_lossy() == name {
                let path = entry.path();
                let relative_path = path.strip_prefix(Path::new(&build_dir_str)).map_err(|e| {
                    mlua::Error::RuntimeError(format!(
                        "Failed to determine relative path for {:?}: {}",
                        path, e
                    ))
                })?;
                return Ok(Some(relative_path.to_string_lossy().to_string()));
            }
        }
        Ok(None)
    })?;
    find_table.set("file", find_file_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FIND", find_table)?;

    Ok(())
}
