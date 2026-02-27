use crate::pkg::sysroot;
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

include!(concat!(env!("OUT_DIR"), "/generated_builtin_hooks.rs"));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalHook {
    pub name: String,
    pub description: String,
    pub platforms: Option<Vec<String>>,
    pub trigger: HookTrigger,
    pub action: HookAction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookTrigger {
    pub paths: Vec<String>,
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
    let mut hooks = Vec::new();

    for (name, content) in BUILTIN_HOOKS {
        if let Ok(hook) = serde_yaml::from_str::<GlobalHook>(content) {
            hooks.push(hook);
        } else {
            eprintln!(
                "{}: Failed to parse builtin hook '{}'.",
                "Warning".yellow().bold(),
                name
            );
        }
    }

    let dirs = vec![get_system_hooks_dir()?, get_user_hooks_dir()?];

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let content = fs::read_to_string(&path)?;
                if let Ok(hook) = serde_yaml::from_str::<GlobalHook>(&content)
                    && !hooks.iter().any(|h| h.name == hook.name)
                {
                    hooks.push(hook);
                }
            }
        }
    }
    Ok(hooks)
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

        let mut matched = false;
        let sysroot = sysroot::get_sysroot();

        for path_pattern in &hook.trigger.paths {
            let pattern = match Pattern::new(path_pattern) {
                Ok(p) => p,
                Err(_) => continue,
            };

            for file in modified_files {
                let file_path = Path::new(file);
                let relative_file = if let Some(root) = sysroot {
                    if let Ok(rel) = file_path.strip_prefix(root) {
                        rel
                    } else {
                        file_path
                    }
                } else if file_path.is_absolute() {
                    let mut components = file_path.components();
                    components.next();
                    components.as_path()
                } else {
                    file_path
                };

                if pattern.matches_path(relative_file) || pattern.matches(file) {
                    matched = true;
                    break;
                }
            }
            if matched {
                break;
            }
        }

        if matched {
            triggered_hooks.insert(hook.name.clone());
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
