use crate::{pkg::types, utils};
use colored::*;
use std::error::Error;
use std::io::{self, Write};
use std::process::Command;

pub fn run_uninstall_commands(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if pkg.package_type != types::PackageType::Script {
        return Err(format!("Package '{}' is not a script.", pkg.name).into());
    }

    let platform = utils::get_platform()?;
    if let Some(script_commands) = &pkg.script {
        if let Some(method) = script_commands
            .iter()
            .find(|m| utils::is_platform_compatible(&platform, &m.platforms))
        {
            if let Some(uninstall_commands) = &method.uninstall {
                println!("Running uninstall commands for '{}'...", pkg.name.cyan());
                for cmd_str in uninstall_commands {
                    println!("Executing: {}", cmd_str.cyan());
                    let output = if cfg!(target_os = "windows") {
                        Command::new("pwsh").arg("-Command").arg(cmd_str).output()?
                    } else {
                        Command::new("bash").arg("-c").arg(cmd_str).output()?
                    };

                    if !output.status.success() {
                        io::stdout().write_all(&output.stdout)?;
                        io::stderr().write_all(&output.stderr)?;
                        return Err(format!("Failed to execute command: '{}'", cmd_str).into());
                    }
                }
                println!("Script '{}' uninstalled successfully.", pkg.name.green());
            }
            Ok(())
        } else {
            Err(format!(
                "No compatible script method found for platform '{}'.",
                platform
            )
            .into())
        }
    } else {
        Err(format!(
            "Package '{}' is a script but has no commands defined.",
            pkg.name
        )
        .into())
    }
}
