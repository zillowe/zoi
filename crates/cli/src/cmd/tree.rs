//! Logic for the `tree` command.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::pkg::install::resolver;
use crate::pkg::resolve;

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

/// One package in the reverse graph: `@repo/name` or `@repo/name:sub`.
type NodeId = String;

/// Runs `tree --reverse`: for each package, the packages in the synced
/// registries whose runtime dependencies include it, and recursively what
/// depends on those. Like `tree`, it works for any package in a registry,
/// installed or not.
///
/// # Errors
///
/// Returns an error if a package cannot be found or a registry index cannot
/// be read.
pub fn run_reverse(package_names: &[String]) -> Result<()> {
    println!("{} Reading registry indexes...", "::".bold().blue());
    let (dependents, versions) = reverse_index()?;

    for (i, source) in package_names.iter().enumerate() {
        let request = resolve::parse_source_string(source)?;
        let repo = request.repo.clone().or_else(|| {
            versions
                .keys()
                .find(|id| node_name(id) == request.name)
                .map(|id| node_repo(id).to_string())
        });
        let Some(repo) = repo else {
            return Err(anyhow!("Package '{source}' was not found."));
        };
        let root =
            node_id(&repo, &request.name, request.sub_package.as_deref());
        let base = node_id(&repo, &request.name, None);
        let sub_prefix = format!("{base}:");
        let known = versions.keys().chain(dependents.keys()).any(|id| {
            *id == root || *id == base || id.starts_with(&sub_prefix)
        });
        if !known {
            return Err(anyhow!("Package '{source}' was not found."));
        }

        if i > 0 {
            println!();
        }
        print_line("", "", &root, &versions, false);
        let mut visited = HashSet::from([root.clone()]);
        let children = dependents_of(&root, &dependents);
        if children.is_empty() {
            println!("{}", "  (no package depends on it)".dimmed());
        }
        let count = children.len();
        for (j, child) in children.iter().enumerate() {
            print_reverse(
                child,
                "",
                j + 1 == count,
                &dependents,
                &versions,
                &mut visited
            );
        }
    }
    Ok(())
}

/// Builds the reverse dependency graph of every synced registry: for each
/// depended-on package, the packages that need it at runtime, plus each
/// package's indexed version.
fn reverse_index()
-> Result<(HashMap<NodeId, BTreeSet<NodeId>>, HashMap<NodeId, String>)> {
    let config = crate::pkg::config::read_config()?;
    let mut handles = Vec::new();
    if let Some(default) = &config.default_registry {
        handles.push(default.handle.clone());
    }
    handles.extend(config.added_registries.iter().map(|r| r.handle.clone()));

    let mut dependents: HashMap<NodeId, BTreeSet<NodeId>> = HashMap::new();
    let mut versions = HashMap::new();
    for handle in handles.iter().filter(|h| !h.is_empty()) {
        let Ok(entries) = crate::pkg::db::list_runtime_dependencies(handle)
        else {
            continue;
        };
        for entry in entries {
            let id =
                node_id(&entry.repo, &entry.name, entry.sub_package.as_deref());
            if let Some(version) = entry.version {
                versions.insert(id.clone(), version);
            }
            for dep_str in &entry.runtime {
                let Ok(dep) =
                    crate::pkg::dependencies::parse_dependency_string(dep_str)
                else {
                    continue;
                };
                if dep.manager != "zoi" {
                    continue;
                }
                let Ok(req) = resolve::parse_source_string(dep.package) else {
                    continue;
                };
                let repo = req.repo.unwrap_or_else(|| entry.repo.clone());
                let target =
                    node_id(&repo, &req.name, req.sub_package.as_deref());
                if target != id {
                    dependents.entry(target).or_default().insert(id.clone());
                }
            }
        }
    }
    Ok((dependents, versions))
}

/// `@repo/name` or `@repo/name:sub`.
fn node_id(repo: &str, name: &str, sub: Option<&str>) -> NodeId {
    match sub {
        Some(sub) => format!("@{repo}/{name}:{sub}"),
        None => format!("@{repo}/{name}")
    }
}

/// The `name` part of a node id.
fn node_name(id: &str) -> &str {
    let rest = id.rsplit_once('/').map_or(id, |(_, rest)| rest);
    rest.split_once(':').map_or(rest, |(name, _)| name)
}

/// The `repo` part of a node id.
fn node_repo(id: &str) -> &str {
    id.trim_start_matches('@')
        .rsplit_once('/')
        .map_or("", |(repo, _)| repo)
}

/// What depends on `id`. A dependency on the whole package (`@core/gcc`)
/// counts for the package itself; asking about a package without naming a
/// sub-package also includes what depends on any of its sub-packages.
fn dependents_of(
    id: &str,
    dependents: &HashMap<NodeId, BTreeSet<NodeId>>
) -> Vec<NodeId> {
    let mut found: BTreeSet<NodeId> =
        dependents.get(id).cloned().unwrap_or_default();
    if !id.contains(':') {
        let prefix = format!("{id}:");
        for (target, deps) in dependents {
            if target.starts_with(&prefix) {
                found.extend(deps.iter().cloned());
            }
        }
    }
    found.into_iter().collect()
}

/// Prints one line of the reverse tree.
fn print_line(
    prefix: &str,
    connector: &str,
    id: &str,
    versions: &HashMap<NodeId, String>,
    repeated: bool
) {
    let (base, sub) =
        id.split_once(':').map_or((id, None), |(b, s)| (b, Some(s)));
    let display = match sub {
        Some(sub) => format!("{}:{}", base.cyan().bold(), sub.yellow()),
        None => base.cyan().bold().to_string()
    };
    let version = versions
        .get(id)
        .map(|v| format!(" v{v}"))
        .unwrap_or_default();
    let mark = if repeated {
        " (*)".dimmed()
    } else {
        "".normal()
    };
    println!("{prefix}{connector}{display}{mark}{version}");
}

/// Prints a dependent and, recursively, what depends on it.
fn print_reverse(
    id: &str,
    prefix: &str,
    is_last: bool,
    dependents: &HashMap<NodeId, BTreeSet<NodeId>>,
    versions: &HashMap<NodeId, String>,
    visited: &mut HashSet<NodeId>
) {
    let connector = if is_last { "└── " } else { "├── " };
    let repeated = !visited.insert(id.to_string());
    print_line(prefix, connector, id, versions, repeated);
    if repeated {
        return;
    }
    let children = dependents_of(id, dependents);
    let new_prefix =
        format!("{prefix}{}", if is_last { "    " } else { "│   " });
    let count = children.len();
    for (i, child) in children.iter().enumerate() {
        print_reverse(
            child,
            &new_prefix,
            i + 1 == count,
            dependents,
            versions,
            visited
        );
    }
}
