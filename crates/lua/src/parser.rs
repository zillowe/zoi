//! Parser for Zoi package definitions (`.pkg.lua`).
//!
//! This module provides functions to parse Lua package definitions from files
//! or Zoi archives, extracting metadata, dependencies, and other configuration.

use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use tar::Archive;
use walkdir::WalkDir;
use zoi_core::{types, utils};
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::functions;

/// Parses a Lua package definition from a Zoi archive (`.zpa` or `.zsa`).
///
/// This function detects the current platform and uses it for resolution.
///
/// # Errors
/// Returns an error if the archive cannot be read, unpacked, or if no
/// `.pkg.lua` is found.
pub fn parse_lua_package_from_archive(
    archive_path: &Path,
    version_override: Option<&str,>,
    scope: Option<types::Scope,>,
    quiet: bool,
) -> Result<types::Package,> {
    let platform = utils::get_platform()?;
    parse_lua_package_from_archive_for_platform(
        archive_path,
        &platform,
        version_override,
        scope,
        quiet,
    )
}

/// Parses a Lua package definition from a Zoi archive for a specific platform.
///
/// # Errors
/// Returns an error if the archive cannot be read, unpacked, or if no
/// `.pkg.lua` is found.
pub fn parse_lua_package_from_archive_for_platform(
    archive_path: &Path,
    platform: &str,
    version_override: Option<&str,>,
    scope: Option<types::Scope,>,
    quiet: bool,
) -> Result<types::Package,> {
    let file = fs::File::open(archive_path,)?;
    let decoder = ZstdDecoder::new(file,)?;
    let mut archive = Archive::new(decoder,);
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-arch-parse-",)
        .tempdir()?;
    archive.unpack(temp_dir.path(),)?;

    let mut pkg_lua = None;
    for entry in WalkDir::new(temp_dir.path(),)
        .into_iter()
        .filter_map(Result::ok,)
    {
        if entry.file_name().to_string_lossy().ends_with(".pkg.lua",) {
            pkg_lua = Some(entry.path().to_path_buf(),);
            break;
        }
    }

    let pkg_lua_path =
        pkg_lua.ok_or_else(|| anyhow!("No .pkg.lua in archive"),)?;
    parse_lua_package_from_file_for_platform(
        pkg_lua_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path"),)?,
        platform,
        version_override,
        scope,
        quiet,
    )
}

/// Parses a Lua package definition from either a file or an archive for a
/// specific platform.
///
/// # Errors
/// Returns an error if the file or archive cannot be read or parsed.
pub fn parse_lua_package_for_platform(
    file_path: &str,
    platform: &str,
    version_override: Option<&str,>,
    scope: Option<types::Scope,>,
    quiet: bool,
) -> Result<types::Package,> {
    let path = Path::new(file_path,);
    if path.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("zpa",) || ext.eq_ignore_ascii_case("zsa",)
    },)
    {
        return parse_lua_package_from_archive_for_platform(
            path,
            platform,
            version_override,
            scope,
            quiet,
        );
    }
    parse_lua_package_from_file_for_platform(
        file_path,
        platform,
        version_override,
        scope,
        quiet,
    )
}

/// Internal function to parse a Lua package definition from a file for a
/// specific platform.
///
/// This function sets up the Lua environment, executes the script, and extracts
/// the resulting package metadata.
fn parse_lua_package_from_file_for_platform(
    file_path: &str,
    platform: &str,
    version_override: Option<&str,>,
    scope: Option<types::Scope,>,
    quiet: bool,
) -> Result<types::Package,> {
    let lua_code = fs::read_to_string(file_path,)?;
    let lua = Lua::new();

    functions::setup_lua_environment(
        &lua,
        platform,
        version_override,
        Some(file_path,),
        None,
        None,
        None,
        None,
        scope,
        None,
        quiet,
    )
    .map_err(|e| {
        anyhow!("Failed to setup Lua environment for '{file_path}': {e}")
    },)?;

    lua.load(&lua_code,).exec().map_err(|e| {
        anyhow!(
            "Failed to execute Lua package file '{file_path}':
{e}"
        )
    },)?;

    let final_pkg_meta: Table = lua
        .globals()
        .get("__ZoiPackageMeta",)
        .map_err(|e| anyhow!(e.to_string()),)?;
    let final_pkg_deps: Table = lua
        .globals()
        .get("__ZoiPackageDeps",)
        .map_err(|e| anyhow!(e.to_string()),)?;
    let final_pkg_updates: Table = lua
        .globals()
        .get("__ZoiPackageUpdates",)
        .map_err(|e| anyhow!(e.to_string()),)?;
    let final_pkg_hooks: Table = lua
        .globals()
        .get("__ZoiPackageHooks",)
        .map_err(|e| anyhow!(e.to_string()),)?;
    let final_pkg_service: Table = lua
        .globals()
        .get("__ZoiPackageService",)
        .map_err(|e| anyhow!(e.to_string()),)?;

    let mut package: types::Package = lua
        .from_value(Value::Table(final_pkg_meta.clone(),),)
        .map_err(|e| {
            anyhow!(
                "Failed to parse 'metadata' block in package file \
                 '{file_path}':\n{e}"
            )
        },)?;

    // Manually extract zoios field to handle boolean/nil correctly if needed,
    // though from_value should handle it.
    package.zoios = final_pkg_meta.get("zoios",).ok();

    package.dependencies = if final_pkg_deps.is_empty() {
        None
    } else {
        Some(
            lua.from_value(Value::Table(final_pkg_deps,),)
                .map_err(|e| {
                    anyhow!(
                        "Failed to parse 'dependencies' block in package file \
                         '{file_path}':\n{e}"
                    )
                },)?,
        )
    };

    package.updates = if final_pkg_updates.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_updates,),).map_err(
            |e| {
                anyhow!(
                    "Failed to parse 'updates' block in package file \
                     '{file_path}':
{e}"
                )
            },
        )?,)
    };

    package.hooks = if final_pkg_hooks.is_empty() {
        None
    } else {
        Some(
            lua.from_value(Value::Table(final_pkg_hooks,),)
                .map_err(|e| {
                    anyhow!(
                        "Failed to parse 'hooks' block in package file \
                         '{file_path}':
{e}"
                    )
                },)?,
        )
    };

    package.service = if final_pkg_service.is_empty() {
        None
    } else {
        Some(lua.from_value(Value::Table(final_pkg_service,),).map_err(
            |e| {
                anyhow!(
                    "Failed to parse 'service' block in package file \
                     '{file_path}':
{e}"
                )
            },
        )?,)
    };

    Ok(package,)
}

/// Parses a Lua package definition from either a file or an archive.
///
/// This function detects the current platform and uses it for resolution.
///
/// # Errors
/// Returns an error if the file or archive cannot be read or parsed.
pub fn parse_lua_package(
    file_path: &str,
    version_override: Option<&str,>,
    scope: Option<types::Scope,>,
    quiet: bool,
) -> Result<types::Package,> {
    let platform = utils::get_platform()?;
    parse_lua_package_for_platform(
        file_path,
        &platform,
        version_override,
        scope,
        quiet,
    )
}
