use crate::{pkg::local, utils};
use mlua::{self, Lua, LuaSerdeExt, Table};
use std::{fs, path::Path};

fn add_parse_util(lua: &Lua) -> Result<(), mlua::Error> {
    let parse_table = lua.create_table()?;

    let json_fn = lua.create_function(|lua, json_str: String| {
        let value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("json", json_fn)?;

    let yaml_fn = lua.create_function(|lua, yaml_str: String| {
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("yaml", yaml_fn)?;

    let toml_fn = lua.create_function(|lua, toml_str: String| {
        let value: toml::Value =
            toml::from_str(&toml_str).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("toml", toml_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("PARSE", parse_table)?;

    Ok(())
}

fn add_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let fetch_table = lua.create_table()?;

    let fetch_fn = lua.create_function(|_, url: String| -> Result<String, mlua::Error> {
        let response =
            reqwest::blocking::get(url).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        let text = response
            .text()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        Ok(text)
    })?;
    fetch_table.set("url", fetch_fn)?;

    let utils_table: Table = lua
        .globals()
        .get("UTILS")
        .unwrap_or_else(|_| lua.create_table().unwrap());
    utils_table.set("FETCH", fetch_table)?;
    lua.globals().set("UTILS", utils_table)?;

    Ok(())
}

fn add_git_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let utils_table: Table = lua.globals().get("UTILS")?;
    let fetch_table: Table = utils_table.get("FETCH")?;

    for provider in ["GITHUB", "GITLAB", "GITEA", "FORGEJO"] {
        let provider_table = lua.create_table()?;
        let latest_table = lua.create_table()?;

        let get_latest_tag =
            lua.create_function(move |_, repo: String| -> Result<String, mlua::Error> {
                if provider != "GITHUB" {
                    return Err(mlua::Error::RuntimeError(format!(
                        "{} not implemented",
                        provider
                    )));
                }
                let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
                let client = reqwest::blocking::Client::new();
                let response = client
                    .get(&url)
                    .header("User-Agent", "zoi")
                    .send()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

                let text = response
                    .text()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let json: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

                if let Some(tag) = json["tag_name"].as_str() {
                    Ok(tag.to_string())
                } else {
                    Err(mlua::Error::RuntimeError(
                        "Could not find tag_name in response".to_string(),
                    ))
                }
            })?;

        let get_latest_release =
            lua.create_function(move |_, _: String| -> Result<String, mlua::Error> {
                Err(mlua::Error::RuntimeError(format!(
                    "LATEST.release for {} not implemented",
                    provider
                )))
            })?;

        let get_latest_commit =
            lua.create_function(move |_, _: String| -> Result<String, mlua::Error> {
                Err(mlua::Error::RuntimeError(format!(
                    "LATEST.commit for {} not implemented",
                    provider
                )))
            })?;

        latest_table.set("tag", get_latest_tag)?;
        latest_table.set("release", get_latest_release)?;
        latest_table.set("commit", get_latest_commit)?;
        provider_table.set("LATEST", latest_table)?;
        fetch_table.set(provider, provider_table)?;
    }

    Ok(())
}

fn add_file_util(lua: &Lua) -> Result<(), mlua::Error> {
    let file_fn = lua.create_function(
        |_, (url, path): (String, String)| -> Result<(), mlua::Error> {
            let response = reqwest::blocking::get(url)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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

fn add_import_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let import_fn =
        lua.create_function(move |_, file_name: String| -> Result<String, mlua::Error> {
            let path = current_path_buf.parent().unwrap().join(file_name);
            fs::read_to_string(path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))
        })?;
    lua.globals().set("IMPORT", import_fn)?;
    Ok(())
}

fn add_include_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let include_fn =
        lua.create_function(move |lua, file_name: String| -> Result<(), mlua::Error> {
            let path = current_path_buf.parent().unwrap().join(file_name);
            let code =
                fs::read_to_string(path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            lua.load(&code).exec()?;
            Ok(())
        })?;
    lua.globals().set("INCLUDE", include_fn)?;
    Ok(())
}

pub fn setup_lua_environment(
    lua: &Lua,
    platform: &str,
    version_override: Option<&str>,
    file_path: Option<&str>,
) -> Result<(), mlua::Error> {
    let system_table = lua.create_table()?;
    let parts: Vec<&str> = platform.split('-').collect();
    system_table.set("OS", *parts.first().unwrap_or(&""))?;
    system_table.set("ARCH", *parts.get(1).unwrap_or(&""))?;
    if let Some(distro) = utils::get_linux_distribution() {
        system_table.set("DISTRO", distro)?;
    }
    if let Some(manager) = utils::get_native_package_manager() {
        system_table.set("MANAGER", manager)?;
    }
    lua.globals().set("SYSTEM", system_table)?;

    let zoi_table = lua.create_table()?;
    if let Some(ver) = version_override {
        zoi_table.set("VERSION", ver)?;
    }

    let path_table = lua.create_table()?;
    if let Ok(user_path) = local::get_store_base_dir(crate::pkg::types::Scope::User) {
        path_table.set("user", user_path.to_string_lossy().to_string())?;
    }
    if let Ok(system_path) = local::get_store_base_dir(crate::pkg::types::Scope::System) {
        path_table.set("system", system_path.to_string_lossy().to_string())?;
    }
    zoi_table.set("PATH", path_table)?;
    lua.globals().set("ZOI", zoi_table)?;

    let utils_table = lua.create_table()?;
    lua.globals().set("UTILS", utils_table)?;

    add_fetch_util(lua)?;
    add_parse_util(lua)?;
    add_git_fetch_util(lua)?;
    add_file_util(lua)?;

    if let Some(path_str) = file_path {
        let path = Path::new(path_str);
        add_import_util(lua, path)?;
        add_include_util(lua, path)?;
    }

    Ok(())
}
