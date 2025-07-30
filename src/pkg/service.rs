use crate::{pkg::types, utils};
use colored::*;
use std::error::Error;
use std::io::{self, Write};
use std::process::Command;

pub fn start_service(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", pkg.name).into());
    }

    let platform = utils::get_platform()?;
    if let Some(service_methods) = &pkg.service {
        if let Some(method) = service_methods
            .iter()
            .find(|m| utils::is_platform_compatible(&platform, &m.platforms))
        {
            println!("Starting service '{}'...", pkg.name.cyan());
            for cmd_str in &method.start {
                println!("Executing: {}", cmd_str.cyan());
                let output = if cfg!(target_os = "windows") {
                    Command::new("cmd").arg("/C").arg(cmd_str).output()?
                } else {
                    Command::new("sh").arg("-c").arg(cmd_str).output()?
                };

                if !output.status.success() {
                    io::stdout().write_all(&output.stdout)?;
                    io::stderr().write_all(&output.stderr)?;
                    return Err(format!("Failed to execute start command: '{}'", cmd_str).into());
                }
            }
            println!("Service '{}' started successfully.", pkg.name.green());
            Ok(())
        } else {
            Err(
                format!(
                    "No compatible service method found for platform '{}'.",
                    platform
                )
                .into(),
            )
        }
    } else {
        Err(format!(
            "Package '{}' is a service but has no service methods defined.",
            pkg.name
        )
        .into())
    }
}

pub fn stop_service(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", pkg.name).into());
    }

    let platform = utils::get_platform()?;
    if let Some(service_methods) = &pkg.service {
        if let Some(method) = service_methods
            .iter()
            .find(|m| utils::is_platform_compatible(&platform, &m.platforms))
        {
            println!("Stopping service '{}'...", pkg.name.cyan());
            for cmd_str in &method.stop {
                println!("Executing: {}", cmd_str.cyan());
                let output = if cfg!(target_os = "windows") {
                    Command::new("cmd").arg("/C").arg(cmd_str).output()?
                } else {
                    Command::new("sh").arg("-c").arg(cmd_str).output()?
                };

                if !output.status.success() {
                    io::stdout().write_all(&output.stdout)?;
                    io::stderr().write_all(&output.stderr)?;
                    return Err(format!("Failed to execute stop command: '{}'", cmd_str).into());
                }
            }
            println!("Service '{}' stopped successfully.", pkg.name.green());
            Ok(())
        } else {
            Err(
                format!(
                    "No compatible service method found for platform '{}'.",
                    platform
                )
                .into(),
            )
        }
    } else {
        Err(format!(
            "Package '{}' is a service but has no service methods defined.",
            pkg.name
        )
        .into())
    }
}
