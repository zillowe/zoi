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

/// How a hook run is reported.
#[derive(Clone, Copy)]
struct Reporter<'a> {
    /// The package the hooks belong to, e.g. `bash` or `systemd:libs`.
    label: Option<&'a str>,
    /// The progress bar to print above, if one is active.
    pb: Option<&'a indicatif::ProgressBar>
}

impl Reporter<'_> {
    /// Prints a line above the progress bars, or plainly without them.
    fn line(self, msg: &str) {
        match self.pb {
            Some(pb) => pb.println(msg),
            None => println!("{msg}")
        }
    }
}

/// The longest tail of a failed hook's output that is shown.
const FAILED_OUTPUT_LINES: usize = 20;

/// Executes a list of shell commands within a specific scope.
///
/// While installing into a sysroot (`zoi system distro build`, `--root`), the
/// commands run inside that root, as global hooks already do. Run on the host
/// they would act on the host instead: bash's hook appended to the host's
/// `/etc/shells` and systemd's ran the host's `systemd-sysusers`.
///
/// With a package label (installs), one short line is printed per command and
/// the hook's own output is captured, then shown only if it fails, so it
/// cannot tangle with the progress bars. Without one, the old behaviour is
/// kept: the command is echoed and its output streams through.
fn execute_commands(
    commands: &[String],
    scope: types::Scope,
    hook_name: &str,
    reporter: Reporter<'_>
) -> Result<()> {
    let scope_str = format!("{scope:?}").to_lowercase();
    for cmd_str in commands {
        let Some(label) = reporter.label else {
            println!("> {}", cmd_str.cyan());
            let Some(mut command) = hook_command(cmd_str, &scope_str)? else {
                continue;
            };
            if !command.status()?.success() {
                return Err(anyhow!("Hook command failed: {cmd_str}"));
            }
            continue;
        };

        reporter.line(&format!(
            "  {} running {hook_name} hook for {}",
            "->".dimmed(),
            label.cyan()
        ));
        let Some(mut command) = hook_command(cmd_str, &scope_str)? else {
            continue;
        };
        let output = command.output()?;
        if output.status.success() {
            continue;
        }

        let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        let lines: Vec<&str> = combined.lines().collect();
        let tail = lines
            .get(lines.len().saturating_sub(FAILED_OUTPUT_LINES)..)
            .unwrap_or_default();
        let mut report = format!(
            "{} {hook_name} hook for {} failed ({}):",
            "Error:".red().bold(),
            label.cyan(),
            output.status
        );
        for line in tail {
            report.push_str("\n    ");
            report.push_str(line);
        }
        if tail.is_empty() {
            report.push_str(" no output");
        }
        reporter.line(&report);
        return Err(anyhow!("{hook_name} hook for '{label}' failed"));
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
    run_hooks_inner(
        hooks,
        hook_type,
        scope,
        Reporter {
            label: None,
            pb: None
        }
    )
}

/// Runs the specified type of hooks for `label` (e.g. `systemd:libs`),
/// printing one short line per hook above `pb` and showing a hook's output
/// only when it fails.
///
/// # Errors
///
/// Returns an error if getting the current platform or executing a hook
/// command fails.
pub fn run_hooks_for(
    hooks: &Hooks,
    hook_type: HookType,
    scope: types::Scope,
    label: &str,
    pb: Option<&indicatif::ProgressBar>
) -> Result<()> {
    run_hooks_inner(
        hooks,
        hook_type,
        scope,
        Reporter {
            label: Some(label),
            pb
        }
    )
}

/// Selects the commands for `hook_type` and runs them.
fn run_hooks_inner(
    hooks: &Hooks,
    hook_type: HookType,
    scope: types::Scope,
    reporter: Reporter<'_>
) -> Result<()> {
    let platform = utils::get_platform()?;

    let (hook_name, commands_to_run) = match hook_type {
        HookType::PreInstall => ("pre-install", &hooks.pre_install),
        HookType::PostInstall => ("post-install", &hooks.post_install),
        HookType::PreUpgrade => ("pre-upgrade", &hooks.pre_upgrade),
        HookType::PostUpgrade => ("post-upgrade", &hooks.post_upgrade),
        HookType::PreRemove => ("pre-remove", &hooks.pre_remove),
        HookType::PostRemove => ("post-remove", &hooks.post_remove)
    };

    if let Some(platform_or_string_vec) = commands_to_run {
        let cmds = match platform_or_string_vec {
            PlatformOrStringVec::StringVec(cmds) => Some(cmds),
            PlatformOrStringVec::Platform(platform_map) => platform_map
                .get(&platform)
                .or_else(|| platform_map.get("default"))
        };
        if let Some(cmds) = cmds {
            execute_commands(cmds, scope, hook_name, reporter)?;
        }
    }

    Ok(())
}
