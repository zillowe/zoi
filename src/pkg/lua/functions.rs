use crate::pkg::lua::api;
use crate::utils;
use mlua::Lua;

pub fn setup_lua_environment(
    lua: &Lua,
    platform: &str,
    version_override: Option<&str>,
    file_path: Option<&str>,
    create_pkg_dir: Option<&str>,
    sub_package: Option<&str>,
    quiet: bool,
) -> Result<(), mlua::Error> {
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

    if let Some(dir) = create_pkg_dir {
        zoi_table.set("CREATE_PKG_DIR", dir)?;
    }

    if let Some(sub) = sub_package {
        lua.globals().set("SUBPKG", sub)?;
    }

    let path_table = lua.create_table()?;
    if let Some(home_dir) = home::home_dir() {
        path_table.set("user", home_dir.join(".zoi").to_string_lossy().to_string())?;
    }

    let system_bin_path = if cfg!(target_os = "windows") {
        "C:\\ProgramData\\zoi\\pkgs\\bin".to_string()
    } else {
        "/usr/local/bin".to_string()
    };
    path_table.set("system", system_bin_path)?;

    zoi_table.set("PATH", path_table)?;

    let pkg_table = lua.create_table()?;
    if let Some(home_dir) = home::home_dir() {
        pkg_table.set("home", home_dir.to_string_lossy().to_string())?;
        pkg_table.set(
            "store",
            home_dir
                .join(".zoi")
                .join("pkgs")
                .join("store")
                .to_string_lossy()
                .to_string(),
        )?;
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
    zoi_table.set("PKG", pkg_table)?;

    lua.globals().set("ZOI", zoi_table)?;

    let utils_table = lua.create_table()?;
    lua.globals().set("UTILS", utils_table)?;

    api::http::add_fetch_util(lua)?;
    api::parse::add_parse_util(lua)?;
    api::http::add_git_fetch_util(lua)?;
    api::fs::add_file_util(lua)?;
    api::fs::add_zcp(lua)?;
    api::fs::add_zlicense(lua)?;
    api::fs::add_zdoc(lua)?;
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

    Ok(())
}
