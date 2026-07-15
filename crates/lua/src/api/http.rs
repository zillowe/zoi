use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use zoi_core::utils;

use serde::Deserialize;
/// Exposes HTTP and Git-forge utilities to the Lua environment.
///
/// These functions enable dynamic package definitions by allowing them to:
/// - `UTILS.FETCH.url`: Fetch raw text content (e.g. version files, checksums).
/// - `UTILS.FETCH.<FORGE>.LATEST`: Query Git forges (GitHub, GitLab, etc.) for the
///   latest tags, releases, or commit SHAs.
///
/// All network requests respect Zoi's global `--offline` and timeout settings.
pub fn add_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let fetch_table = lua.create_table()?;

    let fetch_fn = lua.create_function(|_, url: String| -> Result<String, mlua::Error> {
        let client =
            utils::get_http_client().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        let response = client
            .get(url)
            .send()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        let text = response
            .text()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        Ok(text)
    })?;
    fetch_table.set("url", fetch_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FETCH", fetch_table)?;

    Ok(())
}

#[derive(Deserialize)]
struct GitArgs {
    repo: String,
    domain: Option<String>,
    branch: Option<String>,
}

fn fetch_json(url: &str) -> Result<serde_json::Value, mlua::Error> {
    let client = utils::get_http_client().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

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

pub fn add_git_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
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
