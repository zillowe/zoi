//! High-level Lua environment setup and execution functions.
//!
//! This module provides the `setup_lua_environment` function, which is
//! responsible for initializing the Lua state with all the necessary globals,
//! tables, and utility functions required by Zoi package scripts.

use mlua::{Lua, Table, Value};
use zoi_core::utils;

use crate::api;

/// Selects which APIs a Lua environment exposes to a package script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaEnvMode {
    /// Metadata evaluation, used whenever Zoi only needs to *read* a package
    /// definition (sync/indexing, `show`, `search`, dependency resolution).
    ///
    /// Only side-effect-free helpers are available: `UTILS.FETCH.*`,
    /// `UTILS.PARSE.*`, `UTILS.FS.exists`, `UTILS.ARCHIVE.list`,
    /// `verifyHash`, `verifySignature`, `SYSTEM`, `ZOI`, the registration
    /// functions (`metadata`, `dependencies`, `updates`, `hooks`, `service`)
    /// and `IMPORT`/`INCLUDE` confined to the package directory.
    ///
    /// Everything that runs commands or writes to the host (`cmd`, `zpatch`,
    /// `zsed`, the `z*` staging functions, `UTILS.FS.copy/move/chmod`,
    /// `UTILS.FILE`, `UTILS.DOWNLOAD`, `UTILS.EXTRACT`, `UTILS.MAKE_ARCHIVE`
    /// and `addPgpKey`) is replaced by a stub that raises an error. The Luau
    /// sandbox is enabled, so builtin libraries and API tables are read-only.
    /// Lifecycle functions (`prepare`, `build`, `package`, ...) may still be
    /// *defined*; they only run later in a [`LuaEnvMode::Full`] environment.
    Metadata,
    /// The full build/install/uninstall environment with every API.
    Full
}

/// Global functions that are replaced by stubs in [`LuaEnvMode::Metadata`].
const METADATA_BLOCKED_GLOBALS: &[&str] = &[
    "cmd",
    "zpatch",
    "zsed",
    "zcp",
    "zlicense",
    "zdoc",
    "zman",
    "zshell",
    "zln",
    "zchmod",
    "zchown",
    "zmkdir",
    "zrm",
    "addPgpKey"
];

/// `UTILS` functions that are replaced by stubs in [`LuaEnvMode::Metadata`].
const METADATA_BLOCKED_UTILS: &[&str] =
    &["FILE", "DOWNLOAD", "EXTRACT", "MAKE_ARCHIVE"];

/// `UTILS.FS` functions that are replaced by stubs in
/// [`LuaEnvMode::Metadata`].
const METADATA_BLOCKED_FS: &[&str] = &["copy", "move", "chmod"];

/// Globals that package scripts (and Zoi's own registration functions) are
/// expected to mutate while metadata is evaluated. They are kept writable when
/// the Luau sandbox freezes every other global table.
const METADATA_MUTABLE_GLOBALS: &[&str] = &[
    "PKG",
    "ZOI",
    "SYSTEM",
    "LOCATION",
    "__ZoiPackageMeta",
    "__ZoiPackageDeps",
    "__ZoiPackageUpdates",
    "__ZoiPackageHooks",
    "__ZoiPackageService",
    "__ZoiReferencedFiles",
    "__ZoiBuildOperations",
    "__ZoiUninstallOperations",
    "__ZoiPackageShells"
];

/// Bootstraps the full Lua environment for executing a package definition.
///
/// This function populates the global Lua scope with system information,
/// directory paths, and utility functions that the `.pkg.lua` script
/// expects to have available. It is used for building, testing, installing
/// and uninstalling packages, where lifecycle functions are executed. It is
/// equivalent to [`setup_lua_environment_with_mode`] with
/// [`LuaEnvMode::Full`].
///
/// Code that only needs to read a package's metadata must use
/// [`LuaEnvMode::Metadata`] instead (see [`crate::parser`]).
///
/// # Errors
///
/// Returns an error if the Lua environment cannot be properly initialized.
pub fn setup_lua_environment(
    lua: &Lua,
    platform: &str,
    version_override: Option<&str>,
    file_path: Option<&str>,
    create_pkg_dir: Option<&str>,
    build_dir: Option<&str>,
    staging_dir: Option<&str>,
    sub_package: Option<&str>,
    scope: Option<zoi_core::types::Scope>,
    build_type: Option<&str>,
    quiet: bool
) -> Result<(), mlua::Error> {
    setup_lua_environment_with_mode(
        lua,
        LuaEnvMode::Full,
        platform,
        version_override,
        file_path,
        create_pkg_dir,
        build_dir,
        staging_dir,
        sub_package,
        scope,
        build_type,
        quiet
    )
}

/// Bootstraps the Lua environment for a package definition in the given
/// [`LuaEnvMode`].
///
/// In [`LuaEnvMode::Metadata`] the `build_dir` and `staging_dir` arguments
/// are ignored, dangerous APIs are replaced by stubs and the Luau sandbox is
/// enabled once setup is complete.
///
/// # Errors
///
/// Returns an error if the Lua environment cannot be properly initialized.
pub fn setup_lua_environment_with_mode(
    lua: &Lua,
    mode: LuaEnvMode,
    platform: &str,
    version_override: Option<&str>,
    file_path: Option<&str>,
    create_pkg_dir: Option<&str>,
    build_dir: Option<&str>,
    staging_dir: Option<&str>,
    sub_package: Option<&str>,
    scope: Option<zoi_core::types::Scope>,
    build_type: Option<&str>,
    quiet: bool
) -> Result<(), mlua::Error> {
    let (build_dir, staging_dir) = match mode {
        LuaEnvMode::Full => (build_dir, staging_dir),
        LuaEnvMode::Metadata => (None, None)
    };

    // The directories every privileged API works in live on the Rust side, so
    // a script cannot redirect them by assigning to `BUILD_DIR`.
    lua.set_app_data(api::LuaEnvDirs {
        build_dir: build_dir.map(ToString::to_string),
        staging_dir: staging_dir.map(ToString::to_string)
    });

    // Initialize the global PKG table for script use
    let pkg_table = if let Ok(table) = lua.globals().get::<mlua::Table>("PKG") {
        table
    } else {
        let table = lua.create_table()?;
        lua.globals().set("PKG", table.clone())?;
        table
    };

    if let Some(bt) = build_type {
        lua.globals().set("BUILD_TYPE", bt)?;
    }

    // Initialize internal metadata capture tables if they don't exist
    if lua
        .globals()
        .get::<mlua::Table>("__ZoiPackageMeta")
        .is_err()
    {
        let pkg_meta_table = lua.create_table()?;
        lua.globals().set("__ZoiPackageMeta", pkg_meta_table)?;
    }
    if lua
        .globals()
        .get::<mlua::Table>("__ZoiPackageDeps")
        .is_err()
    {
        let pkg_deps_table = lua.create_table()?;
        lua.globals().set("__ZoiPackageDeps", pkg_deps_table)?;
    }
    if lua
        .globals()
        .get::<mlua::Table>("__ZoiPackageUpdates")
        .is_err()
    {
        let pkg_updates_table = lua.create_table()?;
        lua.globals()
            .set("__ZoiPackageUpdates", pkg_updates_table)?;
    }
    if lua
        .globals()
        .get::<mlua::Table>("__ZoiPackageHooks")
        .is_err()
    {
        let pkg_hooks_table = lua.create_table()?;
        lua.globals().set("__ZoiPackageHooks", pkg_hooks_table)?;
    }
    if lua
        .globals()
        .get::<mlua::Table>("__ZoiPackageService")
        .is_err()
    {
        let pkg_service_table = lua.create_table()?;
        lua.globals()
            .set("__ZoiPackageService", pkg_service_table)?;
    }

    // Host System Information
    // Exposes a 'SYSTEM' table containing OS, Architecture, Distro, etc.
    // Allow maintainers to write platform-specific logic easily.
    let system_table = lua.create_table()?;
    let parts: Vec<&str> = platform.split('-').collect();
    system_table.set("OS", *parts.first().unwrap_or(&""))?;
    system_table.set("ARCH", *parts.get(1).unwrap_or(&""))?;
    if let Some(distro) = utils::get_linux_distribution() {
        system_table.set("DISTRO", distro)?;
    }
    if let Some(de) = utils::get_desktop_environment() {
        system_table.set("DE", de)?;
    }
    if let Some(server) = utils::get_display_server() {
        system_table.set("SERVER", server)?;
    }
    if let Some(dv) = utils::get_distro_version() {
        system_table.set("DISTRO_VER", dv)?;
    }
    if let Some(kernel) = utils::get_kernel_version() {
        system_table.set("KERNEL_VER", kernel)?;
    }
    if let Some(init) = utils::get_init_system() {
        system_table.set("INIT", init)?;
    }
    if let Some(cpu) = utils::get_cpu_info() {
        system_table.set("CPU", cpu)?;
    }
    if let Some(gpu) = utils::get_gpu_info() {
        system_table.set("GPU", gpu)?;
    }
    if let Some(manager) = utils::get_native_package_manager() {
        system_table.set("MANAGER", manager)?;
    }
    lua.globals().set("SYSTEM", system_table)?;

    let zoi_table = lua.create_table()?;
    if let Some(ver) = version_override {
        zoi_table.set("VERSION", ver)?;
    }

    if let Some(s) = scope {
        let scope_str = format!("{s:?}").to_lowercase();
        zoi_table.set("scope", scope_str)?;
    }

    if let Some(dir) = create_pkg_dir {
        zoi_table.set("CREATE_PKG_DIR", dir)?;
    }

    if let Some(sub) = sub_package {
        lua.globals().set("SUBPKG", sub)?;
    }

    let path_table = lua.create_table()?;
    if let Ok(user_data_dir) = utils::get_user_data_dir() {
        path_table.set("user", user_data_dir.to_string_lossy().to_string())?;
    }

    let system_bin_path =
        utils::get_system_bin_dir().to_string_lossy().to_string();
    path_table.set("system", system_bin_path)?;

    zoi_table.set("PATH", path_table)?;

    if let Some(home_dir) = utils::get_user_home() {
        pkg_table.set("home", home_dir.to_string_lossy().to_string())?;
        if let Ok(store_dir) =
            utils::get_store_base_dir(zoi_core::types::Scope::User)
        {
            pkg_table.set("store", store_dir.to_string_lossy().to_string())?;
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        pkg_table.set("template", current_dir.to_string_lossy().to_string())?;
    }

    let root = if cfg!(target_os = "windows") {
        "C:\\"
    } else {
        "/"
    };
    pkg_table.set("root", root)?;

    if let Some(path_str) = file_path {
        let abs_path = if let Ok(p) = std::fs::canonicalize(path_str) {
            p
        } else {
            std::path::Path::new(path_str).to_path_buf()
        };
        pkg_table.set("lua", abs_path.to_string_lossy().to_string())?;
    }
    zoi_table.set("PKG", pkg_table.clone())?;

    let location_table = lua.create_table()?;
    if let Some(sd) = staging_dir {
        let staging_path = std::path::Path::new(sd);
        location_table.set(
            "PKGSTORE",
            staging_path.join("pkgstore").to_string_lossy().to_string()
        )?;
        location_table.set(
            "HOME",
            staging_path.join("usrhome").to_string_lossy().to_string()
        )?;
        location_table.set(
            "ROOT",
            staging_path.join("usrroot").to_string_lossy().to_string()
        )?;
        location_table.set(
            "TEMPLATE",
            staging_path
                .join("createpkgdir")
                .to_string_lossy()
                .to_string()
        )?;
    } else {
        if let Some(home_dir) = utils::get_user_home() {
            location_table.set(
                "PKGSTORE",
                utils::get_store_base_dir(zoi_core::types::Scope::User)
                    .map_err(|error| {
                        mlua::Error::RuntimeError(error.to_string())
                    })?
                    .to_string_lossy()
                    .to_string()
            )?;
            location_table
                .set("HOME", home_dir.to_string_lossy().to_string())?;
        }
        let root = if cfg!(target_os = "windows") {
            "C:\\"
        } else {
            "/"
        };
        location_table.set("ROOT", root.to_string())?;
        if let Ok(current_dir) = std::env::current_dir() {
            location_table
                .set("TEMPLATE", current_dir.to_string_lossy().to_string())?;
        }
    }
    if let Some(path_str) = file_path {
        let abs_path = if let Ok(p) = std::fs::canonicalize(path_str) {
            p
        } else {
            std::path::Path::new(path_str).to_path_buf()
        };
        location_table.set(
            "PKGLUADIR",
            abs_path
                .parent()
                .unwrap_or(&abs_path)
                .to_string_lossy()
                .to_string()
        )?;
    }
    if let Some(bd) = build_dir {
        location_table.set("BUILDDIR", bd)?;
        lua.globals().set("BUILD_DIR", bd)?;
    }
    if let Some(sd) = staging_dir {
        location_table.set("STAGINGDIR", sd)?;
        lua.globals().set("STAGING_DIR", sd)?;
    }
    zoi_table.set("LOCATION", location_table.clone())?;
    lua.globals().set("LOCATION", location_table)?;

    lua.globals().set("ZOI", zoi_table)?;

    let utils_table = lua.create_table()?;
    lua.globals().set("UTILS", utils_table)?;

    api::http::add_fetch_util(lua)?;
    api::parse::add_parse_util(lua)?;
    api::http::add_git_fetch_util(lua)?;
    api::download::add_download_util(lua, quiet)?;
    api::fs::add_file_util(lua, quiet)?;
    api::fs::add_zcp(lua)?;
    api::fs::add_zlicense(lua)?;
    api::fs::add_zdoc(lua)?;
    api::fs::add_zman(lua)?;
    api::fs::add_zshell(lua)?;
    api::fs::add_zsed(lua, quiet)?;
    api::fs::add_zln(lua)?;
    api::fs::add_zchmod(lua)?;
    api::fs::add_zchown(lua)?;
    api::fs::add_zmkdir(lua)?;
    api::crypto::add_verify_hash(lua, quiet)?;
    api::fs::add_zrm(lua)?;
    api::system::add_cmd_util(lua, quiet)?;
    api::system::add_zpatch(lua, quiet)?;
    api::fs::add_fs_util(lua)?;
    api::fs::add_find_util(lua)?;
    api::archive::add_archive_util(lua)?;
    api::archive::add_extract_util(lua, quiet)?;
    api::crypto::add_verify_signature(lua, quiet)?;
    api::crypto::add_add_pgp_key(lua, quiet)?;
    api::lifecycle::add_package_lifecycle_functions(lua)?;

    if let Some(path_str) = file_path {
        let path = std::path::Path::new(path_str);
        api::lifecycle::add_import_util(lua, path)?;
        api::lifecycle::add_include_util(lua, path)?;
    }

    if let Some(sub) = sub_package {
        lua.globals().set("SUBPKG", sub)?;
    }

    if mode == LuaEnvMode::Metadata {
        install_metadata_stubs(lua)?;
        enable_metadata_sandbox(lua)?;
    }

    Ok(())
}

/// Creates a Lua function that always fails with a message explaining that
/// `name` cannot be used while metadata is evaluated.
fn metadata_stub(lua: &Lua, name: &str) -> Result<mlua::Function, mlua::Error> {
    let message = format!(
        "'{name}' is not available during metadata evaluation. Call it from a \
         lifecycle function such as prepare(), build() or package() instead."
    );
    lua.create_function(move |_, _: mlua::MultiValue| -> mlua::Result<()> {
        Err(mlua::Error::RuntimeError(message.clone()))
    })
}

/// Replaces every API with host side effects by a failing stub.
fn install_metadata_stubs(lua: &Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();
    for name in METADATA_BLOCKED_GLOBALS {
        globals.set(*name, metadata_stub(lua, name)?)?;
    }

    let utils_table: Table = globals.get("UTILS")?;
    for name in METADATA_BLOCKED_UTILS {
        utils_table
            .set(*name, metadata_stub(lua, &format!("UTILS.{name}"))?)?;
    }
    if let Ok(fs_table) = utils_table.get::<Table>("FS") {
        for name in METADATA_BLOCKED_FS {
            fs_table
                .set(*name, metadata_stub(lua, &format!("UTILS.FS.{name}"))?)?;
        }
    }
    Ok(())
}

/// Enables the Luau sandbox for metadata evaluation.
///
/// `Lua::sandbox(true)` freezes every table stored directly in the global
/// environment (and the builtin libraries) and gives the script a fresh,
/// writable environment that proxies reads to the frozen globals. The tables
/// scripts are expected to mutate (`PKG` and Zoi's `__Zoi*` capture tables)
/// are therefore moved out of the frozen globals and re-created in the
/// writable environment, where both the script and Zoi's registration
/// functions (which look them up through `lua.globals()`) find them.
fn enable_metadata_sandbox(lua: &Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();
    let mut mutable: Vec<(&str, Value)> = Vec::new();
    for &name in METADATA_MUTABLE_GLOBALS {
        let value: Value = globals.raw_get(name)?;
        if !value.is_nil() {
            globals.raw_set(name, Value::Nil)?;
            mutable.push((name, value));
        }
    }

    lua.sandbox(true)?;

    let env = lua.globals();
    for (name, value) in mutable {
        env.set(name, value)?;
    }
    Ok(())
}
