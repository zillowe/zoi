use super::functions;
use crate::{pkg::types, utils};
use anyhow::{Result, anyhow};
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use std::fs;

pub fn parse_lua_package_for_platform(
    file_path: &str,
    platform: &str,
    version_override: Option<&str>,
) -> Result<types::Package> {
    let lua_code = fs::read_to_string(file_path)?;
    let lua = Lua::new();

    let pkg_meta_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_deps_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_updates_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_hooks_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageMeta", pkg_meta_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageDeps", pkg_deps_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageUpdates", pkg_updates_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageHooks", pkg_hooks_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    let pkg_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("PKG", pkg_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    functions::setup_lua_environment(&lua, platform, version_override, Some(file_path))
        .map_err(|e| anyhow!(e.to_string()))?;

    lua.load(&lua_code)
        .exec()
        .map_err(|e| anyhow!(e.to_string()))?;

    let final_pkg_meta: Table = lua
        .globals()
        .get("__ZoiPackageMeta")
        .map_err(|e| anyhow!(e.to_string()))?;
    let final_pkg_deps: Table = lua
        .globals()
        .get("__ZoiPackageDeps")
        .map_err(|e| anyhow!(e.to_string()))?;
    let final_pkg_updates: Table = lua
        .globals()
        .get("__ZoiPackageUpdates")
        .map_err(|e| anyhow!(e.to_string()))?;
    let final_pkg_hooks: Table = lua
        .globals()
        .get("__ZoiPackageHooks")
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut package: types::Package = lua
        .from_value(Value::Table(final_pkg_meta))
        .map_err(|e| anyhow!(e.to_string()))?;

    package.dependencies = if final_pkg_deps.is_empty() {
        None
    } else {
        Some(
            lua.from_value(Value::Table(final_pkg_deps))
                .map_err(|e| anyhow!(e.to_string()))?,
        )
    };

    package.updates = if final_pkg_updates.is_empty() {
        None
    } else {
        Some(
            lua.from_value(Value::Table(final_pkg_updates))
                .map_err(|e| anyhow!(e.to_string()))?,
        )
    };

    package.hooks = if final_pkg_hooks.is_empty() {
        None
    } else {
        Some(
            lua.from_value(Value::Table(final_pkg_hooks))
                .map_err(|e| anyhow!(e.to_string()))?,
        )
    };

    Ok(package)
}

pub fn parse_lua_package(
    file_path: &str,
    version_override: Option<&str>,
) -> Result<types::Package> {
    let platform = utils::get_platform()?;
    parse_lua_package_for_platform(file_path, &platform, version_override)
}
