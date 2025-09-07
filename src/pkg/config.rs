use crate::pkg::types::{Config, RepoConfig};
use colored::*;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
        let default_config = Config {
            repos: vec!["core".to_string(), "main".to_string(), "extra".to_string()],
            package_managers: None,
            native_package_manager: None,
            telemetry_enabled: false,
            registry: Some("https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git".to_string()),
            git_repos: Vec::new(),
            rollback_enabled: true,
        };
        write_config(&default_config)?;
        return Ok(default_config);
    }

    let content = fs::read_to_string(config_path)?;
    let mut config: Config = serde_yaml::from_str(&content)?;

    let mut needs_update = if config.registry.is_none() {
        config.registry = Some("https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git".to_string());
        true
    } else {
        false
    };

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

    if !config.repos.contains(&"core".to_string()) {
        config.repos.insert(0, "core".to_string());
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
    if !db_root.exists() {
        return Ok(Vec::new());
    }

    let all_repos: Vec<String> = fs::read_dir(db_root)?
        .filter_map(Result::ok)
        .filter(|entry| {
            let path = entry.path();
            let file_name = entry.file_name();
            path.is_dir() && file_name != ".git"
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    Ok(all_repos)
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

pub fn set_registry(url: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    config.registry = Some(url.to_string());
    write_config(&config)
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
