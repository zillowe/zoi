use crate::utils;
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use serde::Deserialize;
use std::{fs, path::Path};
use urlencoding;

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

#[derive(Deserialize)]
struct GitArgs {
    repo: String,
    domain: Option<String>,
    branch: Option<String>,
}

fn fetch_json(url: &str) -> Result<serde_json::Value, mlua::Error> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("zoi")
        .build()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(mlua::Error::RuntimeError(format!(
            "Request to {} failed with status: {} and body: {}",
            url,
            response.status(),
            response.text().unwrap_or_else(|_| "N/A".to_string())
        )));
    }

    let text = response
        .text()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| mlua::Error::RuntimeError(e.to_string()))
}

fn add_git_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let utils_table: Table = lua.globals().get("UTILS")?;
    let fetch_table: Table = utils_table.get("FETCH")?;

    for provider in ["GITHUB", "GITLAB", "GITEA", "FORGEJO"] {
        let provider_table = lua.create_table()?;
        let latest_table = lua.create_table()?;

        for what in ["tag", "release", "commit"] {
            let get_latest_fn = lua.create_function(move |lua, args: Table| {
                let git_args: GitArgs = lua
                    .from_value(Value::Table(args))
                    .map_err(|e| mlua::Error::RuntimeError(format!("Invalid arguments: {}", e)))?;

                let base_url = match provider {
                    "GITHUB" => git_args
                        .domain
                        .unwrap_or_else(|| "https://api.github.com".to_string()),
                    "GITLAB" => git_args
                        .domain
                        .unwrap_or_else(|| "https://gitlab.com".to_string()),
                    "GITEA" => git_args
                        .domain
                        .unwrap_or_else(|| "https://gitea.com".to_string()),
                    "FORGEJO" => git_args
                        .domain
                        .unwrap_or_else(|| "https://codeberg.org".to_string()),
                    _ => unreachable!(),
                };

                let url = match (provider, what) {
                    ("GITHUB", "tag") => format!("{}/repos/{}/tags", base_url, git_args.repo),
                    ("GITHUB", "release") => {
                        format!("{}/repos/{}/releases/latest", base_url, git_args.repo)
                    }
                    ("GITHUB", "commit") => format!(
                        "{}/repos/{}/commits?sha={}",
                        base_url,
                        git_args.repo,
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),

                    ("GITLAB", "tag") => format!(
                        "{}/api/v4/projects/{}/repository/tags",
                        base_url,
                        urlencoding::encode(&git_args.repo)
                    ),
                    ("GITLAB", "release") => format!(
                        "{}/api/v4/projects/{}/releases",
                        base_url,
                        urlencoding::encode(&git_args.repo)
                    ),
                    ("GITLAB", "commit") => format!(
                        "{}/api/v4/projects/{}/repository/commits?ref_name={}",
                        base_url,
                        urlencoding::encode(&git_args.repo),
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),

                    ("GITEA" | "FORGEJO", "tag") => {
                        format!("{}/api/v1/repos/{}/tags", base_url, git_args.repo)
                    }
                    ("GITEA" | "FORGEJO", "release") => {
                        format!(
                            "{}/api/v1/repos/{}/releases/latest",
                            base_url, git_args.repo
                        )
                    }
                    ("GITEA" | "FORGEJO", "commit") => format!(
                        "{}/api/v1/repos/{}/commits?sha={}",
                        base_url,
                        git_args.repo,
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),
                    _ => unreachable!(),
                };

                let json = fetch_json(&url)?;

                let result = match (provider, what) {
                    ("GITHUB", "tag") | ("GITEA", "tag") | ("FORGEJO", "tag") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|t| t["name"].as_str()),
                    ("GITHUB", "release") | ("GITEA", "release") | ("FORGEJO", "release") => {
                        json["tag_name"].as_str()
                    }
                    ("GITHUB", "commit") | ("GITEA", "commit") | ("FORGEJO", "commit") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|c| c["sha"].as_str()),

                    ("GITLAB", "tag") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|t| t["name"].as_str()),
                    ("GITLAB", "release") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|r| r["tag_name"].as_str()),
                    ("GITLAB", "commit") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|c| c["id"].as_str()),
                    _ => unreachable!(),
                };

                result.map(|s| s.to_string()).ok_or_else(|| {
                    mlua::Error::RuntimeError(
                        "Could not extract value from API response".to_string(),
                    )
                })
            })?;
            latest_table.set(what, get_latest_fn)?;
        }

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
    let import_fn = lua.create_function(move |lua, file_name: String| {
        let path = current_path_buf.parent().unwrap().join(&file_name);
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
