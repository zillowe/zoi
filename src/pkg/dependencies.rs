use crate::pkg::local;
use crate::utils;
use colored::*;
use regex::Regex;
use semver::{Version, VersionReq};
use std::error::Error;
use std::fs;
use std::process::Command;

#[derive(Debug)]
struct Dependency<'a> {
    manager: &'a str,
    package: &'a str,
    req: Option<VersionReq>,
}

fn parse_dependency_string(dep_str: &str) -> Result<Dependency, Box<dyn Error>> {
    let (manager, rest) = dep_str.split_once(':').unwrap_or(("zoi", dep_str));
    let (package, req_str) = if let Some(idx) = rest.find(['=', '>', '<', '~', '^']) {
        rest.split_at(idx)
    } else {
        (rest, "*")
    };
    let req = if req_str == "*" {
        None
    } else {
        Some(VersionReq::parse(req_str)?)
    };
    Ok(Dependency {
        manager,
        package,
        req,
    })
}

fn get_native_command_version(command_name: &str) -> Result<Option<Version>, Box<dyn Error>> {
    if !utils::command_exists(command_name) {
        return Ok(None);
    }

    let version_flags = ["--version", "version", "-v", "-V"];
    let mut output = String::new();

    for flag in &version_flags {
        let result = Command::new(command_name).arg(flag).output();
        if let Ok(res) = result {
            if res.status.success() {
                output = String::from_utf8_lossy(&res.stdout).to_string();
                if output.is_empty() {
                    output = String::from_utf8_lossy(&res.stderr).to_string();
                }
                if !output.is_empty() {
                    break;
                }
            }
        }
    }

    if output.is_empty() {
        return Ok(None);
    }

    let re = Regex::new(r"(\d+\.\d+\.\d+)")?;
    if let Some(caps) = re.captures(&output) {
        if let Some(matched) = caps.get(1) {
            if let Ok(version) = Version::parse(matched.as_str()) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

fn record_dependent(dependency_name: &str, dependent_pkg_name: &str) -> Result<(), Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let dependents_dir = home_dir
        .join(".zoi/pkgs/store")
        .join(dependency_name)
        .join("dependents");

    fs::create_dir_all(&dependents_dir)?;

    fs::File::create(dependents_dir.join(dependent_pkg_name))?;

    Ok(())
}

fn install_dependency(dep: &Dependency, parent_pkg_name: &str) -> Result<(), Box<dyn Error>> {
    let version_info = dep
        .req
        .as_ref()
        .map_or("any".to_string(), |r| r.to_string());
    println!(
        "-> Checking dependency: {} (version: {}) via {}",
        dep.package.cyan(),
        version_info.yellow(),
        dep.manager.yellow()
    );

    match dep.manager {
        "zoi" => {
            let zoi_dep_name = dep.package;
            record_dependent(zoi_dep_name, parent_pkg_name)?;
            if let Some(manifest) = local::is_package_installed(zoi_dep_name)? {
                let installed_version = Version::parse(&manifest.version)?;
                if let Some(req) = &dep.req {
                    if req.matches(&installed_version) {
                        println!(
                            "Already installed (version {installed_version} satisfies {req}). Skipping."
                        );
                        return Ok(());
                    } else {
                        return Err(format!(
                            "Version conflict for '{}': need {}, but {} is installed.",
                            dep.package, req, installed_version
                        )
                        .into());
                    }
                } else {
                    println!("Already installed (version {installed_version}). Skipping.");
                    return Ok(());
                }
            }

            println!("Not installed. Proceeding with installation...");
            install_zoi_dependency(dep.package)?;
        }
        "native" => {
            if let Some(installed_version) = get_native_command_version(dep.package)? {
                if let Some(req) = &dep.req {
                    if req.matches(&installed_version) {
                        println!(
                            "Already installed (version {installed_version} satisfies {req}). Skipping."
                        );
                        return Ok(());
                    } else {
                        println!(
                            "{} Installed version {} does not satisfy requirement {}.",
                            "Warning:".yellow(),
                            installed_version,
                            req
                        );
                    }
                } else {
                    println!("Already installed (version {installed_version}). Skipping.");
                    return Ok(());
                }
            }

            if dep.req.is_some() && get_native_command_version(dep.package)?.is_none() {
                println!(
                    "{} Could not determine installed version. Proceeding with installation via system package manager.",
                    "Warning:".yellow()
                );
            }

            let pm =
                utils::get_native_package_manager().ok_or("Native package manager not found")?;
            println!("       (Using native manager: {pm})");
            let args = match pm.as_str() {
                "apt" | "apt-get" => vec!["install", "-y"],
                "pacman" => vec!["-S", "--noconfirm"],
                "dnf" | "yum" => vec!["install", "-y"],
                "brew" => vec!["install"],
                "scoop" => vec!["install"],
                "choco" => vec!["install", "-y"],
                "apk" => vec!["add"],
                _ => return Err(format!("Unsupported native package manager: {pm}").into()),
            };

            let package_to_install = dep.package;

            let mut command = Command::new("sudo");
            command.arg(&pm);
            command.args(args);
            command.arg(package_to_install);

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

pub fn resolve_and_install(deps: &[String], parent_pkg_name: &str) -> Result<(), Box<dyn Error>> {
    if deps.is_empty() {
        return Ok(());
    }

    println!("{}", "Resolving dependencies...".bold());
    for dep_str in deps {
        let dependency = parse_dependency_string(dep_str)?;
        install_dependency(&dependency, parent_pkg_name)?;
    }
    println!("{}", "All dependencies resolved.".green());
    Ok(())
}

fn install_zoi_dependency(package_name: &str) -> Result<(), Box<dyn Error>> {
    use crate::pkg::{install, resolve};
    let resolved_source = resolve::resolve_source(package_name)?;

    install::run_installation(
        &resolved_source.path,
        install::InstallMode::PreferBinary,
        false,
        crate::pkg::types::InstallReason::Dependency,
    )
}
