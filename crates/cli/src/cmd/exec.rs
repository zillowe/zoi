use crate::pkg::{install, local};
use anyhow::{Result, anyhow};
use colored::*;
use indicatif::MultiProgress;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

pub fn run(source: String, bin: Option<String>, args: Vec<String>, verbose: bool) -> Result<()> {
    if verbose {
        println!("{} Resolving package...", "::".bold().blue());
    }

    let installed_before: HashSet<String> = local::get_installed_packages()?
        .into_iter()
        .map(|m| local::installed_manifest_source(&m))
        .collect();

    let (graph, _non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &[source],
        None,
        false,
        true,
        true,
        None,
        !verbose,
    )?;

    let install_plan = install::plan::create_install_plan(&graph.nodes, None, false)?;
    let stages = graph.toposort()?;

    let mut session_installed = Vec::new();
    if !install_plan.is_empty() {
        if verbose {
            println!("\n{} Preparing packages...", "::".bold().blue());
        }
        let m_prep = MultiProgress::new();
        if !verbose {
            m_prep.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        }
        let prepared_nodes = Mutex::new(HashMap::new());

        stages
            .par_iter()
            .flatten()
            .try_for_each(|pkg_id| -> Result<()> {
                let node = graph.nodes.get(pkg_id).ok_or_else(|| {
                    anyhow!(
                        "Package node missing from graph for '{}' during preparation",
                        pkg_id
                    )
                })?;
                let action = install_plan.get(pkg_id).ok_or_else(|| {
                    anyhow!(
                        "Install action missing for package '{}' during preparation",
                        pkg_id
                    )
                })?;

                let prepared =
                    install::installer::prepare_node(node, action, Some(&m_prep), None, verbose)?;

                let mut lock = prepared_nodes.lock().map_err(|e| {
                    anyhow!("Prepared nodes mutex poisoned during preparation: {}", e)
                })?;
                lock.insert(pkg_id.clone(), prepared);
                Ok(())
            })?;

        if verbose {
            println!(
                "{} Installing {} packages...",
                "::".bold().blue(),
                install_plan.len()
            );
        }
        let m = indicatif::MultiProgress::new();
        if !verbose {
            m.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        }
        let session_installed_mutex = Mutex::new(Vec::new());

        for stage in stages {
            stage.into_par_iter().try_for_each(|pkg_id| -> Result<()> {
                let node = graph
                    .nodes
                    .get(&pkg_id)
                    .ok_or_else(|| anyhow!("Package node missing from graph for '{}'", pkg_id))?;

                let prepared = {
                    let lock = prepared_nodes.lock().map_err(|e| {
                        anyhow!("Prepared nodes mutex poisoned during install: {}", e)
                    })?;
                    lock.get(&pkg_id)
                        .cloned()
                        .ok_or_else(|| anyhow!("Prepared node missing for: {}", pkg_id))?
                };

                let manifest = install::installer::install_prepared_node(
                    node,
                    &prepared,
                    Some(&m),
                    true,
                    false,
                    false,
                    verbose,
                )?;

                let mut session_lock = session_installed_mutex.lock().unwrap();
                session_lock.push(manifest);
                Ok(())
            })?;
        }
        session_installed = session_installed_mutex.into_inner().unwrap();
    }

    let root_ids: Vec<&String> = graph
        .adj
        .get("$root")
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    let root_id = root_ids
        .first()
        .ok_or_else(|| anyhow!("Could not find root package in dependency graph"))?;
    let node = graph
        .nodes
        .get(*root_id)
        .ok_or_else(|| anyhow!("Root package node not found"))?;

    let bin_name = match bin {
        Some(name) => name,
        None => {
            let bins = node
                .pkg
                .bins
                .as_ref()
                .ok_or_else(|| anyhow!("Package '{}' provides no binaries", node.pkg.name))?;
            if bins.len() == 1 {
                bins[0].clone()
            } else {
                return Err(anyhow!(
                    "Package '{}' provides multiple binaries ({}). Use --bin to specify which to run.",
                    node.pkg.name,
                    bins.join(", ")
                ));
            }
        }
    };

    let temp_dir = tempfile::Builder::new().prefix("zoi-exec-").tempdir()?;
    let temp_bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&temp_bin_dir)?;

    for gnode in graph.nodes.values() {
        let Ok(pkg_dir) = local::get_package_dir(
            gnode.pkg.scope,
            &gnode.registry_handle,
            &gnode.pkg.repo,
            &gnode.pkg.name,
        ) else {
            continue;
        };
        let version_dir = pkg_dir.join(&gnode.version);
        let bin_dir = version_dir.join("bin");
        if bin_dir.exists() {
            for entry in fs::read_dir(bin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if (path.is_file() || path.is_symlink())
                    && let Some(file_name) = path.file_name()
                {
                    let dest = temp_bin_dir.join(file_name);
                    let _ = fs::remove_file(&dest);
                    let _ = crate::utils::symlink_file(&path, &dest);
                }
            }
        }
    }

    let package_dir = local::get_package_dir(
        node.pkg.scope,
        &node.registry_handle,
        &node.pkg.repo,
        &node.pkg.name,
    )?;
    let version_dir = package_dir.join(&node.version);
    let actual_bin_path = version_dir.join("bin").join(&bin_name);

    if !actual_bin_path.exists() {
        return Err(anyhow!(
            "Binary '{}' not found in package '{}'",
            bin_name,
            node.pkg.name
        ));
    }

    let mut new_path = temp_bin_dir.to_string_lossy().to_string();
    if let Ok(old_path) = std::env::var("PATH") {
        new_path = format!(
            "{}{}{}",
            new_path,
            if cfg!(windows) { ";" } else { ":" },
            old_path
        );
    }

    if verbose {
        println!(
            "{} Running '{}' from '{}'...",
            "::".bold().blue(),
            bin_name.cyan(),
            node.pkg.name.cyan()
        );
    }

    let mut envs = HashMap::new();
    envs.insert("PATH".to_string(), new_path);

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let sysroot = zoi_core::sysroot::get_sysroot();
        if let Some(root) = sysroot {
            if verbose {
                println!(
                    "{} Executing within sysroot: {}",
                    "::".bold().yellow(),
                    root.display()
                );
            }

            let extra_binds = vec![(temp_dir.path().to_path_buf(), temp_dir.path().to_path_buf())];

            let exe_inside_root = actual_bin_path
                .strip_prefix(&root)
                .map(PathBuf::from)
                .unwrap_or(actual_bin_path.clone());

            crate::sandbox::wrap_command_in_root(
                &root,
                &exe_inside_root,
                &args,
                &envs,
                &extra_binds,
            )?
        } else if let Some(sandbox_config) = &node.pkg.sandbox
            && sandbox_config.enabled
        {
            if verbose {
                println!("{} Sandboxing with bwrap.", "::".bold().yellow());
            }
            crate::sandbox::wrap_command(&actual_bin_path, &args, sandbox_config, &version_dir)?
        } else {
            let mut c = Command::new(&actual_bin_path);
            c.args(&args);
            c.envs(&envs);
            c
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let mut c = Command::new(&actual_bin_path);
        c.args(&args);
        c.envs(&envs);
        c
    };

    let status = cmd.status()?;

    if !session_installed.is_empty() {
        if verbose {
            println!("{} Cleaning up ephemeral packages...", "::".bold().blue());
        }
        for manifest in session_installed {
            let ident = crate::pkg::local::installed_manifest_source(&manifest);
            if installed_before.contains(&ident) {
                continue;
            }
            let version_dir = match get_version_dir_from_manifest(&manifest) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Warning: failed to resolve path for {}: {}", ident, e);
                    continue;
                }
            };
            if version_dir.exists()
                && let Err(e) = fs::remove_dir_all(&version_dir)
            {
                eprintln!(
                    "Warning: failed to cleanup ephemeral package {}: {}",
                    ident, e
                );
            }
            let package_dir = version_dir.parent().unwrap().to_path_buf();
            if let Ok(mut entries) = fs::read_dir(&package_dir) {
                let has_other_entries = entries.any(|e| {
                    e.as_ref()
                        .is_ok_and(|e| e.file_name() != "latest" && e.file_name() != "dependents")
                });
                if !has_other_entries {
                    let _ = fs::remove_dir_all(&package_dir);
                }
            }
        }
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn get_version_dir_from_manifest(manifest: &zoi_core::types::InstallManifest) -> Result<PathBuf> {
    crate::pkg::local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version,
    )
}
