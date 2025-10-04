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
    let pkg_install_table = lua.create_table()?;
    let pkg_deps_table = lua.create_table()?;
    lua.globals().set("__ZoiPackageMeta", pkg_meta_table)?;
    lua.globals()
        .set("__ZoiPackageInstall", pkg_install_table)?;
    lua.globals().set("__ZoiPackageDeps", pkg_deps_table)?;
    lua.globals().set("__ZoiPackageSelectable", false)?;

    functions::setup_lua_environment(&lua, platform, version_override, Some(file_path))?;

    let pkg_table = lua.create_table()?;
    lua.globals().set("PKG", pkg_table)?;

    let package_fn = lua.create_function(move |lua, pkg_def: Table| {
        let meta_table: Table = lua.globals().get("__ZoiPackageMeta")?;
        let pkg_global: Table = lua.globals().get("PKG")?;
        for pair in pkg_def.pairs::<Value, Value>() {
            let (key, value) = pair?;
            meta_table.set(key.clone(), value.clone())?;
            pkg_global.set(key, value)?;
        }
        Ok(())
    })?;
    lua.globals().set("package", package_fn)?;

    let install_fn = lua.create_function(move |lua, install_def: Table| {
        let install_table: Table = lua.globals().get("__ZoiPackageInstall")?;
        if let Ok(selectable) = install_def.get::<bool>("selectable") {
            lua.globals().set("__ZoiPackageSelectable", selectable)?;
        }

        for pair in install_def.pairs::<Value, Value>() {
            let (key, value) = pair?;
            if let Value::Integer(i) = key
                && let Value::Table(method_table) = value
            {
                install_table.set(i, method_table)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("install", install_fn)?;

    let dependencies_fn = lua.create_function(move |lua, deps_def: Table| {
        let deps_table: Table = lua.globals().get("__ZoiPackageDeps")?;
        for pair in deps_def.pairs::<String, Value>() {
            let (key, value) = pair?;
            deps_table.set(key, value)?;
        }
        Ok(())
    })?;
    lua.globals().set("dependencies", dependencies_fn)?;

    lua.load(&lua_code).exec()?;

    let final_pkg_meta: Table = lua.globals().get("__ZoiPackageMeta")?;
    let final_pkg_install: Table = lua.globals().get("__ZoiPackageInstall")?;
    let final_pkg_deps: Table = lua.globals().get("__ZoiPackageDeps")?;
    let final_pkg_selectable: bool = lua.globals().get("__ZoiPackageSelectable")?;

    let mut package: types::Package = lua.from_value(Value::Table(final_pkg_meta))?;

    package.installation = lua.from_value(Value::Table(final_pkg_install))?;
    package.selectable = Some(final_pkg_selectable);
    package.dependencies = if final_pkg_deps.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_deps))?)
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
