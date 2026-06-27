use anyhow::{Result, anyhow};
use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::sync::Mutex;
pub use zoi_core::dependency::Dependency;
use zoi_core::types;
use zoi_core::utils;
use zoi_deps::MANAGERS;
use zoi_resolver::local;
use zoi_resolver::resolve;

pub fn install_dependency(
    dep: &Dependency,
    parent_id: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    installed_deps: &mut Vec<String>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let dep_id = format!("{}:{}", dep.manager, dep.package);
    {
        let mut lock = processed_deps
            .lock()
            .map_err(|e| anyhow!("Mutex poisoned: {}", e))?;
        if !lock.insert(dep_id.clone()) {
            return Ok(());
        }
    }

    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} {msg:30.cyan} [{bar:40.cyan/blue}] {percent}%")?
        .progress_chars("#>-");

    let pb = if let Some(m_inner) = m {
        let pb = m_inner.add(ProgressBar::new(100));
        pb.set_style(pb_style);
        let version_info = dep
            .version_str
            .as_ref()
            .map_or("any".to_string(), |r| r.to_string());
        pb.set_message(format!("{}: {}:{}", dep.manager, dep.package, version_info));
        Some(pb)
    } else {
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
        None
    };

    installed_deps.push(dep_id.clone());

    if dep.manager == "zoi" {
        let res =
            install_zoi_dependency(dep, parent_id, scope, yes, all_optional, processed_deps, m);
        if let Some(p) = pb {
            p.finish();
        }
        return res;
    }

    if let Some(pm_commands) = MANAGERS.get(dep.manager) {
        let is_available = match dep.manager {
            "apt" | "apt-get" => utils::command_exists("apt-get") || utils::command_exists("apt"),
            "dnf" | "yum" => utils::command_exists("dnf") || utils::command_exists("yum"),
            "xbps" | "xbps-install" => utils::command_exists("xbps-install"),
            "aur" => utils::command_exists("makepkg") && utils::command_exists("pacman"),
            _ => utils::command_exists(dep.manager),
        };

        if !is_available {
            let msg = format!(
                "Package manager '{}' not found on this system. Skipping dependency '{}'.",
                dep.manager, dep.package
            );
            if let Some(p) = pb {
                p.println(format!("{}: {}", "Warning".yellow(), msg));
            } else {
                println!("{}: {}", "Warning".yellow(), msg);
            }
            return Ok(());
        }

        if let Some(check_cmd_template) = pm_commands.is_installed {
            let check_cmd = check_cmd_template.replace("{package}", dep.package);
            if utils::run_shell_command_quietly(&check_cmd).is_ok() {
                if let Some(p) = pb {
                    p.set_message(format!("{} (already installed)", dep.package));
                    p.finish();
                } else {
                    println!("Already installed. Skipping.");
                }
                return Ok(());
            }
        }

        if let Some(p) = &pb {
            p.set_position(20);
        }

        if dep.manager == "aur" {
            let res = install_aur_dependency(dep, yes);
            if let Some(p) = pb {
                p.finish();
            }
            return res;
        }

        let package_with_version = if let Some(v) = &dep.version_str {
            match dep.manager {
                "apt" | "apt-get" | "zypper" => format!("{}={}", dep.package, v),
                "dnf" | "yum" => format!("{}-{}", dep.package, v),
                "pip" | "pipx" => format!("{}=={}", dep.package, v),
                _ => format!("{}@{}", dep.package, v),
            }
        } else {
            dep.package.to_string()
        };

        let mut install_cmd = pm_commands
            .install
            .replace("{package}", dep.package)
            .replace("{package_with_version}", &package_with_version);

        if install_cmd.starts_with('#') {
            return Err(anyhow!(
                "Installation command for '{}' is not configured: {}",
                dep.manager,
                install_cmd
            ));
        }

        if pm_commands.sudo_install && !utils::is_admin() {
            if utils::command_exists("sudo") {
                install_cmd = format!("sudo {}", install_cmd);
            } else {
                eprintln!(
                    "{}: sudo is required for '{}' but not found. Attempting to run without sudo...",
                    "Warning".yellow(),
                    dep.manager
                );
            }
        }

        if pb.is_none() {
            println!("Running install command: {}", install_cmd.italic());
        }
        let res = utils::run_shell_command(&install_cmd);
        if let Some(p) = pb {
            p.set_position(100);
            p.finish();
        }
        res
    } else if dep.manager == "native" {
        let pm = utils::get_native_package_manager()
            .ok_or_else(|| anyhow!("Native package manager not found for this OS"))?;
        if pb.is_none() {
            println!("-> Using native package manager: {}", pm.cyan());
        }
        let native_dep_str = format!("{}:{}", pm, dep.package);
        let native_dep = zoi_deps::parse_dependency_string(&native_dep_str)?;
        if let Some(p) = pb {
            p.finish_and_clear();
        }
        install_dependency(
            &native_dep,
            parent_id,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
            m,
        )
    } else {
        Err(anyhow!(
            "Unknown or unsupported package manager in dependency: {}",
            dep.manager
        ))
    }
}

fn install_aur_dependency(dep: &Dependency, yes: bool) -> Result<()> {
    if !utils::command_exists("git") {
        return Err(anyhow!(
            "'git' command not found. Needed for AUR installation."
        ));
    }
    if !utils::command_exists("makepkg") {
        return Err(anyhow!(
            "'makepkg' command not found. Are you on Arch Linux?"
        ));
    }

    let temp_dir = tempfile::Builder::new().prefix("zoi-aur-").tempdir()?;
    let repo_url = format!("https://aur.archlinux.org/{}.git", dep.package);

    println!("-> Cloning {}...", repo_url.cyan());
    let clone_status = std::process::Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(&repo_url)
        .arg(temp_dir.path())
        .status()?;

    if !clone_status.success() {
        return Err(anyhow!(
            "Failed to clone AUR repository for {}",
            dep.package
        ));
    }

    println!("-> Building and installing with makepkg...");
    let mut makepkg_cmd = std::process::Command::new("makepkg");
    makepkg_cmd.arg("-si").current_dir(temp_dir.path());
    if yes {
        makepkg_cmd.arg("--noconfirm");
    }

    let install_status = makepkg_cmd.status()?;
    if !install_status.success() {
        return Err(anyhow!("makepkg failed for {}", dep.package));
    }

    Ok(())
}

fn install_zoi_dependency(
    dep: &Dependency,
    parent_id: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    _processed_deps: &Mutex<HashSet<String>>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let zoi_dep_name = if let Some(v) = &dep.version_str {
        format!("{}@{}", dep.package, v)
    } else {
        dep.package.to_string()
    };
    let req = resolve::parse_source_string(&zoi_dep_name)?;
    let mut manifests = local::find_installed_manifests_matching(&req, scope)?;
    if let Some(manifest) = if manifests.len() == 1 {
        Some(manifests.remove(0))
    } else {
        None
    } {
        println!(
            "Zoi package '{}' is already installed (version {}). Skipping.",
            dep.package, manifest.version
        );
        let package_dir = local::get_package_dir(
            scope,
            &manifest.registry_handle,
            &manifest.repo,
            &manifest.name,
        )?;
        local::add_dependent(&package_dir, parent_id)?;
        return Ok(());
    }

    println!("Not installed. Proceeding with zoi installation...");

    let (graph, _) = match crate::resolver::resolve_dependency_graph(
        &[zoi_dep_name.to_string()],
        Some(scope),
        false,
        yes,
        all_optional,
        None,
        true,
    ) {
        Ok(res) => res,
        Err(e) => {
            return Err(anyhow!(
                "Failed to resolve dependency graph for '{}': {}",
                zoi_dep_name,
                e
            ));
        }
    };

    if graph.nodes.is_empty() {
        return Ok(());
    }

    let install_plan = match crate::plan::create_install_plan(&graph.nodes, None, false) {
        Ok(plan) => plan,
        Err(e) => {
            return Err(anyhow!(
                "Failed to create install plan for '{}': {}",
                zoi_dep_name,
                e
            ));
        }
    };

    let stages = graph.toposort()?;
    for stage in stages {
        for id in stage {
            let node = graph
                .nodes
                .get(&id)
                .ok_or_else(|| anyhow!("Package not found in graph: {}", id))?;
            let action = install_plan
                .get(&id)
                .ok_or_else(|| anyhow!("Could not find install action for {}", id))?;
            crate::installer::install_node(node, action, m, None, yes, true, true)?;
        }
    }

    Ok(())
}
