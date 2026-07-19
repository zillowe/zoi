use anyhow::{Result, anyhow};
use colored::*;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use zoi_core::{sysroot, types, utils};

include!(concat!(env!("OUT_DIR"), "/generated_builtin_hooks.rs"));

/// Manages system-wide "Global Transaction Hooks".
///
/// Unlike package-specific hooks, global hooks are triggered based on the
/// file paths modified during a transaction. For example, if any package
/// touches a file in `/usr/share/fonts`, a global hook can automatically
/// run `fc-cache` exactly once at the end of the transaction.
///
/// Hooks are verified against a local trust database (`trusted_hashes.json`)
/// before execution to prevent unauthorized arbitrary command execution.

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalHook {
    pub name: String,
    pub description: String,
    pub platforms: Option<Vec<String>>,
    pub trigger: HookTrigger,
    pub action: HookAction,
    #[serde(skip)]
    pub is_builtin: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookTrigger {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub operation: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookAction {
    pub when: HookWhen,
    pub exec: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum HookWhen {
    #[serde(rename = "PreTransaction")]
    PreTransaction,
    #[serde(rename = "PostTransaction")]
    PostTransaction,
}

pub fn get_user_hooks_dir() -> Result<PathBuf> {
    let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let dir = home.join(".zoi").join("hooks");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

pub fn get_system_hooks_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        Ok(sysroot::apply_sysroot(PathBuf::from(
            "C:\\ProgramData\\zoi\\hooks",
        )))
    } else {
        Ok(sysroot::apply_sysroot(PathBuf::from("/etc/zoi/hooks")))
    }
}

pub fn load_all_hooks() -> Result<Vec<GlobalHook>> {
    let mut hook_map = HashMap::new();

    for (name, content) in BUILTIN_HOOKS {
        if let Ok(mut hook) = serde_yaml::from_str::<GlobalHook>(content) {
            hook.is_builtin = true;
            hook_map.insert(hook.name.clone(), hook);
        } else {
            eprintln!(
                "{}: Failed to parse builtin hook '{}'.",
                "Warning".yellow().bold(),
                name
            );
        }
    }

    let mut dirs = vec![get_system_hooks_dir()?, get_user_hooks_dir()?];

    // Scan the package store for bundled hooks
    for scope in [
        types::Scope::System,
        types::Scope::User,
        types::Scope::Project,
    ] {
        if let Ok(store_root) = utils::get_store_base_dir(scope) {
            if !store_root.exists() {
                continue;
            }
            // Each package has a directory: {hash}-{name}/{version}/hooks/
            if let Ok(pkg_dirs) = fs::read_dir(store_root) {
                for pkg_dir_entry in pkg_dirs.flatten() {
                    let pkg_dir = pkg_dir_entry.path();
                    if !pkg_dir.is_dir() {
                        continue;
                    }
                    // Iterate over version directories
                    if let Ok(version_dirs) = fs::read_dir(&pkg_dir) {
                        for version_dir_entry in version_dirs.flatten() {
                            let version_dir = version_dir_entry.path();
                            if !version_dir.is_dir()
                                || version_dir.file_name().and_then(|s| s.to_str())
                                    == Some("latest")
                                || version_dir.file_name().and_then(|s| s.to_str())
                                    == Some("dependents")
                            {
                                continue;
                            }
                            let hooks_dir = version_dir.join("hooks");
                            if hooks_dir.exists() && hooks_dir.is_dir() {
                                dirs.push(hooks_dir);
                            }
                        }
                    }
                }
            }
        }
    }

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        let mut hook_paths = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                hook_paths.push(entry.path());
            }
        }
        hook_paths.sort();
        for path in hook_paths {
            if path.is_file()
                && path.extension().and_then(|s| s.to_str()) == Some("yaml")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(hook) = serde_yaml::from_str::<GlobalHook>(&content)
            {
                hook_map.insert(hook.name.clone(), hook);
            }
        }
    }

    let mut hooks: Vec<GlobalHook> = hook_map.into_values().collect();
    hooks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(hooks)
}

fn normalized_relative_path(file: &str, sysroot: Option<&Path>) -> String {
    let file_path = Path::new(file);
    let relative_file = if let Some(root) = sysroot {
        file_path.strip_prefix(root).unwrap_or(file_path)
    } else if file_path.is_absolute() {
        let mut components = file_path.components();
        components.next();
        components.as_path()
    } else {
        file_path
    };

    relative_file
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn normalized_hook_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}

fn matches_trigger_dir(dir: &str, modified_file: &str) -> bool {
    let dir = normalized_hook_path(dir);
    !dir.is_empty()
        && (modified_file == dir
            || modified_file
                .strip_prefix(&dir)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

pub fn trigger_matches_modified_files(trigger: &HookTrigger, modified_files: &[String]) -> bool {
    let sysroot = sysroot::get_sysroot();

    for file in modified_files {
        let relative_file = normalized_relative_path(file, sysroot.as_deref());

        for dir in &trigger.dirs {
            if matches_trigger_dir(dir, &relative_file) {
                return true;
            }
        }

        for path_pattern in &trigger.paths {
            let pattern = match Pattern::new(path_pattern) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if pattern.matches_path(Path::new(&relative_file))
                || pattern.matches(&relative_file)
                || pattern.matches(file)
            {
                return true;
            }
        }
    }

    false
}

fn is_hook_trusted(hook: &GlobalHook) -> Result<bool> {
    let mut hasher = Sha256::new();
    hasher.update(hook.action.exec.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let trusted_path = get_user_hooks_dir()?.join("trusted_hashes.json");
    let mut trusted: HashMap<String, String> = if trusted_path.exists() {
        let content = fs::read_to_string(&trusted_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    if let Some(known_hash) = trusted.get(&hook.name)
        && known_hash == &hash
    {
        return Ok(true);
    }

    println!(
        "\n{}: Untrusted global hook detected: {}",
        "SECURITY WARNING".yellow().bold(),
        hook.name.cyan()
    );
    println!("Description: {}", hook.description);
    println!("Execution: {}", hook.action.exec.dimmed());
    println!("Hooks can execute arbitrary commands with your user's permissions.");

    if utils::ask_for_confirmation("Do you trust this hook and want to execute it?", false) {
        trusted.insert(hook.name.clone(), hash);
        let content = serde_json::to_string_pretty(&trusted)?;
        fs::write(trusted_path, content)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn run_global_hooks(when: HookWhen, modified_files: &[String], operation: &str) -> Result<()> {
    let all_hooks = load_all_hooks()?;
    let mut triggered_hooks = HashSet::new();
    let current_platform = utils::get_platform()?;

    for hook in all_hooks {
        if hook.action.when != when {
            continue;
        }

        if let Some(platforms) = &hook.platforms
            && !utils::is_platform_compatible(&current_platform, platforms)
        {
            continue;
        }

        if !hook.trigger.operation.is_empty()
            && !hook.trigger.operation.iter().any(|op| op == operation)
        {
            continue;
        }

        if trigger_matches_modified_files(&hook.trigger, modified_files)
            && triggered_hooks.insert(hook.name.clone())
        {
            if !hook.is_builtin && !is_hook_trusted(&hook)? {
                println!("Skipping untrusted hook: {}", hook.name);
                continue;
            }

            println!(
                "{} Running global hook: {} ({})",
                "::".blue().bold(),
                hook.name.cyan(),
                hook.description.dimmed()
            );

            let status = if cfg!(target_os = "windows") {
                Command::new("pwsh")
                    .arg("-Command")
                    .arg(&hook.action.exec)
                    .status()?
            } else {
                Command::new("bash")
                    .arg("-c")
                    .arg(&hook.action.exec)
                    .status()?
            };

            if !status.success() {
                eprintln!(
                    "{}: Global hook '{}' failed.",
                    "Warning".yellow().bold(),
                    hook.name
                );
            }
        }
    }

    Ok(())
}
