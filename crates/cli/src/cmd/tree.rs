//! Logic for the `tree` command.

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::pkg::install::resolver;
use crate::pkg::{local, resolve, types};

/// Runs the `tree` command to display the dependency tree of one or more
/// packages.
///
/// # Errors
///
/// Returns an error if the dependency tree cannot be resolved.
pub fn run(package_names: &[String]) -> Result<()> {
    if package_names.is_empty() {
        println!("{}", "Please specify at least one package name.".yellow());
        return Ok(());
    }

    println!("{} Resolving dependency tree...", "::".bold().blue());

    let (graph, non_zoi_deps) = resolver::resolve_dependency_graph(
        package_names,
        None,
        false,
        true,
        true,
        None,
        true,
        None
    )?;

    if !non_zoi_deps.is_empty() {
        println!(
            "\n{} External dependencies (non-Zoi):",
            "::".bold().yellow()
        );
        for dep in non_zoi_deps {
            println!("  - {}", dep.dimmed());
        }
    }

    println!("\n{} Dependency tree:", "::".bold().blue());

    let mut visited = HashSet::new();
    for source in package_names {
        if let Some(children) = graph.adj.get("$root") {
            for pkg_id in children {
                let Some(node) = graph.nodes.get(pkg_id) else {
                    continue;
                };
                if source.contains(&node.pkg.name) {
                    print_node(&graph, pkg_id, "", true, &mut visited)?;
                }
            }
        }
    }

    Ok(())
}

/// Recursively prints a node in the dependency tree.
fn print_node(
    graph: &resolver::DependencyGraph,
    pkg_id: &str,
    prefix: &str,
    is_last: bool,
    visited: &mut HashSet<String>
) -> Result<()> {
    let node = graph
        .nodes
        .get(pkg_id)
        .ok_or_else(|| anyhow!("Package not found in graph: {pkg_id}"))?;
    let is_repeated = visited.contains(pkg_id);

    let connector = if is_last { "└── " } else { "├── " };

    let pkg_display = if let Some(sub) = &node.sub_package {
        format!("{}:{}", node.pkg.name.cyan().bold(), sub.yellow())
    } else {
        node.pkg.name.cyan().bold().to_string()
    };

    let version_display = format!("v{}", node.version);
    let repeated_mark = if is_repeated {
        " (*)".dimmed()
    } else {
        "".normal()
    };

    println!(
        "{prefix}{connector}{pkg_display}{repeated_mark} {version_display}"
    );

    if is_repeated {
        return Ok(());
    }
    visited.insert(pkg_id.to_string());

    if let Some(children) = graph.adj.get(pkg_id) {
        let child_count = children.len();
        let mut sorted_children: Vec<_> = children.iter().collect();
        sorted_children.sort();

        for (i, child_id) in sorted_children.iter().enumerate() {
            let new_prefix =
                format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            print_node(
                graph,
                child_id,
                &new_prefix,
                i == child_count - 1,
                visited
            )?;
        }
    }

    Ok(())
}

/// Runs `tree --reverse`: for each installed package, the installed packages
/// that depend on it, and recursively what depends on those.
///
/// It reads the dependent records the store keeps for every package (the
/// same ones `zoi why` and uninstall checks use), so it shows what is
/// installed, not what a registry would resolve.
///
/// # Errors
///
/// Returns an error if a package is not installed or the store cannot be
/// read.
pub fn run_reverse(package_names: &[String]) -> Result<()> {
    for (i, name) in package_names.iter().enumerate() {
        let manifest = installed_manifest(name)?
            .ok_or_else(|| anyhow!("Package '{name}' is not installed."))?;
        if i > 0 {
            println!();
        }
        println!(
            "{} {}",
            local::installed_manifest_source(&manifest).cyan().bold(),
            format!("v{}", manifest.version).dimmed()
        );
        let mut visited = HashSet::new();
        visited.insert(local::installed_manifest_source(&manifest));
        let dependents = sorted_dependents(&manifest)?;
        if dependents.is_empty() {
            println!("{}", "  (nothing installed depends on it)".dimmed());
        }
        let count = dependents.len();
        for (j, dependent) in dependents.iter().enumerate() {
            print_dependent(dependent, "", j + 1 == count, &mut visited)?;
        }
    }
    Ok(())
}

/// The installed manifest a package source refers to, searching every
/// scope, or `None` when it is not installed.
fn installed_manifest(source: &str) -> Result<Option<types::InstallManifest>> {
    let request = resolve::parse_source_string(source)?;
    let mut candidates = Vec::new();
    for scope in [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project
    ] {
        candidates
            .extend(local::find_installed_manifests_matching(&request, scope)?);
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    crate::cmd::installed_select::choose_installed_manifest(
        source,
        &candidates,
        false
    )
    .map(Some)
}

/// A package's dependent records (`#handle@repo/name:sub@version`), sorted.
fn sorted_dependents(manifest: &types::InstallManifest) -> Result<Vec<String>> {
    let package_dir = local::get_package_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name
    )?;
    let mut dependents = local::get_dependents(&package_dir)?;
    dependents.sort();
    dependents.dedup();
    Ok(dependents)
}

/// Prints one dependent and, recursively, what depends on it.
fn print_dependent(
    source: &str,
    prefix: &str,
    is_last: bool,
    visited: &mut HashSet<String>
) -> Result<()> {
    let connector = if is_last { "└── " } else { "├── " };
    let repeated = !visited.insert(source.to_string());
    let mark = if repeated {
        " (*)".dimmed()
    } else {
        "".normal()
    };
    println!("{prefix}{connector}{}{mark}", source.cyan());
    if repeated {
        return Ok(());
    }

    // A record can outlive its package (removed without cleaning up); show
    // it, but there is nothing to follow.
    let Some(manifest) = installed_manifest(source).ok().flatten() else {
        return Ok(());
    };
    let dependents = sorted_dependents(&manifest)?;
    let new_prefix =
        format!("{prefix}{}", if is_last { "    " } else { "│   " });
    let count = dependents.len();
    for (i, dependent) in dependents.iter().enumerate() {
        print_dependent(dependent, &new_prefix, i + 1 == count, visited)?;
    }
    Ok(())
}
