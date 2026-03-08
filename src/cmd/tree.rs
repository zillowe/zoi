use crate::pkg::install::resolver;
use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashSet;

pub fn run(package_names: &[String]) -> Result<()> {
    if package_names.is_empty() {
        println!("{}", "Please specify at least one package name.".yellow());
        return Ok(());
    }

    println!("{} Resolving dependency tree...", "::".bold().blue());

    let (graph, non_zoi_deps) =
        resolver::resolve_dependency_graph(package_names, None, false, true, true, None, true)?;

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

fn print_node(
    graph: &resolver::DependencyGraph,
    pkg_id: &str,
    prefix: &str,
    is_last: bool,
    visited: &mut HashSet<String>,
) -> Result<()> {
    let node = graph
        .nodes
        .get(pkg_id)
        .ok_or_else(|| anyhow!("Package not found in graph: {}", pkg_id))?;
    let is_repeated = visited.contains(pkg_id);

    let connector = if is_last { "└── " } else { "├── " };

    let pkg_display = if let Some(sub) = &node.sub_package {
        format!("{}:{}", node.pkg.name.cyan().bold(), sub.yellow())
    } else {
        node.pkg.name.cyan().bold().to_string()
    };

    let version_display = format!("v{}", node.version.green());
    let repeated_mark = if is_repeated {
        " (*)".dimmed()
    } else {
        "".normal()
    };

    println!(
        "{}{}{}{} {}",
        prefix, connector, pkg_display, repeated_mark, version_display
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
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            print_node(graph, child_id, &new_prefix, i == child_count - 1, visited)?;
        }
    }

    Ok(())
}
