use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use std::fs;
use std::path::Path;

/// Exposes the core Package DSL and lifecycle functions to the Lua environment.
///
/// This module defines the primary entry points for a `.pkg.lua` script:
/// - `metadata`: Defines the static `Package` struct fields.
/// - `dependencies`: Defines the runtime and build dependency graph.
/// - `prepare`/`build`/`package`: Placeholder functions that the maintainer overrides
///   to define the build logic.
/// - `IMPORT`/`INCLUDE`: Helpers for modular package definitions.
///
/// These functions bridge the declarative metadata and the imperative build logic.
pub fn add_import_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let import_fn = lua.create_function(move |lua, file_name: String| {
        let parent = current_path_buf.parent().ok_or_else(|| {
            mlua::Error::RuntimeError(
                "Could not determine parent directory of package file".to_string(),
            )
        })?;
        let path = parent.join(&file_name);
        let content =
            fs::read_to_string(&path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            match extension {
                "json" => {
                    let value: serde_json::Value = serde_json::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                "yaml" | "yml" => {
                    let value: serde_yaml::Value = serde_yaml::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                "toml" => {
                    let value: toml::Value = toml::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                _ => {
                    return lua.to_value(&content);
                }
            }
        }

        lua.to_value(&content)
    })?;
    lua.globals().set("IMPORT", import_fn)?;
    Ok(())
}

pub fn add_include_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let include_fn =
        lua.create_function(move |lua, file_name: String| -> Result<(), mlua::Error> {
            let parent = current_path_buf.parent().ok_or_else(|| {
                mlua::Error::RuntimeError(
                    "Could not determine parent directory of package file".to_string(),
                )
            })?;
            let path = parent.join(file_name);
            let code =
                fs::read_to_string(path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            lua.load(&code).exec()?;
            Ok(())
        })?;
    lua.globals().set("INCLUDE", include_fn)?;
    Ok(())
}

pub fn add_package_lifecycle_functions(lua: &Lua) -> Result<(), mlua::Error> {
    let metadata_fn = lua.create_function(move |lua, pkg_def: Table| {
        if let Ok(meta_table) = lua.globals().get::<Table>("__ZoiPackageMeta")
            && let Ok(pkg_global) = lua.globals().get::<Table>("PKG")
        {
            for pair in pkg_def.pairs::<Value, Value>() {
                let (key, value) = pair?;
                meta_table.set(key.clone(), value.clone())?;
                pkg_global.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("metadata", metadata_fn)?;

    let dependencies_fn = lua.create_function(move |lua, deps_def: Table| {
        if let Ok(deps_table) = lua.globals().get::<Table>("__ZoiPackageDeps") {
            for pair in deps_def.pairs::<String, Value>() {
                let (key, value) = pair?;
                deps_table.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("dependencies", dependencies_fn)?;

    let updates_fn = lua.create_function(move |lua, updates_list: Table| {
        if let Ok(updates_table) = lua.globals().get::<Table>("__ZoiPackageUpdates") {
            for pair in updates_list.pairs::<Value, Table>() {
                let (_, update_info) = pair?;
                updates_table.push(update_info)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("updates", updates_fn)?;

    let hooks_fn = lua.create_function(move |lua, hooks_def: Table| {
        if let Ok(hooks_table) = lua.globals().get::<Table>("__ZoiPackageHooks") {
            for pair in hooks_def.pairs::<String, Value>() {
                let (key, value) = pair?;
                hooks_table.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("hooks", hooks_fn)?;

    let service_fn = lua.create_function(move |lua, service_def: Table| {
        if let Ok(service_table) = lua.globals().get::<Table>("__ZoiPackageService") {
            for pair in service_def.pairs::<String, Value>() {
                let (key, value) = pair?;
                service_table.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("service", service_fn)?;

    let prepare_fn = lua.create_function(|_, _: mlua::MultiValue| Ok(()))?;
    lua.globals().set("prepare", prepare_fn)?;
    let package_fn = lua.create_function(|_, _: mlua::MultiValue| Ok(()))?;
    lua.globals().set("package", package_fn)?;
    let verify_fn = lua.create_function(|_, _: mlua::MultiValue| Ok(true))?;
    lua.globals().set("verify", verify_fn)?;
    let test_fn = lua.create_function(|_, _: mlua::MultiValue| Ok(true))?;
    lua.globals().set("test", test_fn)?;
    let uninstall_fn = lua.create_function(|_, _: mlua::MultiValue| Ok(()))?;
    lua.globals().set("uninstall", uninstall_fn)?;

    Ok(())
}
