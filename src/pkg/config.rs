use crate::pkg::types::{Config, Registry, RepoConfig};
use anyhow::{Result, anyhow};
use colored::*;
use serde_yaml::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn get_default_registry() -> String {
    env!("ZOI_DEFAULT_REGISTRY").to_string()
}

fn get_system_config_path() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        Ok(PathBuf::from("C:\\ProgramData\\zoi\\config.yaml"))
    } else {
        Ok(PathBuf::from("/etc/zoi/config.yaml"))
    }
}

fn get_user_config_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("config.yaml"))
}

fn get_project_config_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    Ok(current_dir.join(".zoi").join("pkgs").join("config.yaml"))
}

fn get_db_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn get_git_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("git"))
}

fn read_yaml_value(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    let content = fs::read_to_string(path)?;
    serde_yaml::from_str(&content).map_err(Into::into)
}

fn read_config_from_path(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(path)?;
    serde_yaml::from_str(&content).map_err(Into::into)
}

pub fn read_config() -> Result<Config> {
    let system_val = read_yaml_value(&get_system_config_path()?)?;
    let user_val = read_yaml_value(&get_user_config_path()?)?;
    let project_val = read_yaml_value(&get_project_config_path()?)?;

    let system_cfg: Config = serde_yaml::from_value(system_val.clone()).unwrap_or_default();
    let user_cfg: Config = serde_yaml::from_value(user_val.clone()).unwrap_or_default();
    let project_cfg: Config = serde_yaml::from_value(project_val.clone()).unwrap_or_default();

    let system_policy = system_cfg.policy.clone();
    let mut merged_cfg = Config {
        policy: system_policy.clone(),
        ..Default::default()
    };

    merged_cfg.repos = system_cfg.repos;
    if !system_policy.repos_unoverridable {
        merged_cfg.repos.extend(user_cfg.repos);
        merged_cfg.repos.extend(project_cfg.repos);
    }
    merged_cfg.repos.sort();
    merged_cfg.repos.dedup();

    merged_cfg.added_registries = system_cfg.added_registries;
    if !system_policy.added_registries_unoverridable {
        merged_cfg
            .added_registries
            .extend(user_cfg.added_registries);
        merged_cfg
            .added_registries
            .extend(project_cfg.added_registries);
    }
    let mut seen_registries = HashSet::new();
    merged_cfg
        .added_registries
        .retain(|r| seen_registries.insert(r.url.clone()));

    merged_cfg.git_repos = system_cfg.git_repos;
    if !system_policy.git_repos_unoverridable {
        merged_cfg.git_repos.extend(user_cfg.git_repos);
        merged_cfg.git_repos.extend(project_cfg.git_repos);
    }
    merged_cfg.git_repos.sort();
    merged_cfg.git_repos.dedup();

    merged_cfg.package_managers = project_cfg
        .package_managers
        .or(user_cfg.package_managers)
        .or(system_cfg.package_managers);
    merged_cfg.native_package_manager = project_cfg
        .native_package_manager
        .or(user_cfg.native_package_manager)
        .or(system_cfg.native_package_manager);
    merged_cfg.registry = project_cfg
        .registry
        .or(user_cfg.registry)
        .or(system_cfg.registry);

    if project_val.get("telemetry_enabled").is_some()
        && !system_policy.telemetry_enabled_unoverridable
    {
        merged_cfg.telemetry_enabled = project_cfg.telemetry_enabled;
    } else if user_val.get("telemetry_enabled").is_some()
        && !system_policy.telemetry_enabled_unoverridable
    {
        merged_cfg.telemetry_enabled = user_cfg.telemetry_enabled;
    } else {
        merged_cfg.telemetry_enabled = system_cfg.telemetry_enabled;
    }

    if project_val.get("rollback_enabled").is_some()
        && !system_policy.rollback_enabled_unoverridable
    {
        merged_cfg.rollback_enabled = project_cfg.rollback_enabled;
    } else if user_val.get("rollback_enabled").is_some()
        && !system_policy.rollback_enabled_unoverridable
    {
        merged_cfg.rollback_enabled = user_cfg.rollback_enabled;
    } else {
        merged_cfg.rollback_enabled = system_cfg.rollback_enabled;
    }

    if project_val.get("default_registry").is_some()
        && !system_policy.default_registry_unoverridable
    {
        merged_cfg.default_registry = project_cfg.default_registry;
    } else if user_val.get("default_registry").is_some()
        && !system_policy.default_registry_unoverridable
    {
        merged_cfg.default_registry = user_cfg.default_registry;
    } else {
        merged_cfg.default_registry = system_cfg.default_registry;
    }

    if project_val.get("parallel_jobs").is_some() && !system_policy.telemetry_enabled_unoverridable
    {
        merged_cfg.parallel_jobs = project_cfg.parallel_jobs;
    } else if user_val.get("parallel_jobs").is_some()
        && !system_policy.telemetry_enabled_unoverridable
    {
        merged_cfg.parallel_jobs = user_cfg.parallel_jobs;
    } else {
        merged_cfg.parallel_jobs = system_cfg.parallel_jobs;
    }

    if let Some(url) = merged_cfg.registry.take()
        && merged_cfg.default_registry.is_none()
    {
        merged_cfg.default_registry = Some(Registry {
            handle: String::new(),
            url,
        });
    }

    if merged_cfg.default_registry.is_none() {
        merged_cfg.default_registry = Some(Registry {
            handle: "zoidberg".to_string(),
            url: get_default_registry(),
        });
    }

    if merged_cfg.repos.is_empty()
        && let Some(reg) = &merged_cfg.default_registry
        && !reg.handle.is_empty()
    {
        let db_root = get_db_root()?;
        let repo_path = db_root.join(&reg.handle);
        if repo_path.join("repo.yaml").exists()
            && let Ok(repo_config) = read_repo_config(&repo_path)
        {
            merged_cfg.repos = repo_config
                .repos
                .into_iter()
                .filter(|r| r.active)
                .map(|r| r.name)
                .collect();
        }
    }

    Ok(merged_cfg)
}

pub fn write_user_config(config: &Config) -> Result<()> {
    let config_path = get_user_config_path()?;
    let parent_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid config path"))?;
    fs::create_dir_all(parent_dir)?;
    let content = serde_yaml::to_string(config)?;
    fs::write(config_path, content)?;
    Ok(())
}

pub fn add_repo(repo_name: &str) -> Result<()> {
    let mut config = read_config_from_path(&get_user_config_path()?)?;
    let lower_repo_name = repo_name.to_lowercase();
    if config.repos.contains(&lower_repo_name) {
        return Err(anyhow!(
            "Repository '{}' already exists in user config.",
            repo_name
        ));
    }
    config.repos.push(lower_repo_name);
    write_user_config(&config)
}

pub fn remove_repo(repo_name: &str) -> Result<()> {
    let mut config = read_config_from_path(&get_user_config_path()?)?;
    let lower_repo_name = repo_name.to_lowercase();
    if let Some(pos) = config.repos.iter().position(|r| r == &lower_repo_name) {
        config.repos.remove(pos);
        write_user_config(&config)
    } else {
        Err(anyhow!(
            "Repository '{}' not found in user config.",
            repo_name
        ))
    }
}

pub fn interactive_add_repo() -> Result<()> {
    let config = read_config()?;
    let all_repos = get_all_repos()?;

    let available_repos: Vec<_> = all_repos
        .into_iter()
        .filter(|r| !config.repos.contains(&r.to_lowercase()))
        .collect();

    if available_repos.is_empty() {
        println!("{}", "No new repositories available to add.".yellow());
        return Ok(());
    }

    println!("{}", "Available repositories to add:".green());
    for (i, repo) in available_repos.iter().enumerate() {
        println!("[{}] {}", i + 1, repo);
    }

    print!(
        "\n{}",
        "Select a repository to add (or 'q' to quit): ".yellow()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "q" {
        println!("Aborted.");
        return Ok(());
    }

    let choice: usize = match input.parse() {
        Ok(num) => num,
        Err(_) => return Err(anyhow!("Invalid input.")),
    };

    if choice > 0 && choice <= available_repos.len() {
        let repo_to_add = &available_repos[choice - 1];
        add_repo(repo_to_add)?;
        println!("Repository '{}' added successfully.", repo_to_add.green());
    } else {
        return Err(anyhow!("Invalid selection."));
    }

    Ok(())
}

pub fn get_all_repos() -> Result<Vec<String>> {
    let db_root = get_db_root()?;
    let config = read_config()?;

    if let Some(default_reg) = config.default_registry
        && !default_reg.handle.is_empty()
    {
        let default_reg_path = db_root.join(default_reg.handle);
        if default_reg_path.join("repo.yaml").exists() {
            let repo_config = read_repo_config(&default_reg_path)?;
            return Ok(repo_config.repos.into_iter().map(|r| r.name).collect());
        }
    }

    Ok(Vec::new())
}

pub fn clone_git_repo(url: &str) -> Result<()> {
    let git_root = get_git_root()?;
    fs::create_dir_all(&git_root)?;
    let repo_name = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or("repo")
        .trim_end_matches(".git");
    let target = git_root.join(repo_name);
    if target.exists() {
        return Err(anyhow!(
            "Git repo '{}' already exists at {}",
            repo_name,
            target.display()
        ));
    }
    println!("Cloning '{}' into {}...", url.cyan(), target.display());
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(&target)
        .status()?;
    if !status.success() {
        return Err(anyhow!("git clone failed"));
    }

    let mut config = read_config_from_path(&get_user_config_path()?)?;
    if !config.git_repos.iter().any(|repo_url| repo_url == url) {
        config.git_repos.push(url.to_string());
        write_user_config(&config)?;
    }

    println!(
        "Cloned git repo as '{}' (use with '@git/{}/<pkg>')",
        repo_name.green(),
        repo_name
    );
    Ok(())
}

pub fn list_git_repos() -> Result<Vec<String>> {
    let git_root = get_git_root()?;
    if !git_root.exists() {
        return Ok(Vec::new());
    }

    let mut repos = Vec::new();
    for entry in fs::read_dir(git_root)? {
        let entry = entry?;
        if entry.path().is_dir() {
            repos.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    repos.sort();
    Ok(repos)
}

pub fn remove_git_repo(repo_name: &str) -> Result<()> {
    let git_root = get_git_root()?;
    let target = git_root.join(repo_name);
    if !target.exists() {
        return Err(anyhow!("Git repository '{}' not found.", repo_name));
    }

    let mut config = read_config_from_path(&get_user_config_path()?)?;
    let mut removed = false;
    config.git_repos.retain(|url| {
        let name_from_url = url
            .trim_end_matches('/')
            .split('/')
            .next_back()
            .unwrap_or("")
            .trim_end_matches(".git");
        if name_from_url == repo_name {
            removed = true;
            false
        } else {
            true
        }
    });

    if removed {
        write_user_config(&config)?;
    }

    fs::remove_dir_all(&target)?;
    println!(
        "Removed git repository '{}' from {}",
        repo_name.green(),
        target.display()
    );
    Ok(())
}

pub fn set_default_registry(url: &str) -> Result<()> {
    let mut config = read_config_from_path(&get_user_config_path()?)?;
    config.default_registry = Some(Registry {
        handle: String::new(),
        url: url.to_string(),
    });
    write_user_config(&config)
}

pub fn add_added_registry(url: &str) -> Result<()> {
    let mut config = read_config_from_path(&get_user_config_path()?)?;
    if config.added_registries.iter().any(|r| r.url == url) {
        return Err(anyhow!("Registry with URL '{}' already exists.", url));
    }
    config.added_registries.push(Registry {
        handle: String::new(),
        url: url.to_string(),
    });
    write_user_config(&config)
}

pub fn remove_added_registry(handle: &str) -> Result<()> {
    let mut config = read_config_from_path(&get_user_config_path()?)?;
    if let Some(pos) = config
        .added_registries
        .iter()
        .position(|r| r.handle == handle)
    {
        config.added_registries.remove(pos);
        let db_root = get_db_root()?;
        let repo_path = db_root.join(handle);
        if repo_path.exists() {
            fs::remove_dir_all(repo_path)?;
        }
        write_user_config(&config)
    } else {
        Err(anyhow!(
            "Added registry with handle '{}' not found.",
            handle
        ))
    }
}

pub fn read_repo_config(db_path: &Path) -> Result<RepoConfig> {
    let config_path = db_path.join("repo.yaml");
    if !config_path.exists() {
        return Err(anyhow!(
            "repo.yaml not found in the root of the package database."
        ));
    }
    let content = fs::read_to_string(config_path)?;
    let config: RepoConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn read_user_config() -> Result<Config> {
    read_config_from_path(&get_user_config_path()?)
}
