use crate::pkg::types::{Hooks, PlatformOrStringVec};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::process::Command;

pub enum HookType {
    PreInstall,
    PostInstall,
    PreUpgrade,
    PostUpgrade,
    PreRemove,
    PostRemove,
}

fn execute_commands(commands: &[String]) -> Result<()> {
    for cmd_str in commands {
        println!("> {}", cmd_str.cyan());
        let status = if cfg!(target_os = "windows") {
            Command::new("pwsh").arg("-Command").arg(cmd_str).status()?
        } else {
            Command::new("bash").arg("-c").arg(cmd_str).status()?
        };

        if !status.success() {
            return Err(anyhow!("Hook command failed: {}", cmd_str));
        }
    }
    Ok(())
}

pub fn run_hooks(hooks: &Hooks, hook_type: HookType) -> Result<()> {
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
                execute_commands(cmds)?;
            }
            PlatformOrStringVec::Platform(platform_map) => {
                if let Some(cmds) = platform_map.get(&platform) {
                    execute_commands(cmds)?;
                } else if let Some(cmds) = platform_map.get("default") {
                    execute_commands(cmds)?;
                }
            }
        }
    }

    Ok(())
}
