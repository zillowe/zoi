use super::{config, executor};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

pub fn run(cmd_alias: Option<&str>, args: &[String], config: &config::ProjectConfig) -> Result<()> {
    if config.commands.is_empty() {
        return Err(anyhow!("No commands defined in zoi.yaml"));
    }

    let target_alias = match cmd_alias {
        Some(alias) => alias.to_string(),
        None => {
            if !args.is_empty() {
                return Err(anyhow!("Cannot pass arguments when in interactive mode."));
            }
            let selections: Vec<&str> = config.commands.iter().map(|c| c.cmd.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose a command to run")
                .items(&selections)
                .default(0)
                .interact_opt()?
                .ok_or(anyhow!("No command chosen."))?;

            config.commands[selection].cmd.clone()
        }
    };

    let platform = utils::get_platform()?;
    let mut resolved_tasks = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();

    resolve_task_dependencies(
        &target_alias,
        config,
        &mut resolved_tasks,
        &mut visited,
        &mut stack,
    )?;

    let stages = group_tasks_into_stages(&resolved_tasks, config)?;

    for stage in stages {
        stage.into_par_iter().try_for_each(|task_alias| {
            let cmd_spec = config
                .commands
                .iter()
                .find(|c| c.cmd == task_alias)
                .ok_or_else(|| anyhow!("Task '{}' not found", task_alias))?;

            let current_hash = if let Some(files) = &cmd_spec.cache_files {
                Some(calculate_files_hash(files)?)
            } else {
                None
            };

            if let Some(hash) = &current_hash
                && is_task_cached(&task_alias, hash)?
            {
                println!(
                    "{} Task '{}' is up to date. Skipping.",
                    "::".bold().green(),
                    task_alias
                );
                return Ok(());
            }

            run_single_command(
                cmd_spec,
                if task_alias == target_alias {
                    args
                } else {
                    &[]
                },
                &platform,
            )?;

            if let Some(hash) = &current_hash {
                update_task_cache(&task_alias, hash)?;
            }

            Ok::<(), anyhow::Error>(())
        })?;
    }

    Ok(())
}

fn resolve_task_dependencies(
    alias: &str,
    config: &config::ProjectConfig,
    resolved: &mut Vec<String>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> Result<()> {
    if stack.contains(alias) {
        return Err(anyhow!("Circular dependency detected in tasks: {}", alias));
    }
    if visited.contains(alias) {
        return Ok(());
    }

    stack.insert(alias.to_string());

    let cmd_spec = config
        .commands
        .iter()
        .find(|c| c.cmd == alias)
        .ok_or_else(|| anyhow!("Command alias '{}' not found in zoi.yaml", alias))?;

    if let Some(deps) = &cmd_spec.depends_on {
        for dep in deps {
            resolve_task_dependencies(dep, config, resolved, visited, stack)?;
        }
    }

    stack.remove(alias);
    visited.insert(alias.to_string());
    resolved.push(alias.to_string());
    Ok(())
}

fn group_tasks_into_stages(
    resolved_tasks: &[String],
    config: &config::ProjectConfig,
) -> Result<Vec<Vec<String>>> {
    let mut in_degree = HashMap::new();
    let mut adj = HashMap::new();

    for alias in resolved_tasks {
        in_degree.insert(alias.clone(), 0);
    }

    for alias in resolved_tasks {
        let cmd_spec = config
            .commands
            .iter()
            .find(|c| c.cmd == *alias)
            .ok_or_else(|| {
                anyhow!(
                    "Command spec for '{}' disappeared during task grouping",
                    alias
                )
            })?;
        if let Some(deps) = &cmd_spec.depends_on {
            for dep in deps {
                if resolved_tasks.contains(dep) {
                    adj.entry(dep.clone())
                        .or_insert_with(Vec::new)
                        .push(alias.clone());
                    let degree = in_degree
                        .get_mut(alias)
                        .ok_or_else(|| anyhow!("Task '{}' missing from in_degree map", alias))?;
                    *degree += 1;
                }
            }
        }
    }

    let mut stages = Vec::new();
    let mut current_stage: Vec<String> = in_degree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(a, _)| a.clone())
        .collect();

    while !current_stage.is_empty() {
        let mut next_stage = Vec::new();
        for task in &current_stage {
            if let Some(neighbors) = adj.get(task) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).ok_or_else(|| {
                        anyhow!("Neighbor task '{}' missing from in_degree map", neighbor)
                    })?;
                    *degree -= 1;
                    if *degree == 0 {
                        next_stage.push(neighbor.clone());
                    }
                }
            }
        }
        stages.push(current_stage);
        current_stage = next_stage;
    }

    Ok(stages)
}

fn run_single_command(
    command_to_run: &config::CommandSpec,
    args: &[String],
    platform: &str,
) -> Result<()> {
    let run_cmd = match &command_to_run.run {
        config::PlatformOrString::String(s) => s.clone(),
        config::PlatformOrString::Platform(p) => p
            .get(platform)
            .or_else(|| p.get("default"))
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "No command found for platform '{}' and no default specified",
                    platform
                )
            })?,
    };

    let env_vars = match &command_to_run.env {
        config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
        config::PlatformOrEnvMap::Platform(p) => p
            .get(platform)
            .or_else(|| p.get("default"))
            .cloned()
            .unwrap_or_default(),
    };

    println!(
        "{} Running command: {}...",
        "::".bold().blue(),
        command_to_run.cmd.bold()
    );
    let mut full_command = run_cmd;
    if !args.is_empty() {
        full_command.push(' ');
        full_command.push_str(&args.join(" "));
    }
    executor::run_shell_command(&full_command, &env_vars)
}

fn get_task_cache_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let cache_dir = current_dir.join(".zoi").join("cache");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join("tasks.json"))
}

fn read_task_cache() -> Result<HashMap<String, String>> {
    let path = get_task_cache_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

fn is_task_cached(alias: &str, current_hash: &str) -> Result<bool> {
    let cache = read_task_cache()?;
    Ok(cache.get(alias).is_some_and(|h| h == current_hash))
}

fn update_task_cache(alias: &str, hash: &str) -> Result<()> {
    let mut cache = read_task_cache()?;
    cache.insert(alias.to_string(), hash.to_string());
    let path = get_task_cache_path()?;
    let content = serde_json::to_string_pretty(&cache)?;
    fs::write(path, content)?;
    Ok(())
}

fn calculate_files_hash(files: &[String]) -> Result<String> {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    let mut found_any = false;
    for file_glob in files {
        for entry in glob::glob(file_glob)? {
            let path = entry?;
            if path.is_file() {
                let content = fs::read(path)?;
                hasher.update(&content);
                found_any = true;
            }
        }
    }
    if !found_any {
        return Ok("no-files".to_string());
    }
    Ok(hex::encode(hasher.finalize()))
}
