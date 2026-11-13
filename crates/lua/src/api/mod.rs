//! Lua APIs for Zoi.
//!
//! This module contains the implementations of the various APIs exposed to
//! Lua scripts, organized into submodules by functionality.

use mlua::Lua;

/// Archive manipulation utilities (zip, tar, etc.).
pub mod archive;
/// Cryptographic utilities (hash verification, PGP).
pub mod crypto;
/// File download utilities with progress reporting.
pub mod download;
/// Filesystem and staging utilities.
pub mod fs;
/// HTTP and Git-forge integration.
pub mod http;
/// Core Package DSL and lifecycle functions.
pub mod lifecycle;
/// Data parsing utilities (JSON, YAML, TOML).
pub mod parse;
/// System command execution and patching.
pub mod system;

/// Directories of the current Lua environment, owned by the Rust side.
///
/// The `BUILD_DIR` and `STAGING_DIR` Lua globals are informational only: a
/// package script can overwrite them. Every API that touches the filesystem
/// or spawns processes (`cmd`, `zpatch`, `zsed`, `UTILS.DOWNLOAD`,
/// `UTILS.EXTRACT`, ...) resolves its directories from this app-data value,
/// which is set by `setup_lua_environment` and cannot be changed from Lua.
#[derive(Debug, Clone, Default)]
pub struct LuaEnvDirs {
    /// The temporary build directory, if the environment has one.
    pub build_dir: Option<String>,
    /// The staging directory, if the environment has one.
    pub staging_dir: Option<String>
}

/// Returns the Rust-side build directory of the Lua environment.
///
/// # Errors
///
/// Returns a runtime error if the environment was set up without a build
/// directory.
pub(crate) fn build_dir(lua: &Lua) -> Result<String, mlua::Error> {
    opt_build_dir(lua).ok_or_else(|| {
        mlua::Error::RuntimeError(
            "No build directory is available here: this function can only be \
             used while a package is being built (e.g. from prepare(), \
             build() or package())."
                .to_string()
        )
    })
}

/// Returns the Rust-side build directory of the Lua environment, if any.
pub(crate) fn opt_build_dir(lua: &Lua) -> Option<String> {
    lua.app_data_ref::<LuaEnvDirs>()
        .and_then(|dirs| dirs.build_dir.clone())
}

/// Returns the Rust-side staging directory of the Lua environment, if any.
pub(crate) fn opt_staging_dir(lua: &Lua) -> Option<String> {
    lua.app_data_ref::<LuaEnvDirs>()
        .and_then(|dirs| dirs.staging_dir.clone())
}
