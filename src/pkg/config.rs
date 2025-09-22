use crate::pkg::types::{Config, Registry, RepoConfig};
use colored::*;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn get_default_registry() -> String {
    env!("ZOI_DEFAULT_REGISTRY").to_string()
}

fn get_config_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("config.yaml"))
}

fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn get_git_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("git"))
}

pub fn read_config() -> Result<Config, Box<dyn Error>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        let db_path = get_db_root()?;
        let default_repos = if db_path.join("repo.yaml").exists() {
            let repo_config = read_repo_config(&db_path)?;
            repo_config
                .repos
                .into_iter()
                .filter(|r| r.active)
                .map(|r| r.name)
                .collect()
        } else {
            Vec::new()
        };

        let default_config = Config {
            repos: default_repos,
            package_managers: None,
            native_package_manager: None,
            telemetry_enabled: false,
            registry: None,
            default_registry: Some(Registry {
                handle: "zoidberg".to_string(),
                url: get_default_registry(),
            }),
            added_registries: Vec::new(),
            git_repos: Vec::new(),
            rollback_enabled: true,
        };
        write_config(&default_config)?;
        return Ok(default_config);
    }

    let content = fs::read_to_string(config_path)?;
    let mut config: Config = serde_yaml::from_str(&content)?;

    let mut needs_update = false;
    if let Some(url) = config.registry.take() {
        if config.default_registry.is_none() {
            config.default_registry = Some(Registry {
                handle: String::new(),
                url,
            });
        }
        needs_update = true;
    }

    if config.default_registry.is_none() {
        config.default_registry = Some(Registry {
            handle: "zoidberg".to_string(),
            url: get_default_registry(),
        });
        needs_update = true;
    }

    let original_repos = config.repos.clone();

    let mut new_repos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for repo in &original_repos {
        let lower = repo.to_lowercase();
        if seen.insert(lower.clone()) {
            new_repos.push(lower);
        }
    }
    config.repos = new_repos;

    if config.repos != original_repos {
        needs_update = true;
    }

    if needs_update {
        write_config(&config)?;
    }

    Ok(config)
}

pub fn write_config(config: &Config) -> Result<(), Box<dyn Error>> {
    let config_path = get_config_path()?;
    let parent_dir = config_path.parent().ok_or("Invalid config path")?;
    fs::create_dir_all(parent_dir)?;
    let content = serde_yaml::to_string(config)?;
    fs::write(config_path, content)?;
    Ok(())
}

pub fn add_repo(repo_name: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    let lower_repo_name = repo_name.to_lowercase();
    if config.repos.contains(&lower_repo_name) {
        return Err(format!("Repository '{}' already exists.", repo_name).into());
    }
    config.repos.push(lower_repo_name);
    write_config(&config)
}

pub fn remove_repo(repo_name: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    let lower_repo_name = repo_name.to_lowercase();
    if let Some(pos) = config.repos.iter().position(|r| r == &lower_repo_name) {
        config.repos.remove(pos);
        write_config(&config)
    } else {
        Err(format!("Repository '{}' not found.", repo_name).into())
    }
}

pub fn interactive_add_repo() -> Result<(), Box<dyn Error>> {
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
        Err(_) => return Err("Invalid input.".into()),
    };

    if choice > 0 && choice <= available_repos.len() {
        let repo_to_add = &available_repos[choice - 1];
        add_repo(repo_to_add)?;
        println!("Repository '{}' added successfully.", repo_to_add.green());
    } else {
        return Err("Invalid selection.".into());
    }

    Ok(())
}

pub fn get_all_repos() -> Result<Vec<String>, Box<dyn Error>> {
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

pub fn clone_git_repo(url: &str) -> Result<(), Box<dyn Error>> {
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
        return Err(format!(
            "Git repo '{}' already exists at {}",
            repo_name,
            target.display()
        )
        .into());
    }
    println!("Cloning '{}' into {}...", url.cyan(), target.display());
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(&target)
        .status()?;
    if !status.success() {
        return Err("git clone failed".into());
    }

    let mut config = read_config()?;
    if !config.git_repos.iter().any(|repo_url| repo_url == url) {
        config.git_repos.push(url.to_string());
        write_config(&config)?;
    }

    println!(
        "Cloned git repo as '{}' (use with '@git/{}/<pkg>')",
        repo_name.green(),
        repo_name
    );
    Ok(())
}

pub fn list_git_repos() -> Result<Vec<String>, Box<dyn Error>> {
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

pub fn remove_git_repo(repo_name: &str) -> Result<(), Box<dyn Error>> {
    let git_root = get_git_root()?;
    let target = git_root.join(repo_name);
    if !target.exists() {
        return Err(format!("Git repository '{}' not found.", repo_name).into());
    }

    let mut config = read_config()?;
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
        write_config(&config)?;
    }

    fs::remove_dir_all(&target)?;
    println!(
        "Removed git repository '{}' from {}",
        repo_name.green(),
        target.display()
    );
    Ok(())
}

pub fn set_default_registry(url: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    config.default_registry = Some(Registry {
        handle: String::new(),
        url: url.to_string(),
    });
    write_config(&config)
}

pub fn add_added_registry(url: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    if config.added_registries.iter().any(|r| r.url == url) {
        return Err(format!("Registry with URL '{}' already exists.", url).into());
    }
    config.added_registries.push(Registry {
        handle: String::new(),
        url: url.to_string(),
    });
    write_config(&config)
}

pub fn remove_added_registry(handle: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
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
        write_config(&config)
    } else {
        Err(format!("Added registry with handle '{}' not found.", handle).into())
    }
}

pub fn read_repo_config(db_path: &Path) -> Result<RepoConfig, Box<dyn Error>> {
    let config_path = db_path.join("repo.yaml");
    if !config_path.exists() {
        return Err("repo.yaml not found in the root of the package database.".into());
    }
    let content = fs::read_to_string(config_path)?;
    let config: RepoConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}
