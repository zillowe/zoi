//! Implementation of the `zoi exec` command.
//!
//! This command allows running a binary from a package without explicitly
//! installing it into the global or user scope. It handles temporary
//! installation and cleanup.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use rayon::prelude::*;

use crate::pkg::{install, local};

/// Runs the 'exec' command.
///
/// Temporarily installs a package if necessary and executes a binary from it.
/// Ephemeral packages are cleaned up after execution.
///
/// # Panics
///
/// This function may panic if internal mutexes are poisoned.
///
/// # Errors
///
/// Returns an error if:
/// - Package resolution fails.
/// - Installation of temporary packages fails.
/// - Execution of the binary fails.
/// - Cleanup of ephemeral packages fails.
pub fn run(
    source: &str,
    bin: Option<String>,
    args: &[String],
    verbose: bool
) -> Result<()> {
    if verbose {
        println!("{} Resolving package...", "::".bold().blue());
    }

    // Ephemeral packages are written into the shared store, so the store
    // lock is held while they are resolved and installed. It is released
    // before the user's program runs: holding it for the lifetime of an
    // interactive session would block every other Zoi command.
    let install_lock = crate::pkg::lock::acquire_lock()?;

    let installed_manifests = local::get_installed_packages()?;

    let (graph, _non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &[source.to_string()],
        None,
        false,
        true,
        true,
        None,
        !verbose,
        None
    )?;

    let mut install_plan =
        install::plan::create_install_plan(&graph.nodes, None, false)?;
    crate::cmd::shell::drop_already_installed(
        &mut install_plan,
        &graph.nodes,
        &installed_manifests
    );
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
                        "Package node missing from graph for '{pkg_id}' \
                         during preparation"
                    )
                })?;
                // Packages that are already installed have no plan entry.
                let Some(action) = install_plan.get(pkg_id) else {
                    return Ok(());
                };

                let prepared = install::installer::prepare_node(
                    node,
                    action,
                    Some(&m_prep),
                    None,
                    verbose,
                    false
                )?;

                let mut lock = prepared_nodes.lock().map_err(|e| {
                    anyhow!(
                        "Prepared nodes mutex poisoned during preparation: {e}"
                    )
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
                if !install_plan.contains_key(&pkg_id) {
                    return Ok(());
                }
                let node = graph.nodes.get(&pkg_id).ok_or_else(|| {
                    anyhow!("Package node missing from graph for '{pkg_id}'")
                })?;

                let prepared = {
                    let lock = prepared_nodes.lock().map_err(|e| {
                        anyhow!(
                            "Prepared nodes mutex poisoned during install: {e}"
                        )
                    })?;
                    lock.get(&pkg_id).cloned().ok_or_else(|| {
                        anyhow!("Prepared node missing for: {pkg_id}")
                    })?
                };

                let manifest = install::installer::install_prepared_node(
                    node,
                    &prepared,
                    Some(&m),
                    true,
                    false,
                    false,
                    verbose
                )?;

                let mut session_lock =
                    session_installed_mutex.lock().map_err(|e| {
                        anyhow!("Session installed mutex poisoned: {e}")
                    })?;
                session_lock.push(manifest);
                Ok(())
            })?;
        }
        session_installed = session_installed_mutex
            .into_inner()
            .map_err(|e| anyhow!("Session installed mutex poisoned: {e}"))?;
    }

    let root_ids: Vec<&String> = graph
        .adj
        .get("$root")
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    let root_id = root_ids.first().ok_or_else(|| {
        anyhow!("Could not find root package in dependency graph")
    })?;
    let node = graph
        .nodes
        .get(*root_id)
        .ok_or_else(|| anyhow!("Root package node not found"))?;

    let bin_name = if let Some(name) = bin {
        name
    } else {
        let bins = node.pkg.bins.as_ref().ok_or_else(|| {
            anyhow!("Package '{}' provides no binaries", node.pkg.name)
        })?;
        if bins.len() == 1 {
            bins.first()
                .ok_or_else(|| {
                    anyhow!(
                        "Package '{}' binaries list is empty",
                        node.pkg.name
                    )
                })?
                .clone()
        } else {
            return Err(anyhow!(
                "Package '{}' provides multiple binaries ({}). Use --bin to \
                 specify which to run.",
                node.pkg.name,
                bins.join(", ")
            ));
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
            &gnode.pkg.name
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
        &node.pkg.name
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

            let extra_binds = vec![(
                temp_dir.path().to_path_buf(),
                temp_dir.path().to_path_buf()
            )];

            let exe_inside_root = actual_bin_path
                .strip_prefix(&root)
                .map(PathBuf::from)
                .unwrap_or(actual_bin_path.clone());

            crate::sandbox::wrap_command_in_root(
                &root,
                &exe_inside_root,
                args,
                &envs,
                &extra_binds,
                false
            )?
        } else if let Some(sandbox_config) = &node.pkg.sandbox
            && sandbox_config.enabled
        {
            if verbose {
                println!("{} Sandboxing with bwrap.", "::".bold().yellow());
            }
            crate::sandbox::wrap_command(
                &actual_bin_path,
                args,
                sandbox_config,
                &version_dir
            )?
        } else {
            let mut c = Command::new(&actual_bin_path);
            c.args(args);
            c.envs(&envs);
            c
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let mut c = Command::new(&actual_bin_path);
        c.args(args);
        c.envs(&envs);
        c
    };

    drop(install_lock);
    let status = cmd.status()?;

    // Cleanup removes store directories, so it needs the lock back. If
    // another Zoi process holds it now, skipping cleanup is safer than
    // deleting files underneath it.
    let cleanup_lock = if session_installed.is_empty() {
        None
    } else {
        match crate::pkg::lock::acquire_lock() {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!(
                    "Warning: skipping cleanup of ephemeral packages: {e}"
                );
                session_installed.clear();
                None
            }
        }
    };

    crate::cmd::shell::cleanup_ephemeral_installs(
        session_installed,
        &installed_manifests,
        verbose
    );

    drop(cleanup_lock);

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
