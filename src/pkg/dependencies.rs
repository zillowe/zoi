use crate::pkg::{install, local, resolve, types};
use crate::utils;
use colored::*;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use regex::Regex;
use semver::{Version, VersionReq};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
struct Dependency<'a> {
    manager: &'a str,
    package: &'a str,
    req: Option<VersionReq>,
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
            if main.is_empty() || desc.contains(['=', '>', '<', '~', '^']) {
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

    let (package, req_str) = if let Some(idx) = package_and_version.find(['=', '>', '<', '~', '^'])
    {
        package_and_version.split_at(idx)
    } else {
        (package_and_version, "*")
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

fn install_dependency(
    dep: &Dependency,
    parent_pkg_name: &str,
    scope: types::Scope,
    yes: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let dep_id = format!("{}:{}", dep.manager, dep.package);
    if !processed_deps.insert(dep_id.clone()) {
        return Ok(());
    }

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

    let os = std::env::consts::OS;
    let manager = dep.manager;

    let is_compatible = match manager {
        "brew" | "macports" => os == "macos",
        "scoop" | "choco" | "winget" => os == "windows",
        "apt" | "apt-get" => utils::get_linux_distro_family().as_deref() == Some("debian"),
        "pacman" | "yay" | "paru" | "aur" => {
            utils::get_linux_distro_family().as_deref() == Some("arch")
        }
        "dnf" | "yum" => utils::get_linux_distro_family().as_deref() == Some("fedora"),
        "zypper" => utils::get_linux_distro_family().as_deref() == Some("suse"),
        "apk" => utils::get_linux_distro_family().as_deref() == Some("alpine"),
        "portage" => utils::get_linux_distro_family().as_deref() == Some("gentoo"),
        "snap" | "flatpak" => os == "linux",
        "pkg" => os == "freebsd",
        "pkg_add" => os == "openbsd",
        "zoi" | "cargo" | "native" | "go" | "npm" | "deno" | "jsr" | "bun" | "pip" | "pipx"
        | "cargo-binstall" | "gem" | "yarn" | "pnpm" | "composer" | "dotnet" | "nix" | "conda" => {
            true
        }
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
            let zoi_dep_name = dep.package;
            if let Some(manifest) = local::is_package_installed(zoi_dep_name, scope)? {
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
            install_zoi_dependency(dep.package, yes, processed_deps)?;
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
                return Err(format!("Cargo dependency failed for '{}'", dep.package).into());
            }
        }
        "cargo-binstall" => {
            let status = Command::new("cargo")
                .arg("binstall")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(
                    format!("cargo-binstall dependency failed for '{}'", dep.package).into(),
                );
            }
        }
        "go" => {
            let package_with_version = format!("{}@latest", dep.package);
            let status = Command::new("go")
                .arg("install")
                .arg(package_with_version)
                .status()?;
            if !status.success() {
                return Err(format!("Go dependency failed for '{}'", dep.package).into());
            }
        }
        "npm" => {
            let status = Command::new("npm")
                .arg("install")
                .arg("-g")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("NPM dependency failed for '{}'", dep.package).into());
            }
        }
        "deno" => {
            let status = Command::new("deno")
                .arg("install")
                .arg("-g")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("Deno dependency failed for '{}'", dep.package).into());
            }
        }
        "yarn" => {
            let status = Command::new("yarn")
                .arg("global")
                .arg("add")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("Yarn dependency failed for '{}'", dep.package).into());
            }
        }
        "pnpm" => {
            let status = Command::new("pnpm")
                .arg("add")
                .arg("-g")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pnpm dependency failed for '{}'", dep.package).into());
            }
        }
        "bun" => {
            let status = Command::new("bun")
                .arg("install")
                .arg("-g")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("Bun dependency failed for '{}'", dep.package).into());
            }
        }
        "pip" => {
            let status = Command::new("pip")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pip dependency failed for '{}'", dep.package).into());
            }
        }
        "pipx" => {
            let status = Command::new("pipx")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("pipx dependency failed for '{}'", dep.package).into());
            }
        }
        "gem" => {
            let status = Command::new("gem")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("gem dependency failed for '{}'", dep.package).into());
            }
        }
        "composer" => {
            let status = Command::new("composer")
                .arg("global")
                .arg("require")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("composer dependency failed for '{}'", dep.package).into());
            }
        }
        "dotnet" => {
            let status = Command::new("dotnet")
                .arg("tool")
                .arg("install")
                .arg("-g")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("dotnet dependency failed for '{}'", dep.package).into());
            }
        }
        "nix" => {
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
            let status = Command::new("sudo")
                .arg(manager)
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "pacman" => {
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
        "yay" => {
            let status = Command::new("yay")
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("yay dependency failed for '{}'", dep.package).into());
            }
        }
        "paru" => {
            let status = Command::new("paru")
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("paru dependency failed for '{}'", dep.package).into());
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
            let status = Command::new("sudo")
                .arg(manager)
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("{} dependency failed for '{}'", manager, dep.package).into());
            }
        }
        "brew" => {
            let status = Command::new("brew")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("brew dependency failed for '{}'", dep.package).into());
            }
        }
        "scoop" => {
            let status = Command::new("scoop")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("scoop dependency failed for '{}'", dep.package).into());
            }
        }
        "choco" => {
            let status = Command::new("choco")
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("choco dependency failed for '{}'", dep.package).into());
            }
        }
        "apk" => {
            let status = Command::new("sudo")
                .arg("apk")
                .arg("add")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("apk dependency failed for '{}'", dep.package).into());
            }
        }
        "pkg" => {
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
            let status = Command::new("sudo")
                .arg("zypper")
                .arg("install")
                .arg("-y")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("zypper dependency failed for '{}'", dep.package).into());
            }
        }
        "portage" => {
            let status = Command::new("sudo")
                .arg("emerge")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("Portage dependency failed for '{}'", dep.package).into());
            }
        }
        "snap" => {
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
            let status = Command::new("winget")
                .arg("install")
                .arg(dep.package)
                .arg("--silent")
                .arg("--accept-package-agreements")
                .arg("--accept-source-agreements")
                .status()?;
            if !status.success() {
                return Err(format!("winget dependency failed for '{}'", dep.package).into());
            }
        }
        "conda" => {
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
            let status = Command::new("sudo")
                .arg("port")
                .arg("install")
                .arg(dep.package)
                .status()?;
            if !status.success() {
                return Err(format!("macports dependency failed for '{}'", dep.package).into());
            }
        }
        _ => return Err(format!("Unknown package manager in dependency: {}", dep.manager).into()),
    }
    Ok(())
}

pub fn resolve_and_install_required(
    deps: &[String],
    parent_pkg_name: &str,
    scope: types::Scope,
    yes: bool,
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
    scope: types::Scope,
    yes: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
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
                for dep in &parsed_deps {
                    install_dependency(
                        dep,
                        parent_pkg_name,
                        scope,
                        yes,
                        processed_deps,
                        installed_deps,
                    )?;
                }
            } else {
                println!(
                    "--yes provided, installing the first option for '{}'",
                    group.name
                );
                if let Some(dep) = parsed_deps.get(0) {
                    install_dependency(
                        dep,
                        parent_pkg_name,
                        scope,
                        yes,
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
                            println!("Invalid number: {}", num);
                        }
                    } else {
                        println!("Invalid input: {}", part);
                    }
                }
            }
            to_install.sort();
            to_install.dedup();

            for index in to_install {
                let dep = &parsed_deps[index];
                install_dependency(
                    dep,
                    parent_pkg_name,
                    scope,
                    yes,
                    processed_deps,
                    installed_deps,
                )?;
            }
        } else {
            let items: Vec<_> = parsed_deps
                .iter()
                .map(|d| {
                    let desc = d.description.unwrap_or("No description");
                    let mut dep_display = format!("{}:{}", d.manager, d.package);
                    if let Some(req) = &d.req {
                        dep_display.push_str(&req.to_string());
                    }
                    format!("{} - {}", dep_display, desc)
                })
                .collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install")
                .items(&items)
                .default(0)
                .interact()?;

            let dep = &parsed_deps[selection];
            install_dependency(
                dep,
                parent_pkg_name,
                scope,
                yes,
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
    scope: types::Scope,
    yes: bool,
    processed_deps: &mut HashSet<String>,
    installed_deps: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    if deps.is_empty() {
        return Ok(());
    }

    if yes {
        println!("{}", "Installing all optional dependencies...".bold());
        for dep_str in deps {
            let dependency = parse_dependency_string(dep_str)?;
            install_dependency(
                &dependency,
                parent_pkg_name,
                scope,
                true,
                processed_deps,
                installed_deps,
            )?;
        }
        return Ok(());
    }

    println!("{}", "This package has optional dependencies:".bold());
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

    if trimmed_input == "none" || trimmed_input == "n" {
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
                    println!("Invalid number: {}", num);
                }
            } else {
                println!("Invalid input: {}", part);
            }
        }
    }

    to_install.sort();
    to_install.dedup();

    for index in to_install {
        let dep = &parsed_deps[index];
        install_dependency(
            dep,
            parent_pkg_name,
            scope,
            yes,
            processed_deps,
            installed_deps,
        )?;
    }

    Ok(())
}

fn install_zoi_dependency(
    package_name: &str,
    yes: bool,
    processed_deps: &mut HashSet<String>,
) -> Result<(), Box<dyn Error>> {
    let resolved_source = resolve::resolve_source(package_name)?;

    install::run_installation(
        resolved_source.path.to_str().unwrap(),
        install::InstallMode::PreferBinary,
        false,
        crate::pkg::types::InstallReason::Dependency,
        yes,
        processed_deps,
    )
}

pub fn uninstall_dependency(
    dep_str: &str,
    zoi_uninstaller: &dyn Fn(&str) -> Result<(), Box<dyn Error>>,
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
        "deno" => Command::new("deno")
            .arg("uninstall")
            .arg(dep.package)
            .status()?,
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
        _ => return Err(format!("Unknown package manager for uninstall: {}", dep.manager).into()),
    };

    if !status.success() {
        return Err(format!("Failed to uninstall dependency: {}", dep.package).into());
    }

    Ok(())
}
