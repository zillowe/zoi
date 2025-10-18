use crate::pkg::{
    dependencies, local, resolve,
    types::{self, InstallReason, Package},
};
use anyhow::{Result, anyhow};
use colored::*;
use semver::{Version, VersionReq};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct InstallNode {
    pub pkg: Package,
    pub version: String,
    pub reason: InstallReason,
    pub source: String,
    pub registry_handle: String,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
}

#[derive(Default, Debug)]
pub struct DependencyGraph {
    pub nodes: HashMap<String, InstallNode>,
    adj: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toposort(&self) -> Result<Vec<Vec<String>>> {
        let mut in_degree: HashMap<String, usize> =
            self.nodes.keys().map(|id| (id.clone(), 0)).collect();
        let mut rev_adj: HashMap<String, Vec<String>> = HashMap::new();

        for (from, to_set) in &self.adj {
            for to in to_set {
                *in_degree.get_mut(to).unwrap() += 1;
                rev_adj.entry(to.clone()).or_default().push(from.clone());
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, d)| *d == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut stages = Vec::new();
        let mut count = 0;

        while !queue.is_empty() {
            let mut stage = Vec::new();
            for _ in 0..queue.len() {
                let u = queue.pop_front().unwrap();
                stage.push(u.clone());
                count += 1;

                if let Some(neighbors) = self.adj.get(&u) {
                    for v_id in neighbors {
                        let degree = in_degree.get_mut(v_id).unwrap();
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(v_id.clone());
                        }
                    }
                }
            }
            stages.push(stage);
        }

        if count != self.nodes.len() {
            return Err(anyhow!("Cycle detected in dependency graph."));
        }

        Ok(stages)
    }
}

pub fn resolve_dependency_graph(
    initial_sources: &[String],
    scope_override: Option<types::Scope>,
    force: bool,
    yes: bool,
    all_optional: bool,
) -> Result<DependencyGraph> {
    let mut graph = DependencyGraph::new();
    let mut queue: VecDeque<(String, Option<String>)> =
        initial_sources.iter().map(|s| (s.clone(), None)).collect();
    let mut processed_sources = HashSet::new();

    while let Some((source, parent_id)) = queue.pop_front() {
        let (mut pkg, version, _, pkg_lua_path, registry_handle) =
            match resolve::resolve_package_and_version(&source) {
                Ok(res) => res,
                Err(e) => return Err(anyhow!("Failed to resolve '{}': {}", source, e)),
            };

        let handle = registry_handle.as_deref().unwrap_or("local");
        if let Some(scope) = scope_override {
            pkg.scope = scope;
        }

        let pkg_id = format!("{}@{}", pkg.name, version);

        if let Some(parent_id) = &parent_id {
            graph
                .adj
                .entry(parent_id.clone())
                .or_default()
                .insert(pkg_id.clone());
        }

        if graph.nodes.contains_key(&pkg_id) || processed_sources.contains(&source) {
            continue;
        }

        processed_sources.insert(source.clone());

        if !force
            && let Some(installed) = local::is_package_installed(&pkg.name, pkg.scope)?
            && let (Ok(installed_v), Ok(req_v)) = (
                Version::parse(&installed.version),
                VersionReq::parse(&version),
            )
            && req_v.matches(&installed_v)
        {
            println!(
                "Already installed: {} ({}) satisfies {}. Skipping.",
                pkg.name.cyan(),
                installed.version.yellow(),
                version.yellow()
            );
            continue;
        }

        let mut chosen_options = Vec::new();
        let mut chosen_optionals = Vec::new();

        if let Some(deps) = &pkg.dependencies {
            let mut deps_to_process = Vec::new();
            if let Some(runtime) = &deps.runtime {
                deps_to_process.extend(runtime.get_required_simple());

                let options =
                    dependencies::prompt_for_options(&runtime.get_required_options(), yes)?;
                chosen_options.extend(options.clone());
                deps_to_process.extend(options);

                let optionals = dependencies::prompt_for_optionals(
                    runtime.get_optional(),
                    Some("runtime"),
                    yes,
                    all_optional,
                )?;
                chosen_optionals.extend(optionals.clone());
                deps_to_process.extend(optionals);
            }

            if let Some(build) = &deps.build {
                let needs_build = pkg.types.contains(&"source".to_string())
                    && !pkg.types.contains(&"pre-compiled".to_string());
                if needs_build {
                    println!("Resolving build dependencies for {}", pkg.name.cyan());
                    deps_to_process.extend(build.get_required_simple());

                    let options =
                        dependencies::prompt_for_options(&build.get_required_options(), yes)?;
                    chosen_options.extend(options.clone());
                    deps_to_process.extend(options);

                    let optionals = dependencies::prompt_for_optionals(
                        build.get_optional(),
                        Some("build"),
                        yes,
                        all_optional,
                    )?;
                    chosen_optionals.extend(optionals.clone());
                    deps_to_process.extend(optionals);
                }
            }

            for dep_source in deps_to_process {
                queue.push_back((dep_source, Some(pkg_id.clone())));
            }
        }

        let node = InstallNode {
            pkg: pkg.clone(),
            version,
            reason: if let Some(parent) = parent_id {
                InstallReason::Dependency { parent }
            } else {
                InstallReason::Direct
            },
            source: pkg_lua_path.to_string_lossy().to_string(),
            registry_handle: handle.to_string(),
            chosen_options,
            chosen_optionals,
        };

        graph.nodes.insert(pkg_id, node);
    }

    Ok(graph)
}
