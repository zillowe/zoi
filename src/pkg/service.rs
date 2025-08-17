use crate::{pkg::types, utils};
use colored::*;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

fn get_service_dir(pkg_name: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let path = home::home_dir()
        .ok_or("Could not find home directory.")?
        .join(".zoi")
        .join("pkgs")
        .join("store")
        .join(pkg_name)
        .join("service");
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn start_service(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", pkg.name).into());
    }

    let platform = utils::get_platform()?;
    if let Some(service_methods) = &pkg.service {
        let method = service_methods.iter().find(|m| {
            let platforms = match m {
                types::ServiceMethod::Command { platforms, .. } => platforms,
                types::ServiceMethod::Docker { platforms, .. } => platforms,
            };
            utils::is_platform_compatible(&platform, platforms)
        });

        if let Some(method) = method {
            println!("Starting service '{}'...", pkg.name.cyan());
            match method {
                types::ServiceMethod::Command { start, .. } => {
                    for cmd_str in start {
                        println!("Executing: {}", cmd_str.cyan());
                        let output = if cfg!(target_os = "windows") {
                            Command::new("pwsh").arg("-Command").arg(cmd_str).output()?
                        } else {
                            Command::new("bash").arg("-c").arg(cmd_str).output()?
                        };

                        if !output.status.success() {
                            io::stdout().write_all(&output.stdout)?;
                            io::stderr().write_all(&output.stderr)?;
                            return Err(
                                format!("Failed to execute start command: '{}'", cmd_str).into()
                            );
                        }
                    }
                }
                types::ServiceMethod::Docker { docker, .. } => {
                    if !utils::command_exists("docker") {
                        return Err("Docker is not installed or not in PATH.".into());
                    }
                    for docker_method in docker {
                        match docker_method {
                            types::DockerType::Compose { file } => {
                                if !utils::command_exists("docker-compose") {
                                    return Err(
                                        "docker-compose is not installed or not in PATH.".into()
                                    );
                                }
                                let service_dir = get_service_dir(&pkg.name)?;
                                let compose_path = service_dir.join("docker-compose.yml");

                                let compose_content = if file.starts_with("http") {
                                    println!(
                                        "Downloading docker-compose file from {}",
                                        file.cyan()
                                    );
                                    let client = crate::utils::build_blocking_http_client(60)?;
                                    client.get(file).send()?.text()?
                                } else {
                                    return Err(format!(
                                        "File path for docker-compose is not supported yet: {}",
                                        file
                                    )
                                    .into());
                                };
                                fs::write(&compose_path, compose_content)?;

                                let cmd_str = format!(
                                    "docker-compose -f {} up -d",
                                    compose_path.to_string_lossy()
                                );
                                println!("Executing: {}", cmd_str.cyan());
                                let output =
                                    Command::new("bash").arg("-c").arg(&cmd_str).output()?;

                                if !output.status.success() {
                                    io::stdout().write_all(&output.stdout)?;
                                    io::stderr().write_all(&output.stderr)?;
                                    return Err(format!(
                                        "Failed to start docker-compose service: {}",
                                        String::from_utf8_lossy(&output.stderr)
                                    )
                                    .into());
                                }
                            }
                        }
                    }
                }
            }
            println!("Service '{}' started successfully.", pkg.name.green());
            Ok(())
        } else {
            Err(format!(
                "No compatible service method found for platform '{}'.",
                platform
            )
            .into())
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
        let method = service_methods.iter().find(|m| {
            let platforms = match m {
                types::ServiceMethod::Command { platforms, .. } => platforms,
                types::ServiceMethod::Docker { platforms, .. } => platforms,
            };
            utils::is_platform_compatible(&platform, platforms)
        });

        if let Some(method) = method {
            println!("Stopping service '{}'...", pkg.name.cyan());
            match method {
                types::ServiceMethod::Command { stop, .. } => {
                    for cmd_str in stop {
                        println!("Executing: {}", cmd_str.cyan());
                        let output = if cfg!(target_os = "windows") {
                            Command::new("pwsh").arg("-Command").arg(cmd_str).output()?
                        } else {
                            Command::new("bash").arg("-c").arg(cmd_str).output()?
                        };

                        if !output.status.success() {
                            io::stdout().write_all(&output.stdout)?;
                            io::stderr().write_all(&output.stderr)?;
                            return Err(
                                format!("Failed to execute stop command: '{}'", cmd_str).into()
                            );
                        }
                    }
                }
                types::ServiceMethod::Docker { docker, .. } => {
                    if !utils::command_exists("docker") {
                        return Err("Docker is not installed or not in PATH.".into());
                    }
                    for docker_method in docker {
                        match docker_method {
                            types::DockerType::Compose { .. } => {
                                if !utils::command_exists("docker-compose") {
                                    return Err(
                                        "docker-compose is not installed or not in PATH.".into()
                                    );
                                }
                                let service_dir = get_service_dir(&pkg.name)?;
                                let compose_path = service_dir.join("docker-compose.yml");

                                if !compose_path.exists() {
                                    return Err(format!(
                                        "docker-compose file not found for service '{}'. Was it started?",
                                        pkg.name
                                    )
                                    .into());
                                }

                                let cmd_str = format!(
                                    "docker-compose -f {} down",
                                    compose_path.to_string_lossy()
                                );
                                println!("Executing: {}", cmd_str.cyan());
                                let output =
                                    Command::new("bash").arg("-c").arg(&cmd_str).output()?;

                                if !output.status.success() {
                                    io::stdout().write_all(&output.stdout)?;
                                    io::stderr().write_all(&output.stderr)?;
                                    return Err(format!(
                                        "Failed to stop docker-compose service: {}",
                                        String::from_utf8_lossy(&output.stderr)
                                    )
                                    .into());
                                }
                            }
                        }
                    }
                }
            }
            println!("Service '{}' stopped successfully.", pkg.name.green());
            Ok(())
        } else {
            Err(format!(
                "No compatible service method found for platform '{}'.",
                platform
            )
            .into())
        }
    } else {
        Err(format!(
            "Package '{}' is a service but has no service methods defined.",
            pkg.name
        )
        .into())
    }
}
