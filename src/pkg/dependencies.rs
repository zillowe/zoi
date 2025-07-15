use crate::utils;
use colored::*;
use std::error::Error;
use std::process::Command;

#[derive(Debug)]
struct Dependency<'a> {
    manager: &'a str,
    package: &'a str,
    version: Option<&'a str>,
}

fn parse_dependency_string(dep_str: &str) -> Result<Dependency, Box<dyn Error>> {
    let (manager, rest) = dep_str.split_once(':').unwrap_or(("zoi", dep_str));
    let (package, version) = rest
        .split_once(|c| c == '=' || c == '>' || c == '<')
        .map(|(p, v)| (p, Some(v)))
        .unwrap_or((rest, None));

    Ok(Dependency {
        manager,
        package,
        version,
    })
}

fn install_dependency(dep: &Dependency) -> Result<(), Box<dyn Error>> {
    let version_info = dep.version.unwrap_or("any");
    println!(
        "    -> Installing dependency: {} (version: {}) via {}",
        dep.package.cyan(),
        version_info.yellow(),
        dep.manager.yellow()
    );

    // NOTE: A real implementation would need to properly handle version constraints.
    // This is a simplified version that just installs the package by name.

    match dep.manager {
        "zoi" => {
            let status = Command::new(std::env::current_exe()?)
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err("Zoi dependency failed to install".into());
            }
        }
        "native" => {
            let pm_command =
                utils::get_native_package_manager().ok_or("Native package manager not found")?;
            println!("       (Using native manager: {})", pm_command);

            let install_args = match pm_command.as_str() {
                "apt" | "apt-get" => vec!["install", "-y"],
                "pacman" => vec!["-S", "--noconfirm"],
                "dnf" | "yum" => vec!["install", "-y"],
                "brew" => vec!["install"],
                "scoop" => vec!["install"],
                "choco" => vec!["install", "-y"],
                "apk" => vec!["add"],
                _ => {
                    return Err(
                        format!("Unsupported native package manager: {}", pm_command).into(),
                    )
                }
            };

            let mut command = Command::new("sudo");
            command.arg(&pm_command);
            command.args(install_args);
            command.arg(dep.package);

            let status = command.status()?;
            if !status.success() {
                return Err(format!("Failed to install native dependency: {}", dep.package).into());
            }
        }
        "cargo" => {
            let status = Command::new("cargo")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err("Cargo dependency failed".into());
            }
        }
        _ => return Err(format!("Unknown package manager in dependency: {}", dep.manager).into()),
    }
    Ok(())
}

pub fn resolve_and_install(deps: &[String]) -> Result<(), Box<dyn Error>> {
    if deps.is_empty() {
        return Ok(());
    }
    println!("{}", "  Resolving dependencies...".bold());
    for dep_str in deps {
        let dependency = parse_dependency_string(dep_str)?;
        install_dependency(&dependency)?;
    }
    println!("{}", "  All dependencies resolved.".green());
    Ok(())
}
