use crate::pkg::{install, local, resolve, types};
use crate::utils;
use colored::*;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use semver::{Version, VersionReq};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use tempfile::Builder;

#[derive(Debug)]
struct Dependency<'a> {
    manager: &'a str,
    package: &'a str,
    req: Option<VersionReq>,
    version_str: Option<String>,
    description: Option<&'a str>,
}

fn get_dependency_graph_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let path = home_dir.join(".zoi").join("pkgs").join("dependencies.json");
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(path)
}

fn read_dependency_graph() -> Result<HashMap<String, Vec<String>>, Box<dyn Error>> {
    let path = get_dependency_graph_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }
    let graph: HashMap<String, Vec<String>> = serde_json::from_str(&content)?;
    Ok(graph)
}

fn write_dependency_graph(graph: &HashMap<String, Vec<String>>) -> Result<(), Box<dyn Error>> {
    let path = get_dependency_graph_path()?;
    let content = serde_json::to_string_pretty(graph)?;
    fs::write(path, content)?;
    Ok(())
}

fn add_dependency_link(dependent: &str, dependency: &str) -> Result<(), Box<dyn Error>> {
    let mut graph = read_dependency_graph()?;
    let dependents = graph.entry(dependency.to_string()).or_default();
    if !dependents.contains(&dependent.to_string()) {
        dependents.push(dependent.to_string());
    }
    write_dependency_graph(&graph)
}

pub fn remove_dependency_link(dependent: &str, dependency: &str) -> Result<(), Box<dyn Error>> {
    let mut graph = read_dependency_graph()?;
    if let Some(dependents) = graph.get_mut(dependency) {
        dependents.retain(|d| d != dependent);
    }
    graph.retain(|_, dependents| !dependents.is_empty());
    write_dependency_graph(&graph)
}

pub fn get_dependents(dependency: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let graph = read_dependency_graph()?;
    Ok(graph.get(dependency).cloned().unwrap_or_default())
}

fn parse_dependency_string(dep_str: &str) -> Result<Dependency<'_>, Box<dyn Error>> {
    let (manager, rest) = dep_str.split_once(':').unwrap_or(("zoi", dep_str));

    let (package_and_version, description) = if manager != "go" {
        if let Some((main, desc)) = rest.rsplit_once(':') {
            if main.is_empty() || desc.contains(['=', '>', '<', '~', '^', '@']) {
                (rest, None)
            } else {
                (main, Some(desc))
            }
        } else {
            (rest, None)
        }
    } else {
        (rest, None)
    };

    let (package, version_str) = if let Some(at_pos) = package_and_version.rfind('@') {
        if at_pos > 0 {
            let (p, v) = package_and_version.split_at(at_pos);
            (p, Some(v[1..].to_string()))
        } else {
            (package_and_version, None)
        }
    } else if let Some(idx) = package_and_version.find(['=', '>', '<', '~', '^']) {
        let (p, v) = package_and_version.split_at(idx);
        (p, Some(v.to_string()))
    } else {
        (package_and_version, None)
    };

    let req = if let Some(v_str) = &version_str {
        let req_parse_str = if v_str.chars().next().unwrap().is_ascii_digit() {
            format!("={}", v_str)
        } else {
            v_str.to_string()
        };
        Some(VersionReq::parse(&req_parse_str)?)
    } else {
        None
    };

    Ok(Dependency {
        manager,
        package,
        req,
        version_str,
        description,
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
        if let Ok(res) = result
            && res.status.success()
        {
            output = String::from_utf8_lossy(&res.stdout).to_string();
            if output.is_empty() {
                output = String::from_utf8_lossy(&res.stderr).to_string();
            }
            if !output.is_empty() {
                break;
            }
        }
    }

    if output.is_empty() {
        return Ok(None);
    }

    let re = Regex::new(r"(\d+\.\d+\.\d+)")?;
    if let Some(caps) = re.captures(&output)
        && let Some(matched) = caps.get(1)
        && let Ok(version) = Version::parse(matched.as_str())
    {
        return Ok(Some(version));
    }

    Ok(None)
}

fn install_dependency(
    dep: &Dependency,
    parent_pkg_name: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let dep_id = format!("{}:{}", dep.manager, dep.package);
    if !processed_deps.insert(dep_id.clone()) {
        return Ok(());
    }

    let version_info = dep
        .version_str
        .as_ref()
        .map_or("any".to_string(), |r| r.to_string());
    println!(
        "-> Checking dependency: {} (version: {}) via {}",
        dep.package.cyan(),
        version_info.yellow(),
        dep.manager.yellow()
    );

    let os = std::env::consts::OS;
    let manager = dep.manager;

    let is_compatible = match manager {
        "brew" | "macports" | "brew-cask" | "mas" => os == "macos",
        "scoop" | "choco" | "winget" => os == "windows",
        "apt" | "apt-get" => utils::get_linux_distro_family().as_deref() == Some("debian"),
        "pacman" | "yay" | "paru" | "pikaur" | "trizen" | "aur" => {
            utils::get_linux_distro_family().as_deref() == Some("arch")
        }
        "dnf" | "yum" => utils::get_linux_distro_family().as_deref() == Some("fedora"),
        "zypper" => utils::get_linux_distro_family().as_deref() == Some("suse"),
        "apk" => utils::get_linux_distro_family().as_deref() == Some("alpine"),
        "portage" => utils::get_linux_distro_family().as_deref() == Some("gentoo"),
        "xbps" | "xbps-install" => utils::get_linux_distribution().as_deref() == Some("void"),
        "eopkg" => utils::get_linux_distribution().as_deref() == Some("solus"),
        "guix" => utils::get_linux_distribution().as_deref() == Some("guix"),
        "snap" | "flatpak" => os == "linux",
        "pkg" => os == "freebsd",
        "pkg_add" => os == "openbsd",
        "zoi" | "cargo" | "native" | "go" | "npm" | "deno" | "jsr" | "bun" | "pip" | "pipx"
        | "cargo-binstall" | "gem" | "yarn" | "pnpm" | "composer" | "dotnet" | "nix" | "conda"
        | "script" | "volta" => true,
        _ => false,
    };

    if !is_compatible {
        println!(
            "Skipping dependency '{}' for manager '{}' on an incompatible OS.",
            dep.package, manager
        );
        return Ok(());
    }

    add_dependency_link(parent_pkg_name, &dep_id)?;
    installed_deps.push(dep_id.clone());

    match manager {
        "zoi" => {
            let zoi_dep_name = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            if let Some(manifest) = local::is_package_installed(&dep.package.to_lowercase(), scope)?
            {
                match Version::parse(&manifest.version) {
                    Ok(installed_version) => {
                        if let Some(req) = &dep.req {
                            if req.matches(&installed_version) {
                                println!(
                                    "Already installed (version {} satisfies {}). Skipping.",
                                    installed_version, req
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
                            println!(
                                "Already installed (version {}). Skipping.",
                                installed_version
                            );
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        println!(
                            "{} Could not parse version ('{}') for installed package '{}': {}.",
                            "Warning:".yellow(),
                            manifest.version,
                            dep.package,
                            e
                        );
                        if dep.req.is_some() {
                            println!("Proceeding with installation due to version check failure.");
                        } else {
                            println!("Assuming package is installed and skipping.");
                            return Ok(());
                        }
                    }
                }
            }

            println!("Not installed. Proceeding with installation...");

            install_zoi_dependency(&zoi_dep_name, yes, all_optional, processed_deps)?;
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
            println!("(Using native manager: {pm})");
            let args = match pm.as_str() {
                "apt" | "apt-get" => vec!["install", "-y"],
                "pacman" => vec!["-S", "--needed", "--noconfirm"],
                "dnf" | "yum" => vec!["install", "-y"],
                "brew" => vec!["install"],
                "scoop" => vec!["install"],
                "choco" => vec!["install", "-y"],
                "apk" => vec!["add"],
                "pkg" => vec!["install", "-y"],
                "pkg_add" => vec!["-I"],
                _ => return Err(format!("Unsupported native package manager: {pm}").into()),
            };

            let package_to_install = if let Some(v) = &dep.version_str {
                println!(
                    "{} Version specifications for native dependencies are not guaranteed to work.",
                    "Warning:".yellow()
                );
                match pm.as_str() {
                    "apt" | "apt-get" | "zypper" => format!("{}={}", dep.package, v),
                    "dnf" | "yum" => format!("{}-{}", dep.package, v),
                    _ => dep.package.to_string(),
                }
            } else {
                dep.package.to_string()
            };

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
            let mut command = Command::new("cargo");
            command.arg("install");
            if let Some(v) = &dep.version_str {
                command.arg("--version").arg(v);
            }
            command.arg(dep.package);
            let status = command.status()?;
            if !status.success() {
                return Err(format!("Cargo dependency failed for '{}'", dep.package).into());
            }
        }
        "cargo-binstall" => {
            let mut command = Command::new("cargo");
            command.arg("binstall");
            if let Some(v) = &dep.version_str {
                command.arg("--version").arg(v);
            }
            command.arg(dep.package);
            let status = command.status()?;
            if !status.success() {
                return Err(
                    format!("cargo-binstall dependency failed for '{}'", dep.package).into(),
                );
            }
        }
        "go" => {
            let package_with_version = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                format!("{}@latest", dep.package)
            };
            let status = Command::new("go")
                .arg("install")
                .arg(package_with_version)
                .status()?;
            if !status.success() {
                return Err(format!("Go dependency failed for '{}'", dep.package).into());
            }
        }
        "npm" | "yarn" | "pnpm" | "bun" | "volta" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let mut command = Command::new(manager);
            match manager {
                "npm" => command.args(["install", "-g"]),
                "yarn" => command.args(["global", "add"]),
                "pnpm" => command.args(["add", "-g"]),
                "bun" => command.args(["install", "-g"]),
                "volta" => command.args(["install"]),
                _ => unreachable!(),
            };
            let status = command.arg(package_to_install).status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "deno" => {
            let package_to_install = if let Some(pkg) = dep.package.strip_prefix("npm-") {
                format!("npm:{}", pkg.trim_start())
            } else if let Some(pkg) = dep.package.strip_prefix("jsr-") {
                format!("jsr:{}", pkg.trim_start())
            } else {
                dep.package.to_string()
            };

            let status = Command::new("deno")
                .arg("install")
                .arg("-g")
                .arg("-A")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("Deno dependency failed for '{}'", dep.package).into());
            }
        }
        "uv" => {
            let status = Command::new("uv")
                .arg("tool")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("uv tool install failed for '{}'", dep.package).into());
            }
        }
        "pip" | "pipx" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}=={}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new(manager)
                .arg("install")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "dart-pub" => {
            let status = Command::new("dart")
                .arg("pub")
                .arg("global")
                .arg("activate")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(
                    format!("dart pub global activate failed for '{}'", dep.package).into(),
                );
            }
        }
        "gem" => {
            let mut command = Command::new("gem");
            command.arg("install");
            if let Some(v) = &dep.version_str {
                command.arg("-v").arg(v);
            }
            command.arg(dep.package);
            let status = command.status()?;
            if !status.success() {
                return Err(format!("gem dependency failed for '{}'", dep.package).into());
            }
        }
        "composer" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}:{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("composer")
                .arg("global")
                .arg("require")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("composer dependency failed for '{}'", dep.package).into());
            }
        }
        "dotnet" => {
            let mut command = Command::new("dotnet");
            command.args(["tool", "install", "-g"]);
            if let Some(v) = &dep.version_str {
                command.arg("--version").arg(v);
            }
            command.arg(dep.package);
            let status = command.status()?;
            if !status.success() {
                return Err(format!("dotnet dependency failed for '{}'", dep.package).into());
            }
        }
        "nix" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for nix are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("nix-env")
                .arg("-iA")
                .arg(format!("nixpkgs.{}", dep.package))
                .status()?;
            if !status.success() {
                return Err(format!("nix dependency failed for '{}'", dep.package).into());
            }
        }
        "jsr" => {
            let status = Command::new("npx")
                .arg("jsr")
                .arg("add")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("JSR dependency failed for '{}'", dep.package).into());
            }
        }
        "apt" | "apt-get" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}={}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("sudo")
                .arg(manager)
                .arg("install")
                .arg("-y")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "pacman" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for pacman are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("pacman")
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pacman dependency failed for '{}'", dep.package).into());
            }
        }
        "yay" | "paru" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for {} are not supported. Installing latest.",
                    "Warning:".yellow(),
                    manager
                );
            }
            let status = Command::new(manager)
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "aur" => {
            let temp_dir = std::env::temp_dir().join(format!("zoi-aur-{}", dep.package));
            if temp_dir.exists() {
                fs::remove_dir_all(&temp_dir)?;
            }
            fs::create_dir_all(&temp_dir)?;

            let url = format!("https://aur.archlinux.org/{}.git", dep.package);
            let clone_status = Command::new("git")
                .arg("clone")
                .arg("--depth=1")
                .arg(&url)
                .arg(&temp_dir)
                .status()?;

            if !clone_status.success() {
                return Err(format!(
                    "Failed to clone AUR package '{}' from '{}'",
                    dep.package, url
                )
                .into());
            }

            let makepkg_status = Command::new("makepkg")
                .arg("-si")
                .arg("--noconfirm")
                .current_dir(&temp_dir)
                .status()?;

            if !makepkg_status.success() {
                return Err(
                    format!("Failed to build and install AUR package '{}'", dep.package).into(),
                );
            }

            fs::remove_dir_all(&temp_dir)?;
        }
        "dnf" | "yum" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}-{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("sudo")
                .arg(manager)
                .arg("install")
                .arg("-y")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "brew" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("brew")
                .arg("install")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("brew dependency failed for '{}'", dep.package).into());
            }
        }
        "brew-cask" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("brew")
                .arg("install")
                .arg("--cask")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("brew --cask dependency failed for '{}'", dep.package).into());
            }
        }
        "mas" => {
            let status = Command::new("mas")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("mas dependency failed for '{}'", dep.package).into());
            }
        }
        "scoop" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("{}@{}", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("scoop")
                .arg("install")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("scoop dependency failed for '{}'", dep.package).into());
            }
        }
        "choco" => {
            let mut command = Command::new("choco");
            command.args(["install", "-y"]);
            if let Some(v) = &dep.version_str {
                command.arg("--version").arg(v);
            }
            command.arg(dep.package);
            let status = command.status()?;
            if !status.success() {
                return Err(format!("choco dependency failed for '{}'", dep.package).into());
            }
        }
        "apk" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for apk are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("apk")
                .arg("add")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("apk dependency failed for '{}'", dep.package).into());
            }
        }
        "xbps" | "xbps-install" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for xbps are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("xbps-install")
                .arg("-S")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("xbps-install dependency failed for '{}'", dep.package).into());
            }
        }
        "eopkg" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for eopkg are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("eopkg")
                .arg("it")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("eopkg dependency failed for '{}'", dep.package).into());
            }
        }
        "guix" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for guix are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("guix")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("guix dependency failed for '{}'", dep.package).into());
            }
        }
        "pkg" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for pkg are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("pkg")
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pkg dependency failed for '{}'", dep.package).into());
            }
        }
        "pkg_add" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for pkg_add are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("pkg_add")
                .arg("-I")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pkg_add dependency failed for '{}'", dep.package).into());
            }
        }
        "zypper" => {
            let package_to_install = if let Some(v) = &dep.version_str {
                format!("'{}={}'", dep.package, v)
            } else {
                dep.package.to_string()
            };
            let status = Command::new("sudo")
                .arg("zypper")
                .arg("install")
                .arg("-y")
                .arg(package_to_install)
                .status()?;
            if !status.success() {
                return Err(format!("zypper dependency failed for '{}'", dep.package).into());
            }
        }
        "portage" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for portage are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("emerge")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("Portage dependency failed for '{}'", dep.package).into());
            }
        }
        "snap" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for snap are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("snap")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("snap dependency failed for '{}'", dep.package).into());
            }
        }
        "flatpak" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for flatpak are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("flatpak")
                .arg("install")
                .arg("flathub")
                .arg(dep.package)
                .arg("-y")
                .status()?;
            if !status.success() {
                return Err(format!("flatpak dependency failed for '{}'", dep.package).into());
            }
        }
        "winget" => {
            let mut command = Command::new("winget");
            command.arg("install").arg(dep.package).arg("--silent");
            if let Some(v) = &dep.version_str {
                command.arg("--version").arg(v);
            }
            command
                .arg("--accept-package-agreements")
                .arg("--accept-source-agreements");
            let status = command.status()?;
            if !status.success() {
                return Err(format!("winget dependency failed for '{}'", dep.package).into());
            }
        }
        "conda" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for conda are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("conda")
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("conda dependency failed for '{}'", dep.package).into());
            }
        }
        "macports" => {
            if dep.version_str.is_some() {
                println!(
                    "{} Version specifications for macports are not supported. Installing latest.",
                    "Warning:".yellow()
                );
            }
            let status = Command::new("sudo")
                .arg("port")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("macports dependency failed for '{}'", dep.package).into());
            }
        }
        "script" => {
            let url = if dep.package.starts_with("http") {
                dep.package.to_string()
            } else {
                format!("https://{}", dep.package)
            };

            let platform = utils::get_platform()?;
            let os = platform.split('-').next().unwrap_or("");
            let platform_ext = if os == "windows" { "ps1" } else { "sh" };

            let final_url = format!("{}.{}", url, platform_ext);
            println!("Executing script from URL: {}", final_url.cyan());

            let temp_dir = Builder::new().prefix("zoi-script-dep").tempdir()?;
            let script_path = temp_dir.path().join(format!("script.{}", platform_ext));

            println!("Downloading script from: {}", final_url.cyan());
            let client = crate::utils::build_blocking_http_client(60)?;
            let mut response = client.get(&final_url).send()?;

            if !response.status().is_success() {
                return Err(
                    format!("Failed to download script: HTTP {}", response.status()).into(),
                );
            }

            let total_size = response.content_length().unwrap_or(0);
            let pb = ProgressBar::new(total_size);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
                .progress_chars("#>- "));

            let mut downloaded_bytes = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                let bytes_read = response.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
                pb.inc(bytes_read as u64);
            }
            pb.finish_with_message("Download complete.");

            fs::write(&script_path, downloaded_bytes)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
            }

            let mut command = if cfg!(target_os = "windows") {
                let mut cmd = Command::new("powershell");
                cmd.arg("-ExecutionPolicy")
                    .arg("Bypass")
                    .arg("-File")
                    .arg(&script_path);
                cmd
            } else {
                let mut cmd = Command::new("bash");
                cmd.arg(&script_path);
                cmd
            };

            let status = command.status()?;
            if !status.success() {
                return Err(format!("Script dependency failed for '{}'", dep.package).into());
            }
        }
        _ => return Err(format!("Unknown package manager in dependency: {}", dep.manager).into()),
    }
    Ok(())
}

pub fn resolve_and_install_required(
    deps: &[String],
    parent_pkg_name: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    if deps.is_empty() {
        return Ok(());
    }

    println!("{}", "Resolving required dependencies...".bold());
    for dep_str in deps {
        let dependency = parse_dependency_string(dep_str)?;
        install_dependency(
            &dependency,
            parent_pkg_name,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
        )?;
    }
    println!("{}", "All required dependencies resolved.".green());
    Ok(())
}

pub fn resolve_and_install_required_options(
    option_groups: &[types::DependencyOptionGroup],
    parent_pkg_name: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
    chosen_options: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    if option_groups.is_empty() {
        return Ok(());
    }

    for group in option_groups {
        println!(
            "[{}] There are {} options available for {}",
            group.name.bold(),
            group.depends.len(),
            group.desc.italic()
        );

        let mut parsed_deps = Vec::new();
        for (i, dep_str) in group.depends.iter().enumerate() {
            let dep = parse_dependency_string(dep_str)?;
            let desc = dep.description.unwrap_or("No description");
            let mut dep_display = format!("{}:{}", dep.manager, dep.package);
            if let Some(req) = &dep.req {
                dep_display.push_str(&req.to_string());
            }
            println!(
                "  {}. {} - {}",
                (i + 1).to_string().cyan(),
                dep_display.bold(),
                desc.italic()
            );
            parsed_deps.push(dep);
        }

        if yes {
            if group.all {
                println!(
                    "--yes provided, installing all options for '{}'",
                    group.name
                );
                for dep in parsed_deps.iter() {
                    chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                    install_dependency(
                        dep,
                        parent_pkg_name,
                        scope,
                        yes,
                        all_optional,
                        processed_deps,
                        installed_deps,
                    )?;
                }
            } else {
                println!(
                    "--yes provided, installing the first option for '{}'",
                    group.name
                );
                if let Some(dep) = parsed_deps.first() {
                    chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                    install_dependency(
                        dep,
                        parent_pkg_name,
                        scope,
                        yes,
                        all_optional,
                        processed_deps,
                        installed_deps,
                    )?;
                }
            }
            continue;
        }

        if group.all {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install (e.g. '1,3', 'all')")
                .default("1".into())
                .interact_text()?;

            let trimmed_input = input.trim().to_lowercase();
            let mut to_install = Vec::new();

            if trimmed_input == "all" {
                to_install.extend(0..parsed_deps.len());
            } else {
                for part in trimmed_input.split([',', ' ']).filter(|s| !s.is_empty()) {
                    if let Ok(num) = part.parse::<usize>() {
                        if num > 0 && num <= parsed_deps.len() {
                            to_install.push(num - 1);
                        } else {
                            return Err(format!("Invalid number: {}", num).into());
                        }
                    } else {
                        return Err(format!("Invalid input: {}", part).into());
                    }
                }
            }
            to_install.sort();
            to_install.dedup();

            for index in to_install {
                let dep = &parsed_deps[index];
                chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                install_dependency(
                    dep,
                    parent_pkg_name,
                    scope,
                    yes,
                    all_optional,
                    processed_deps,
                    installed_deps,
                )?;
            }
        } else {
            let items: Vec<_> = parsed_deps
                .iter()
                .map(|d| {
                    let mut dep_display = format!("{}:{}", d.manager, d.package);
                    if let Some(req) = &d.req {
                        dep_display.push_str(&req.to_string());
                    }
                    dep_display
                })
                .collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install")
                .items(&items)
                .default(0)
                .interact()?;

            let dep = &parsed_deps[selection];
            chosen_options.push(format!("{}:{}", dep.manager, dep.package));
            install_dependency(
                dep,
                parent_pkg_name,
                scope,
                yes,
                all_optional,
                processed_deps,
                installed_deps,
            )?;
        }
    }
    Ok(())
}

pub fn resolve_and_install_optional(
    deps: &[String],
    parent_pkg_name: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
    chosen_optionals: &mut Vec<String>,
    dep_type: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if deps.is_empty() {
        return Ok(());
    }

    let type_str = dep_type.map(|s| format!("{} ", s)).unwrap_or_default();

    if yes || all_optional {
        println!(
            "{}",
            format!("Installing all optional {} dependencies...", type_str).bold()
        );
        for dep_str in deps {
            chosen_optionals.push(dep_str.clone());
            let dependency = parse_dependency_string(dep_str)?;
            install_dependency(
                &dependency,
                parent_pkg_name,
                scope,
                true,
                all_optional,
                processed_deps,
                installed_deps,
            )?;
        }
        return Ok(());
    }

    println!(
        "{}",
        format!("This package has optional {} dependencies:", type_str).bold()
    );
    let mut parsed_deps = Vec::new();
    for (i, dep_str) in deps.iter().enumerate() {
        let dep = parse_dependency_string(dep_str)?;
        let desc = dep.description.unwrap_or("No description");
        let mut dep_display = format!("{}:{}", dep.manager, dep.package);
        if let Some(req) = &dep.req {
            dep_display.push_str(&req.to_string());
        }
        println!(
            "  {}. {} - {}",
            (i + 1).to_string().cyan(),
            dep_display.bold(),
            desc.italic()
        );
        parsed_deps.push(dep);
    }

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose which to install (e.g. '1,3', 'all', 'none')")
        .default("none".into())
        .interact_text()?;

    let trimmed_input = input.trim().to_lowercase();

    if trimmed_input == "none" || trimmed_input == "n" || trimmed_input == "0" {
        println!("Skipping optional dependencies.");
        return Ok(());
    }

    let mut to_install = Vec::new();
    if trimmed_input == "all" || trimmed_input == "y" {
        to_install.extend(0..parsed_deps.len());
    } else {
        for part in trimmed_input.split([',', ' ']).filter(|s| !s.is_empty()) {
            if let Ok(num) = part.parse::<usize>() {
                if num > 0 && num <= parsed_deps.len() {
                    to_install.push(num - 1);
                } else {
                    return Err(format!("Invalid number: {}", num).into());
                }
            } else {
                return Err(format!("Invalid input: {}", part).into());
            }
        }
    }

    to_install.sort();
    to_install.dedup();

    for index in to_install {
        let dep = &parsed_deps[index];
        chosen_optionals.push(format!("{}:{}", dep.manager, dep.package));
        install_dependency(
            dep,
            parent_pkg_name,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
        )?;
    }

    Ok(())
}

fn install_zoi_dependency(
    package_name: &str,
    yes: bool,
    all_optional: bool,
    processed_deps: &mut HashSet<String>,
) -> Result<(), Box<dyn Error>> {
    let resolved_source = resolve::resolve_source(package_name)?;

    install::run_installation(
        resolved_source.path.to_str().unwrap(),
        install::InstallMode::PreferBinary,
        false,
        crate::pkg::types::InstallReason::Dependency,
        yes,
        all_optional,
        processed_deps,
    )
}

type ZoiUninstaller = dyn Fn(&str) -> Result<(), Box<dyn Error>>;

pub fn uninstall_dependency(
    dep_str: &str,
    zoi_uninstaller: &ZoiUninstaller,
) -> Result<(), Box<dyn Error>> {
    let dep = parse_dependency_string(dep_str)?;
    println!(
        "-> Attempting to uninstall dependency: {} via {}",
        dep.package.cyan(),
        dep.manager.yellow()
    );

    let manager = dep.manager;
    let status = match manager {
        "zoi" => {
            let zoi_dep_name = dep.package;
            return zoi_uninstaller(zoi_dep_name);
        }
        "native" => {
            let pm =
                utils::get_native_package_manager().ok_or("Native package manager not found")?;
            println!("(Using native manager: {pm})");
            let (cmd, args) = match pm.as_str() {
                "apt" | "apt-get" => ("sudo", vec![pm.as_str(), "remove", "-y"]),
                "pacman" => ("sudo", vec!["pacman", "-Rns", "--noconfirm"]),
                "dnf" | "yum" => ("sudo", vec![pm.as_str(), "remove", "-y"]),
                "brew" => ("brew", vec!["uninstall"]),
                "scoop" => ("scoop", vec!["uninstall"]),
                "choco" => ("choco", vec!["uninstall", "-y"]),
                "apk" => ("sudo", vec!["apk", "del"]),
                "pkg" => ("sudo", vec!["pkg", "delete", "-y"]),
                "pkg_add" => ("sudo", vec!["pkg_delete"]),
                _ => return Err(format!("Unsupported native package manager: {pm}").into()),
            };

            let mut command = Command::new(cmd);
            command.args(args).arg(dep.package).status()?
        }
        "cargo" | "cargo-binstall" => Command::new("cargo")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
        "go" => {
            println!("Skipping uninstall for Go, please remove binary manually.");
            return Ok(());
        }
        "npm" => Command::new("npm")
            .arg("uninstall")
            .arg("-g")
            .arg(dep.package)
            .status()?,
        "deno" => {
            let package_name = if let Some(pkg) = dep.package.strip_prefix("npm-") {
                pkg.trim_start()
            } else if let Some(pkg) = dep.package.strip_prefix("jsr-") {
                pkg.trim_start()
            } else {
                dep.package
            };

            let executable_name = if let Some(idx) = package_name.rfind('/') {
                &package_name[idx + 1..]
            } else {
                package_name
            };

            Command::new("deno")
                .arg("uninstall")
                .arg(executable_name)
                .status()?
        }
        "yarn" => Command::new("yarn")
            .arg("global")
            .arg("remove")
            .arg(dep.package)
            .status()?,
        "pnpm" => Command::new("pnpm")
            .arg("remove")
            .arg("-g")
            .arg(dep.package)
            .status()?,
        "bun" => Command::new("bun")
            .arg("remove")
            .arg("-g")
            .arg(dep.package)
            .status()?,
        "volta" => {
            println!("Skipping uninstall for Volta, please remove binary manually.");
            return Ok(());
        }
        "pip" | "pipx" => Command::new(manager)
            .arg("uninstall")
            .arg(dep.package)
            .arg("-y")
            .status()?,
        "gem" => Command::new("gem")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
        "composer" => Command::new("composer")
            .arg("global")
            .arg("remove")
            .arg(dep.package)
            .status()?,
        "dotnet" => Command::new("dotnet")
            .arg("tool")
            .arg("uninstall")
            .arg("-g")
            .arg(dep.package)
            .status()?,
        "nix" => Command::new("nix-env")
            .arg("-e")
            .arg(dep.package)
            .status()?,
        "jsr" => {
            println!("Skipping uninstall for JSR.");
            return Ok(());
        }
        "apt" | "apt-get" => Command::new("sudo")
            .arg(manager)
            .arg("remove")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "pacman" => Command::new("sudo")
            .arg("pacman")
            .arg("-Rns")
            .arg("--noconfirm")
            .arg(dep.package)
            .status()?,
        "yay" | "paru" => Command::new(manager)
            .arg("-Rns")
            .arg("--noconfirm")
            .arg(dep.package)
            .status()?,
        "aur" => {
            println!("AUR packages are uninstalled via pacman. Trying with pacman...");
            Command::new("sudo")
                .arg("pacman")
                .arg("-Rns")
                .arg("--noconfirm")
                .arg(dep.package)
                .status()?
        }
        "dnf" | "yum" => Command::new("sudo")
            .arg(manager)
            .arg("remove")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "brew" => Command::new("brew")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
        "brew-cask" => Command::new("brew")
            .arg("uninstall")
            .arg("--cask")
            .arg(dep.package)
            .status()?,
        "mas" => Command::new("mas")
            .arg("remove")
            .arg(dep.package)
            .status()?,
        "scoop" => Command::new("scoop")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
        "choco" => Command::new("choco")
            .arg("uninstall")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "apk" => Command::new("sudo")
            .arg("apk")
            .arg("del")
            .arg(dep.package)
            .status()?,
        "xbps" | "xbps-install" => Command::new("sudo")
            .arg("xbps-remove")
            .arg("-R")
            .arg(dep.package)
            .status()?,
        "eopkg" => Command::new("sudo")
            .arg("eopkg")
            .arg("rm")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "guix" => Command::new("guix")
            .arg("remove")
            .arg(dep.package)
            .status()?,
        "pkg" => Command::new("sudo")
            .arg("pkg")
            .arg("delete")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "pkg_add" => Command::new("sudo")
            .arg("pkg_delete")
            .arg(dep.package)
            .status()?,
        "zypper" => Command::new("sudo")
            .arg("zypper")
            .arg("remove")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "portage" => Command::new("sudo")
            .arg("emerge")
            .arg("--unmerge")
            .arg(dep.package)
            .status()?,
        "snap" => Command::new("sudo")
            .arg("snap")
            .arg("remove")
            .arg(dep.package)
            .status()?,
        "flatpak" => Command::new("flatpak")
            .arg("uninstall")
            .arg(dep.package)
            .arg("-y")
            .status()?,
        "winget" => Command::new("winget")
            .arg("uninstall")
            .arg(dep.package)
            .arg("--silent")
            .status()?,
        "conda" => Command::new("conda")
            .arg("uninstall")
            .arg("-y")
            .arg(dep.package)
            .status()?,
        "macports" => Command::new("sudo")
            .arg("port")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
        "script" => {
            println!("Skipping uninstall for script dependency, as it's a one-time execution.");
            return Ok(());
        }
        _ => return Err(format!("Unknown package manager for uninstall: {}", dep.manager).into()),
    };

    if !status.success() {
        return Err(format!("Failed to uninstall dependency: {}", dep.package).into());
    }

    Ok(())
}
