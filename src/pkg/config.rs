use crate::pkg::types::Config;
use colored::*;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

fn get_config_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("config.yaml"))
}

fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

pub fn read_config() -> Result<Config, Box<dyn Error>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        let default_config = Config {
            repos: vec!["main".to_string(), "extra".to_string()],
        };
        write_config(&default_config)?;
        return Ok(default_config);
    }

    let content = fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&content)?;
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
    if config.repos.contains(&repo_name.to_string()) {
        return Err(format!("Repository '{}' already exists.", repo_name).into());
    }
    config.repos.push(repo_name.to_string());
    write_config(&config)
}

pub fn remove_repo(repo_name: &str) -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    if let Some(pos) = config.repos.iter().position(|r| r == repo_name) {
        config.repos.remove(pos);
        write_config(&config)
    } else {
        Err(format!("Repository '{}' not found.", repo_name).into())
    }
}

pub fn interactive_add_repo() -> Result<(), Box<dyn Error>> {
    let db_root = get_db_root()?;
    let config = read_config()?;

    let all_repos: Vec<String> = fs::read_dir(db_root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    let available_repos: Vec<_> = all_repos
        .into_iter()
        .filter(|r| !config.repos.contains(r))
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
