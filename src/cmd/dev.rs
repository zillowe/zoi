use crate::pkg::{install, local, types};
use crate::project::config as project_config;
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashMap;
use std::process::Command;

pub fn run(run_cmd: Option<String>) -> Result<()> {
    let config = project_config::load()?;
    println!(
        "{} Entering development shell for project: {}",
        "::".bold().blue(),
        config.name.cyan().bold()
    );

    let (graph, _non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &config.pkgs,
        Some(types::Scope::Project),
        false,
        true,
        true,
        None,
        true,
    )?;

    let install_plan = install::plan::create_install_plan(&graph.nodes, None, false)?;
    if !install_plan.is_empty() {
        println!(
            "{} Ensuring project dependencies are installed...",
            "::".bold().blue()
        );
        let m = indicatif::MultiProgress::new();
        let stages = graph.toposort()?;
        for stage in stages {
            use rayon::prelude::*;
            stage.into_par_iter().try_for_each(|pkg_id| -> Result<()> {
                let node = graph
                    .nodes
                    .get(&pkg_id)
                    .ok_or_else(|| anyhow!("Package not found in graph: {}", pkg_id))?;
                let action = install_plan
                    .get(&pkg_id)
                    .ok_or_else(|| anyhow!("Install action not found for: {}", pkg_id))?;
                install::installer::install_node(node, action, Some(&m), None, true, true)?;
                Ok(())
            })?;
        }
    }

    let mut env_vars: HashMap<String, String> = HashMap::new();

    let mut bin_paths = Vec::new();
    let mut lib_paths = Vec::new();
    let mut include_paths = Vec::new();
    let mut pkg_config_paths = Vec::new();

    let sep = if cfg!(windows) { ";" } else { ":" };

    for node in graph.nodes.values() {
        let handle = &node.registry_handle;
        let pkg = &node.pkg;
        let package_dir = local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
        let version_dir = package_dir.join(&node.version);

        let bin_dir = version_dir.join("bin");
        if bin_dir.exists() {
            bin_paths.push(bin_dir);
        }

        let lib_dir = version_dir.join("lib");
        if lib_dir.exists() {
            lib_paths.push(lib_dir.clone());
            let pkgconfig_dir = lib_dir.join("pkgconfig");
            if pkgconfig_dir.exists() {
                pkg_config_paths.push(pkgconfig_dir);
            }
        }

        let include_dir = version_dir.join("include");
        if include_dir.exists() {
            include_paths.push(include_dir);
        }

        let share_dir = version_dir.join("share");
        if share_dir.exists() {
            let pkgconfig_dir = share_dir.join("pkgconfig");
            if pkgconfig_dir.exists() {
                pkg_config_paths.push(pkgconfig_dir);
            }
        }
    }

    if !bin_paths.is_empty() {
        let mut path = bin_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(sep);
        if let Ok(old_path) = std::env::var("PATH") {
            path = format!("{}{}{}", path, sep, old_path);
        }
        env_vars.insert("PATH".to_string(), path);
    }

    if !lib_paths.is_empty() {
        let lib_path_var = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut path = lib_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(sep);
        if let Ok(old_path) = std::env::var(lib_path_var) {
            path = format!("{}{}{}", path, sep, old_path);
        }
        env_vars.insert(lib_path_var.to_string(), path);
    }

    if !include_paths.is_empty() {
        let path = include_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(sep);
        for var in &["CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH"] {
            let mut full_path = path.clone();
            if let Ok(old_path) = std::env::var(var) {
                full_path = format!("{}{}{}", full_path, sep, old_path);
            }
            env_vars.insert(var.to_string(), full_path);
        }
    }

    if !pkg_config_paths.is_empty() {
        let mut path = pkg_config_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(sep);
        if let Ok(old_path) = std::env::var("PKG_CONFIG_PATH") {
            path = format!("{}{}{}", path, sep, old_path);
        }
        env_vars.insert("PKG_CONFIG_PATH".to_string(), path);
    }

    if let Some(shell_spec) = &config.shell {
        let platform = utils::get_platform()?;
        let extra_env = match &shell_spec.env {
            project_config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
            project_config::PlatformOrEnvMap::Platform(p) => p
                .get(&platform)
                .or_else(|| p.get("default"))
                .cloned()
                .unwrap_or_default(),
        };
        for (k, v) in extra_env {
            env_vars.insert(k, v);
        }
    }

    if let Some(cmd_str) = run_cmd {
        println!("{} Running: {}", "::".bold().blue(), cmd_str.cyan());
        let mut child = if cfg!(windows) {
            Command::new("pwsh")
                .arg("-Command")
                .arg(&cmd_str)
                .envs(&env_vars)
                .spawn()?
        } else {
            Command::new("bash")
                .arg("-c")
                .arg(&cmd_str)
                .envs(&env_vars)
                .spawn()?
        };
        let status = child.wait()?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    } else {
        let shell_bin = std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(windows) {
                "pwsh".to_string()
            } else {
                "bash".to_string()
            }
        });

        println!(
            "{} Entering dev shell (type 'exit' to leave)...",
            "::".bold().green()
        );

        let mut child = Command::new(&shell_bin)
            .envs(&env_vars)
            .env("ZOI_SHELL", "dev")
            .spawn()?;

        let _ = child.wait()?;
        println!("{} Exited dev shell.", "::".bold().blue());
    }

    Ok(())
}
