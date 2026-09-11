use std::{env, fs};

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::types::SourceType;

/// Implements "Direct Repository Installation" (`zoi install --repo`).
///
/// This allows Zoi to install a package directly from a Git repository
/// that contains a `zoi.lua` project file declaring its installable
/// package via `package("<source>")`. It:
/// - Detects the Git provider (GitHub, GitLab, Codeberg).
/// - Fetches the project configuration over HTTP (RAW URL).
/// - Resolves and installs the specific package defined in the project.
use crate::pkg::types;

/// Installs a package directly from a Git repository using its `zoi.lua`.
///
/// The repository root must contain a `zoi.lua` file that declares the
/// installable package source with a top-level `package("<source>")` call.
///
/// # Errors
///
/// This function will return an error if:
/// - The repository specification is invalid.
/// - The repository does not contain a supported project file (`zoi.lua`).
/// - The package installation fails.
/// - Network issues occur while fetching repository content.
/// # Errors
///
/// Returns an error if the repository installation fails.
pub fn run(
    repo_spec: &str,
    force: bool,
    all_optional: bool,
    yes: bool,
    scope: Option<crate::cli::SetupScope>,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>
) -> Result<()> {
    println!(
        "Installing from git repository: {}",
        repo_spec.cyan().bold()
    );

    let (provider, repo_path) = parse_repo_spec(repo_spec)?;

    crate::pkg::utils::confirm_untrusted_source(
        &SourceType::GitRepo(repo_spec.to_string()),
        yes
    )?;

    let repo_file_names = ["zoi.lua"];
    let mut repo_file_content: Option<String> = None;
    let mut used_url = String::new();

    for file_name in &repo_file_names {
        if let Ok(url) = get_repo_file_url(&provider, &repo_path, file_name) {
            println!("Attempting to fetch repo config from: {url}");
            let client = crate::pkg::utils::get_http_client().ok();
            if let Some(c) = client
                && let Ok(content_res) = c.get(&url).send()
                && content_res.status().is_success()
            {
                repo_file_content = Some(content_res.text()?);
                used_url = url;
                break;
            }
        }
    }

    let repo_file_content = repo_file_content.ok_or_else(|| {
        anyhow!(
            "Could not find zoi.lua in the repository on main/master branches."
        )
    })?;
    println!("Using repo config from: {}", used_url.cyan());

    let package_source = read_repo_package_source(&repo_file_content)?;

    let scope_override = scope.map(|s| match s {
        crate::cli::SetupScope::User => types::Scope::User,
        crate::cli::SetupScope::System => types::Scope::System
    });

    println!("Starting installation of package from git repo...");

    let source_to_install = if package_source.starts_with("http") {
        println!("Package source is a URL: {}", package_source.cyan());
        let client = crate::pkg::utils::get_http_client()?;
        let pkg_content = client.get(package_source).send()?.text()?;
        let temp_path = env::temp_dir().join(format!(
            "zoi-repo-install-{}.pkg.lua",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(&temp_path, pkg_content)?;
        temp_path
            .to_str()
            .ok_or_else(|| anyhow!("Temporary path contains invalid UTF-8"))?
            .to_string()
    } else if package_source.ends_with(".pkg.lua")
        || (package_source.contains('/') && !package_source.starts_with('@'))
    {
        println!(
            "Package source is a path in the repo: {}",
            package_source.cyan()
        );
        let pkg_url =
            get_repo_file_url(&provider, &repo_path, &package_source)?;
        let client = crate::pkg::utils::get_http_client()?;
        let pkg_content = client.get(&pkg_url).send()?.text()?;
        let temp_path = env::temp_dir().join(format!(
            "zoi-repo-install-{}.pkg.lua",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(&temp_path, pkg_content)?;
        temp_path
            .to_str()
            .ok_or_else(|| anyhow!("Temporary path contains invalid UTF-8"))?
            .to_string()
    } else {
        println!(
            "Package source is a package name: {}",
            package_source.cyan()
        );
        package_source.clone()
    };

    crate::cmd::install::run(
        &[source_to_install],
        None,
        force,
        all_optional,
        yes,
        scope_override.map(|s| match s {
            types::Scope::User => crate::cli::InstallScope::User,
            types::Scope::System => crate::cli::InstallScope::System,
            types::Scope::Project => crate::cli::InstallScope::Project
        }),
        false,
        false,
        false,
        false,
        false,
        None,
        false,
        plugin_manager,
        false,
        false,
        false,
        false,
        3,
        false,
        false,
        None
    )?;

    Ok(())
}

/// Reads the installable package source declared by a repository's `zoi.lua`
/// via a top-level `package("<source>")` call.
///
/// The script runs in a minimal Lua VM exposing only the `package`
/// declaration; every other global behaves like vanilla Lua.
///
/// # Errors
///
/// Returns an error if the script cannot be executed or declares no package.
fn read_repo_package_source(content: &str) -> Result<String> {
    let lua = mlua::Lua::new();
    let declared = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let declared_clone = declared.clone();
    let package_fn = lua
        .create_function(move |_, source: String| {
            *declared_clone.lock().expect("mutex poisoned") = Some(source);
            Ok(())
        })
        .map_err(|e| anyhow!("Failed to set up repo config reader: {e}"))?;
    lua.globals()
        .set("package", package_fn)
        .map_err(|e| anyhow!("Failed to set up repo config reader: {e}"))?;
    lua.load(content)
        .exec()
        .map_err(|e| anyhow!("Failed to execute repo zoi.lua: {e}"))?;
    declared
        .lock()
        .expect("mutex poisoned")
        .clone()
        .ok_or_else(|| {
            anyhow!(
                "Repository zoi.lua declares no package. Add a top-level \
                 `package(\"<source>\")` call."
            )
        })
}

/// Parses a repository specification string into a provider and path.
fn parse_repo_spec(spec: &str) -> Result<(String, String)> {
    if let Some((provider_alias, path)) = spec.split_once(':') {
        let provider = match provider_alias {
            "gh" | "github" => "github",
            "gl" | "gitlab" => "gitlab",
            "cb" | "codeberg" => "codeberg",
            _ => {
                return Err(anyhow!(
                    "Unknown provider alias: {provider_alias}"
                ));
            }
        };
        Ok((provider.to_string(), path.to_string()))
    } else {
        Ok(("github".to_string(), spec.to_string()))
    }
}

/// Gets the URL for a file in a repository by checking common branches.
fn get_repo_file_url(
    provider: &str,
    repo_path: &str,
    file_path: &str
) -> Result<String> {
    let branches = ["main", "master"];
    let client = crate::pkg::utils::get_http_client()?;
    for branch in &branches {
        let url = match provider {
            "github" => format!(
                "https://raw.githubusercontent.com/{repo_path}/refs/heads/{branch}/{file_path}"
            ),
            "gitlab" => format!(
                "https://gitlab.com/{repo_path}/-/raw/{branch}/{file_path}"
            ),
            "codeberg" => format!(
                "https://codeberg.org/{repo_path}/raw/branch/{branch}/{file_path}"
            ),
            _ => return Err(anyhow!("Unknown provider")),
        };

        let res = client.get(&url).send();
        if let Ok(response) = res
            && response.status().is_success()
        {
            return Ok(url);
        }
    }
    Err(anyhow!(
        "Could not find '{file_path}' in repo '{repo_path}' on branches main \
         or master."
    ))
}
