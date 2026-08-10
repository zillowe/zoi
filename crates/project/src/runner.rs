use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use colored::Colorize;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use rayon::prelude::*;
use zoi_core::utils;

use super::{config, executor};

/// Executes one or more project tasks concurrently with dependency resolution.
///
/// This runner performs:
/// - Dependency Resolution: Builds a DAG of tasks using topological sort.
/// - Stage Grouping: Identifies independent tasks that can run in parallel.
/// - Parallel Execution: Uses `rayon` to execute stages concurrently.
/// - Incremental Builds: Uses file-based hashing (`cache_files`) to skip tasks
///   if their inputs haven't changed.
///
/// # Errors
///
/// Returns an error if no commands are defined, task dependencies cannot be
/// resolved, or if any task execution fails.
pub fn run(
    cmd_alias: Option<&str>,
    args: &[String],
    config: &config::ProjectConfig
) -> Result<()> {
    if config.commands.is_empty() {
        return Err(anyhow!("No commands defined in zoi.yaml"));
    }

    let target_alias = if let Some(alias) = cmd_alias {
        alias.to_string()
    } else {
        if !args.is_empty() {
            return Err(anyhow!(
                "Cannot pass arguments when in interactive mode."
            ));
        }
        let selections: Vec<&str> =
            config.commands.iter().map(|c| c.cmd.as_str()).collect();
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose a command to run")
            .items(&selections)
            .default(0)
            .interact_opt()?
            .ok_or(anyhow!("No command chosen."))?;

        config
            .commands
            .get(selection)
            .ok_or_else(|| anyhow!("Invalid selection"))?
            .cmd
            .clone()
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
        &mut stack
    )?;

    let stages = group_tasks_into_stages(&resolved_tasks, config)?;

    for stage in stages {
        stage.into_par_iter().try_for_each(|task_alias| {
            let cmd_spec = config
                .commands
                .iter()
                .find(|c| c.cmd == task_alias)
                .ok_or_else(|| anyhow!("Task '{task_alias}' not found"))?;

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
                &platform
            )?;

            if let Some(hash) = &current_hash {
                update_task_cache(&task_alias, hash)?;
            }

            Ok::<(), anyhow::Error>(())
        })?;
    }

    Ok(())
}

/// Recursively resolves task dependencies and produces a topologically sorted
/// list.
fn resolve_task_dependencies(
    alias: &str,
    config: &config::ProjectConfig,
    resolved: &mut Vec<String>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>
) -> Result<()> {
    if stack.contains(alias) {
        return Err(anyhow!("Circular dependency detected in tasks: {alias}"));
    }
    if visited.contains(alias) {
        return Ok(());
    }

    stack.insert(alias.to_string());

    let cmd_spec =
        config
            .commands
            .iter()
            .find(|c| c.cmd == alias)
            .ok_or_else(|| {
                anyhow!("Command alias '{alias}' not found in zoi.yaml")
            })?;

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

/// Groups tasks into execution stages where tasks in each stage can run in
/// parallel.
fn group_tasks_into_stages(
    resolved_tasks: &[String],
    config: &config::ProjectConfig
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
                    "Command spec for '{alias}' disappeared during task \
                     grouping"
                )
            })?;
        if let Some(deps) = &cmd_spec.depends_on {
            for dep in deps {
                if resolved_tasks.contains(dep) {
                    adj.entry(dep.clone())
                        .or_insert_with(Vec::new)
                        .push(alias.clone());
                    let degree = in_degree.get_mut(alias).ok_or_else(|| {
                        anyhow!("Task '{alias}' missing from in_degree map")
                    })?;
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
                    let degree =
                        in_degree.get_mut(neighbor).ok_or_else(|| {
                            anyhow!(
                                "Neighbor task '{neighbor}' missing from \
                                 in_degree map"
                            )
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

/// Executes a single task command, appending any extra arguments if it is the
/// target task.
fn run_single_command(
    command_to_run: &config::CommandSpec,
    args: &[String],
    platform: &str
) -> Result<()> {
    let run_cmd = match &command_to_run.run {
        config::PlatformOrString::String(s) => s.clone(),
        config::PlatformOrString::Platform(p) => p
            .get(platform)
            .or_else(|| p.get("default"))
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "No command found for platform '{platform}' and no \
                     default specified"
                )
            })?
    };

    let env_vars = match &command_to_run.env {
        config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
        config::PlatformOrEnvMap::Platform(p) => p
            .get(platform)
            .or_else(|| p.get("default"))
            .cloned()
            .unwrap_or_default()
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

/// Returns the path to the task cache file.
fn get_task_cache_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let cache_dir = current_dir.join(".zoi").join("cache");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join("tasks.json"))
}

/// Reads the task cache from disk.
fn read_task_cache() -> Result<HashMap<String, String>> {
    let path = get_task_cache_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

/// Checks if a task is already cached with the given hash.
fn is_task_cached(alias: &str, current_hash: &str) -> Result<bool> {
    let cache = read_task_cache()?;
    Ok(cache.get(alias).is_some_and(|h| h == current_hash))
}

/// Updates the task cache with a new hash for the specified task.
fn update_task_cache(alias: &str, hash: &str) -> Result<()> {
    let mut cache = read_task_cache()?;
    cache.insert(alias.to_string(), hash.to_string());
    let path = get_task_cache_path()?;
    let content = serde_json::to_string_pretty(&cache)?;
    fs::write(path, content)?;
    Ok(())
}

/// Calculates a SHA-256 hash of all files matching the provided glob patterns.
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
