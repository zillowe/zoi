use crate::pkg::{install, types};
use anyhow::{Result, anyhow};
use colored::*;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::sync::Mutex;

#[derive(Debug, Deserialize)]
struct RepoFile {
    package: String,
}

pub fn run(
    repo_spec: &str,
    force: bool,
    all_optional: bool,
    yes: bool,
    scope: Option<crate::cli::SetupScope>,
) -> Result<()> {
    println!(
        "Installing from git repository: {}",
        repo_spec.cyan().bold()
    );

    let (provider, repo_path) = parse_repo_spec(repo_spec)?;

    let repo_file_names = ["zoi.yaml"];
    let mut repo_file_content: Option<String> = None;
    let mut used_url = String::new();

    for file_name in &repo_file_names {
        if let Ok(url) = get_repo_file_url(&provider, &repo_path, file_name) {
            println!("Attempting to fetch repo config from: {}", url);
            if let Ok(content_res) = reqwest::blocking::get(&url)
                && content_res.status().is_success()
            {
                repo_file_content = Some(content_res.text()?);
                used_url = url;
                break;
            }
        }
    }

    let repo_file_content = repo_file_content.ok_or_else(|| {
        anyhow!("Could not find zoi.yaml in the repository on main/master branches.")
    })?;
    println!("Using repo config from: {}", used_url.cyan());

    let repo_file: RepoFile = serde_yaml::from_str(&repo_file_content)?;

    let package_source = &repo_file.package;

    let mode = install::InstallMode::PreferPrebuilt;

    let scope_override = scope.map(|s| match s {
        crate::cli::SetupScope::User => types::Scope::User,
        crate::cli::SetupScope::System => types::Scope::System,
    });

    let processed_deps = Mutex::new(HashSet::new());

    println!("Starting installation of package from git repo...");

    if package_source.starts_with("http") {
        println!("Package source is a URL: {}", package_source.cyan());
        let pkg_content = reqwest::blocking::get(package_source)?.text()?;
        let temp_path = env::temp_dir().join(format!(
            "zoi-repo-install-{}.pkg.lua",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(&temp_path, pkg_content)?;
        let result = install::run_installation(
            temp_path.to_str().unwrap(),
            mode,
            force,
            types::InstallReason::Direct,
            yes,
            all_optional,
            &processed_deps,
            scope_override,
            None,
        );
        fs::remove_file(temp_path)?;
        result
    } else if package_source.ends_with(".pkg.lua")
        || (package_source.contains('/') && !package_source.starts_with('@'))
    {
        println!(
            "Package source is a path in the repo: {}",
            package_source.cyan()
        );
        let pkg_url = get_repo_file_url(&provider, &repo_path, package_source)?;
        let pkg_content = reqwest::blocking::get(&pkg_url)?.text()?;
        let temp_path = env::temp_dir().join(format!(
            "zoi-repo-install-{}.pkg.lua",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(&temp_path, pkg_content)?;
        let result = install::run_installation(
            temp_path.to_str().unwrap(),
            mode,
            force,
            types::InstallReason::Direct,
            yes,
            all_optional,
            &processed_deps,
            scope_override,
            None,
        );
        fs::remove_file(temp_path)?;
        result
    } else {
        println!(
            "Package source is a package name: {}",
            package_source.cyan()
        );
        install::run_installation(
            package_source,
            mode,
            force,
            types::InstallReason::Direct,
            yes,
            all_optional,
            &processed_deps,
            scope_override,
            None,
        )
    }
}

fn parse_repo_spec(spec: &str) -> Result<(String, String)> {
    if let Some((provider_alias, path)) = spec.split_once(':') {
        let provider = match provider_alias {
            "gh" | "github" => "github",
            "gl" | "gitlab" => "gitlab",
            "cb" | "codeberg" => "codeberg",
            _ => return Err(anyhow!("Unknown provider alias: {}", provider_alias)),
        };
        Ok((provider.to_string(), path.to_string()))
    } else {
        Ok(("github".to_string(), spec.to_string()))
    }
}

fn get_repo_file_url(provider: &str, repo_path: &str, file_path: &str) -> Result<String> {
    let branches = ["main", "master"];
    for branch in &branches {
        let url = match provider {
            "github" => format!(
                "https://raw.githubusercontent.com/{}/refs/heads/{}/{}",
                repo_path, branch, file_path
            ),
            "gitlab" => format!(
                "https://gitlab.com/{}/-/raw/{}/{}",
                repo_path, branch, file_path
            ),
            "codeberg" => format!(
                "https://codeberg.org/{}/raw/branch/{}/{}",
                repo_path, branch, file_path
            ),
            _ => return Err(anyhow!("Unknown provider")),
        };

        let res = reqwest::blocking::get(&url);
        if let Ok(response) = res
            && response.status().is_success()
        {
            return Ok(url);
        }
    }
    Err(anyhow!(
        "Could not find '{}' in repo '{}' on branches main or master.",
        file_path,
        repo_path
    ))
}
