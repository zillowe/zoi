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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    PostRemove
}

/// Executes a list of shell commands within a specific scope.
///
/// While installing into a sysroot (`zoi system distro build`, `--root`), the
/// commands run inside that root, as global hooks already do. Run on the host
/// they would act on the host instead: bash's hook appended to the host's
/// `/etc/shells` and systemd's ran the host's `systemd-sysusers`.
fn execute_commands(commands: &[String], scope: types::Scope) -> Result<()> {
    let scope_str = format!("{scope:?}").to_lowercase();
    for cmd_str in commands {
        println!("> {}", cmd_str.cyan());
        let Some(mut command) = hook_command(cmd_str, &scope_str)? else {
            continue;
        };

        let status = command.status()?;

        if !status.success() {
            return Err(anyhow!("Hook command failed: {cmd_str}"));
        }
    }
    Ok(())
}

/// Builds the command that runs one hook, or `None` if it cannot run (a
/// sysroot without a shell yet).
fn hook_command(cmd_str: &str, scope_str: &str) -> Result<Option<Command>> {
    #[cfg(target_os = "linux")]
    if let Some(root) = zoi_core::sysroot::get_sysroot() {
        let has_bash = root.join("bin/bash").exists()
            || root.join("usr/bin/bash").exists();
        if !has_bash {
            eprintln!(
                "{}: skipping hook in {}: the target has no /bin/bash yet.",
                "Warning".yellow().bold(),
                root.display()
            );
            return Ok(None);
        }
        let mut envs = std::collections::HashMap::new();
        envs.insert("ZOI_SCOPE".to_string(), scope_str.to_string());
        envs.insert(
            "PATH".to_string(),
            "/usr/bin:/bin:/usr/sbin:/sbin".to_string()
        );
        return zoi_sandbox::wrap_command_in_root(
            &root,
            std::path::Path::new("/bin/bash"),
            &["-c".to_string(), cmd_str.to_string()],
            &envs,
            &[],
            false
        )
        .map(Some);
    }

    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("pwsh");
        c.arg("-Command").arg(cmd_str);
        c
    } else {
        let mut c = Command::new("bash");
        c.arg("-c").arg(cmd_str);
        c
    };
    command.env("ZOI_SCOPE", scope_str);
    Ok(Some(command))
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
    scope: types::Scope
) -> Result<()> {
    let platform = utils::get_platform()?;

    let commands_to_run = match hook_type {
        HookType::PreInstall => &hooks.pre_install,
        HookType::PostInstall => &hooks.post_install,
        HookType::PreUpgrade => &hooks.pre_upgrade,
        HookType::PostUpgrade => &hooks.post_upgrade,
        HookType::PreRemove => &hooks.pre_remove,
        HookType::PostRemove => &hooks.post_remove
    };

    if let Some(platform_or_string_vec) = commands_to_run {
        match platform_or_string_vec {
            PlatformOrStringVec::StringVec(cmds) => {
                execute_commands(cmds, scope)?;
            }
            PlatformOrStringVec::Platform(platform_map) => {
                if let Some(cmds) = platform_map.get(&platform) {
                    execute_commands(cmds, scope)?;
                } else if let Some(cmds) = platform_map.get("default") {
                    execute_commands(cmds, scope)?;
                }
            }
        }
    }

    Ok(())
}
