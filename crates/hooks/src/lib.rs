//! Hooks functionality for Zoi.
//!
//! This crate provides the logic for executing package-specific and global
//! hooks.

use std::process::Command;

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::types::{self, Hooks, PlatformOrStringVec};
use zoi_core::utils;

/// Manages system-wide "Global Transaction Hooks".
pub mod global;

/// The type of hook being executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq,)]
pub enum HookType {
    /// Runs before a package is installed.
    PreInstall,
    /// Runs after a package is installed.
    PostInstall,
    /// Runs before a package is upgraded.
    PreUpgrade,
    /// Runs after a package is upgraded.
    PostUpgrade,
    /// Runs before a package is removed.
    PreRemove,
    /// Runs after a package is removed.
    PostRemove,
}

/// Executes a list of shell commands within a specific scope.
fn execute_commands(commands: &[String], scope: types::Scope,) -> Result<(),> {
    let scope_str = format!("{scope:?}").to_lowercase();
    for cmd_str in commands {
        println!("> {}", cmd_str.cyan());
        let mut command = if cfg!(target_os = "windows") {
            let mut c = Command::new("pwsh",);
            c.arg("-Command",).arg(cmd_str,);
            c
        } else {
            let mut c = Command::new("bash",);
            c.arg("-c",).arg(cmd_str,);
            c
        };

        command.env("ZOI_SCOPE", &scope_str,);

        let status = command.status()?;

        if !status.success() {
            return Err(anyhow!("Hook command failed: {cmd_str}"),);
        }
    }
    Ok((),)
}

/// Runs the specified type of hooks for a package.
///
/// # Arguments
///
/// * `hooks` - The hooks configuration from the package.
/// * `hook_type` - The type of hook to run.
/// * `scope` - The installation scope (System, User, or Project).
///
/// # Errors
///
/// Returns an error if getting the current platform or executing a hook command
/// fails.
pub fn run_hooks(
    hooks: &Hooks,
    hook_type: HookType,
    scope: types::Scope,
) -> Result<(),> {
    let platform = utils::get_platform()?;

    let commands_to_run = match hook_type {
        HookType::PreInstall => &hooks.pre_install,
        HookType::PostInstall => &hooks.post_install,
        HookType::PreUpgrade => &hooks.pre_upgrade,
        HookType::PostUpgrade => &hooks.post_upgrade,
        HookType::PreRemove => &hooks.pre_remove,
        HookType::PostRemove => &hooks.post_remove,
    };

    if let Some(platform_or_string_vec,) = commands_to_run {
        match platform_or_string_vec {
            PlatformOrStringVec::StringVec(cmds,) => {
                execute_commands(cmds, scope,)?;
            }
            PlatformOrStringVec::Platform(platform_map,) => {
                if let Some(cmds,) = platform_map.get(&platform,) {
                    execute_commands(cmds, scope,)?;
                } else if let Some(cmds,) = platform_map.get("default",) {
                    execute_commands(cmds, scope,)?;
                }
            }
        }
    }

    Ok((),)
}
