use crate::{pkg::types, utils};
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use std::error::Error;
use std::fs;

fn add_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let fetch_fn = lua.create_function(|lua, url: String| -> Result<Table, mlua::Error> {
        let response =
            reqwest::blocking::get(url).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        let fetch_response = lua.create_table()?;

        fetch_response.set("status", response.status().as_u16())?;

        let response_text = response
            .text()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        fetch_response.set("text", response_text)?;

        Ok(fetch_response)
    })?;

    lua.globals().set("fetch", fetch_fn)?;

    Ok(())
}

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

    let system_table = lua.create_table()?;
    let parts: Vec<&str> = platform.split('-').collect();
    system_table.set("OS", *parts.first().unwrap_or(&""))?;
    system_table.set("ARCH", *parts.get(1).unwrap_or(&""))?;
    if let Some(distro) = utils::get_linux_distribution()
        && platform.starts_with("linux")
    {
        system_table.set("DISTRO", distro)?;
    }
    if let Some(ver) = version_override {
        system_table.set("VERSION", ver)?;
    }
    lua.globals().set("SYSTEM", system_table)?;

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

    add_fetch_util(&lua)?;

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
