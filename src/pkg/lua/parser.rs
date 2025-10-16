use super::functions;
use crate::{pkg::types, utils};
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use std::error::Error;
use std::fs;

pub fn parse_lua_package_for_platform(
    file_path: &str,
    platform: &str,
    version_override: Option<&str>,
) -> std::result::Result<types::Package, Box<dyn Error>> {
    let lua_code = fs::read_to_string(file_path)?;
    let lua = Lua::new();

    let pkg_meta_table = lua.create_table()?;
    let pkg_deps_table = lua.create_table()?;
    let pkg_updates_table = lua.create_table()?;
    let pkg_hooks_table = lua.create_table()?;
    lua.globals().set("__ZoiPackageMeta", pkg_meta_table)?;
    lua.globals().set("__ZoiPackageDeps", pkg_deps_table)?;
    lua.globals()
        .set("__ZoiPackageUpdates", pkg_updates_table)?;
    lua.globals().set("__ZoiPackageHooks", pkg_hooks_table)?;

    let pkg_table = lua.create_table()?;
    lua.globals().set("PKG", pkg_table)?;

    functions::setup_lua_environment(&lua, platform, version_override, Some(file_path))?;

    lua.load(&lua_code).exec()?;

    let final_pkg_meta: Table = lua.globals().get("__ZoiPackageMeta")?;
    let final_pkg_deps: Table = lua.globals().get("__ZoiPackageDeps")?;
    let final_pkg_updates: Table = lua.globals().get("__ZoiPackageUpdates")?;
    let final_pkg_hooks: Table = lua.globals().get("__ZoiPackageHooks")?;

    let mut package: types::Package = lua.from_value(Value::Table(final_pkg_meta))?;

    package.dependencies = if final_pkg_deps.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_deps))?)
    };

    package.updates = if final_pkg_updates.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_updates))?)
    };

    package.hooks = if final_pkg_hooks.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_hooks))?)
    };

    Ok(package)
}

pub fn parse_lua_package(
    file_path: &str,
    version_override: Option<&str>,
) -> std::result::Result<types::Package, Box<dyn Error>> {
    let platform = utils::get_platform()?;
    parse_lua_package_for_platform(file_path, &platform, version_override)
}
