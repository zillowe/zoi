use anyhow::{Result, anyhow};
use colored::*;
use std::process::Command;
use zoi_core::types::{self, Hooks, PlatformOrStringVec};
use zoi_core::utils;

pub mod global;

pub enum HookType {
    PreInstall,
    PostInstall,
    PreUpgrade,
    PostUpgrade,
    PreRemove,
    PostRemove,
}

fn execute_commands(commands: &[String], scope: types::Scope) -> Result<()> {
    let scope_str = format!("{:?}", scope).to_lowercase();
    for cmd_str in commands {
        println!("> {}", cmd_str.cyan());
        let mut command = if cfg!(target_os = "windows") {
            let mut c = Command::new("pwsh");
            c.arg("-Command").arg(cmd_str);
            c
        } else {
            let mut c = Command::new("bash");
            c.arg("-c").arg(cmd_str);
            c
        };

        command.env("ZOI_SCOPE", &scope_str);

        let status = command.status()?;

        if !status.success() {
            return Err(anyhow!("Hook command failed: {}", cmd_str));
        }
    }
    Ok(())
}

pub fn run_hooks(hooks: &Hooks, hook_type: HookType, scope: types::Scope) -> Result<()> {
    let platform = utils::get_platform()?;

    let commands_to_run = match hook_type {
        HookType::PreInstall => &hooks.pre_install,
        HookType::PostInstall => &hooks.post_install,
        HookType::PreUpgrade => &hooks.pre_upgrade,
        HookType::PostUpgrade => &hooks.post_upgrade,
        HookType::PreRemove => &hooks.pre_remove,
        HookType::PostRemove => &hooks.post_remove,
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
