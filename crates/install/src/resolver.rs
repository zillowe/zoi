use crate::pubgrub::{PkgName, SemVersion, ZoiDependencyProvider};
use anyhow::{Result, anyhow};
use pubgrub::{DependencyProvider, Ranges, resolve as pubgrub_resolve};
use rustc_hash::FxHashMap;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use zoi_core::types::{self, InstallReason, Package};
use zoi_deps as dependencies;
use zoi_project::lockfile::FrozenLockPackage;
use zoi_resolver::resolve;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallNode {
    pub pkg: Package,
    pub version: String,
    pub revision: String,
    pub sub_package: Option<String>,
    pub repo_type: String,
    pub description: String,
    pub reason: InstallReason,
    pub source: String,
    pub registry_handle: String,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
    pub dependencies: Vec<String>,
    pub git_sha: Option<String>,
}

#[derive(Default, Debug)]
pub struct DependencyGraph {
    pub nodes: HashMap<String, InstallNode>,
    pub adj: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toposort(&self) -> Result<Vec<Vec<String>>> {
        let mut in_degree: HashMap<String, usize> =
            self.nodes.keys().map(|id| (id.clone(), 0)).collect();

        for (from, to_set) in &self.adj {
            if from == "$root" {
                continue;
            }
            for to in to_set {
                if let Some(degree) = in_degree.get_mut(to) {
                    *degree += 1;
                }
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
                let u = queue
                    .pop_front()
                    .ok_or_else(|| anyhow!("Queue length inconsistency in toposort"))?;
                stage.push(u.clone());
                count += 1;

                if let Some(neighbors) = self.adj.get(&u) {
                    for v_id in neighbors {
                        let degree = in_degree
                            .get_mut(v_id)
                            .ok_or_else(|| anyhow!("v_id '{}' missing from in_degree", v_id))?;
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

        stages.reverse();

        Ok(stages)
    }
}

fn extract_zoi_dependencies(deps: &types::DependenciesV2) -> Vec<String> {
    let mut zoi_deps = Vec::new();

    let mut to_process = deps.runtime.clone();
    for b in &deps.build {
        to_process.extend(b.packages.clone());
    }

    for dep_str in to_process {
        if let Ok(dep) = dependencies::parse_dependency_string(&dep_str)
            && dep.manager == "zoi"
        {
            zoi_deps.push(dep.package.to_string());
        }
    }
    zoi_deps
}

pub fn build_graph_from_locked_packages(
    locked_packages: &[FrozenLockPackage],
    scope_override: Option<types::Scope>,
    quiet: bool,
    yes: bool,
) -> Result<(DependencyGraph, Vec<String>)> {
    if !quiet {
        println!(":: Resolving dependencies from zoi.lock...");
    }

    let mut graph = DependencyGraph::new();
    let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut pkg_id_to_locked_deps: HashMap<String, Vec<String>> = HashMap::new();

    for locked in locked_packages {
        let request = resolve::parse_source_string(&locked.source)?;
        let (pkg, version_str, _, pkg_lua_path, handle, repo_type, git_sha) =
            resolve::resolve_package_and_version(&locked.source, quiet, yes)?;

        let mut pkg = pkg;
        if let Some(scope) = scope_override {
            pkg.scope = scope;
        }

        let pkg_id = if let Some(sub) = &request.sub_package {
            format!("{}@{}:{}", pkg.name, version_str, sub)
        } else {
            format!("{}@{}", pkg.name, version_str)
        };

        let flattened_deps = if let Some(deps) = &locked.dependencies {
            extract_zoi_dependencies(deps)
        } else {
            Vec::new()
        };

        pkg_id_to_locked_deps.insert(pkg_id.clone(), flattened_deps.clone());

        graph.nodes.insert(
            pkg_id.clone(),
            InstallNode {
                description: pkg.description.clone(),
                repo_type: repo_type.unwrap_or_else(|| "unofficial".to_string()),
                pkg,
                version: version_str,
                revision: locked.revision.clone(),
                sub_package: request.sub_package.clone(),
                reason: if locked.direct {
                    InstallReason::Direct
                } else {
                    InstallReason::Dependency {
                        parent: "unknown".to_string(),
                    }
                },
                source: pkg_lua_path.to_string_lossy().to_string(),
                registry_handle: handle.unwrap_or_else(|| "zoidberg".to_string()),
                chosen_options: locked.chosen_options.clone(),
                chosen_optionals: locked.chosen_optionals.clone(),
                dependencies: flattened_deps,
                git_sha: locked.git_sha.clone().or(git_sha),
            },
        );
    }

    for pkg_id in graph.nodes.keys() {
        let mut children = HashSet::new();
        if let Some(locked_deps) = pkg_id_to_locked_deps.get(pkg_id) {
            for dep_pkg_source in locked_deps {
                let dep_req = resolve::parse_source_string(dep_pkg_source)?;
                let dep_version = dep_req.version_spec.as_deref().unwrap_or_default();
                let dep_id = if let Some(sub) = dep_req.sub_package {
                    format!("{}@{}:{}", dep_req.name, dep_version, sub)
                } else {
                    format!("{}@{}", dep_req.name, dep_version)
                };

                if graph.nodes.contains_key(&dep_id) {
                    children.insert(dep_id.clone());
                    reverse_deps.entry(dep_id).or_default().push(pkg_id.clone());
                }
            }
        }
        graph.adj.insert(pkg_id.clone(), children);
    }

    let direct_ids: Vec<String> = graph
        .nodes
        .iter()
        .filter_map(|(pkg_id, node)| {
            let is_direct =
                matches!(node.reason, InstallReason::Direct) || !reverse_deps.contains_key(pkg_id);
            is_direct.then(|| pkg_id.clone())
        })
        .collect();

    graph
        .adj
        .insert("$root".to_string(), direct_ids.iter().cloned().collect());

    let direct_id_set: HashSet<String> = direct_ids.iter().cloned().collect();
    let parent_sources: HashMap<String, String> = reverse_deps
        .iter()
        .filter_map(|(pkg_id, parents)| {
            let parent_id = parents.first()?;
            let parent_node = graph.nodes.get(parent_id)?;
            Some((
                pkg_id.clone(),
                zoi_resolver::local::package_source_string(
                    &parent_node.registry_handle,
                    &parent_node.pkg.repo,
                    &parent_node.pkg.name,
                    parent_node.sub_package.as_deref(),
                    &parent_node.version,
                ),
            ))
        })
        .collect();

    for (pkg_id, node) in &mut graph.nodes {
        if direct_id_set.contains(pkg_id) {
            node.reason = InstallReason::Direct;
        } else if let Some(parent_source) = parent_sources.get(pkg_id) {
            node.reason = InstallReason::Dependency {
                parent: parent_source.clone(),
            };
        }
    }

    Ok((graph, Vec::new()))
}

pub fn resolve_dependency_graph(
    initial_sources: &[String],
    scope_override: Option<types::Scope>,
    _force: bool,
    yes: bool,
    all_optional: bool,
    _build_type: Option<&str>,
    quiet: bool,
) -> Result<(DependencyGraph, Vec<String>)> {
    if !quiet {
        println!(":: Resolving dependencies...");
    }

    let mut non_zoi_deps = Vec::new();
    let mut root_deps = FxHashMap::default();

    for source in initial_sources {
        let parse_result = dependencies::parse_dependency_string(source);
        if let Ok(dep) = parse_result
            && dep.manager != "zoi"
        {
            non_zoi_deps.push(source.clone());
            continue;
        }

        let request = resolve::parse_source_string(source)?;
        let resolved = resolve::resolve_source(source, quiet, yes)?;

        let pkg_name = PkgName {
            name: request.name,
            sub_package: request.sub_package,
            repo: resolved.repo_name.unwrap_or_default(),
            registry: resolved
                .registry_handle
                .unwrap_or_else(|| "zoidberg".to_string()),
            explicit_source: matches!(
                resolved.source_type,
                zoi_core::types::SourceType::LocalFile
                    | zoi_core::types::SourceType::Url
                    | zoi_core::types::SourceType::GitRepo(_)
            )
            .then(|| source.clone()),
        };

        let range = if request.version_spec.is_some() {
            let resolved_version = resolve::resolve_requested_version_spec(source, true, true)?
                .ok_or_else(|| anyhow!("version spec missing despite check for '{}'", source))?;
            crate::pubgrub::semver_to_range(&resolved_version)
        } else {
            Ranges::full()
        };

        root_deps.insert(pkg_name, range);
    }

    let project_config = zoi_project::config::load().ok();

    let provider = ZoiDependencyProvider::new(
        root_deps,
        initial_sources.to_vec(),
        quiet,
        yes,
        all_optional,
        project_config,
    )?;
    let root_pkg = PkgName {
        name: "$root".to_string(),
        sub_package: None,
        repo: "".to_string(),
        registry: "".to_string(),
        explicit_source: None,
    };
    let root_version = SemVersion(Version::new(0, 0, 0));

    let mut final_nodes = HashMap::new();
    let mut final_adj: HashMap<String, HashSet<String>> = HashMap::new();

    match pubgrub_resolve::<ZoiDependencyProvider>(&provider, root_pkg, root_version) {
        Ok(solution) => {
            for (name, version) in solution.iter() {
                if name.name == "$root" {
                    continue;
                }

                let source = name
                    .explicit_source
                    .clone()
                    .unwrap_or_else(|| format!("{}@{}", name, version));
                let (pkg, version_str, _, pkg_lua_path, handle, repo_type, git_sha) =
                    resolve::resolve_package_and_version(&source, quiet, yes)?;

                let mut pkg = pkg;
                if let Some(s) = scope_override {
                    pkg.scope = s;
                }

                let pkg_id = if let Some(sub) = &name.sub_package {
                    format!("{}@{}:{}", pkg.name, version_str, sub)
                } else {
                    format!("{}@{}", pkg.name, version_str)
                };

                let cache_key = (name.clone(), version.clone());
                let (chosen_options, chosen_optionals, all_req_deps) = provider
                    .chosen_cache
                    .borrow()
                    .get(&cache_key)
                    .cloned()
                    .unwrap_or_default();

                for dep_str in &all_req_deps {
                    if let Ok(dep_req) = zoi_deps::parse_dependency_string(dep_str)
                        && dep_req.manager != "zoi"
                    {
                        non_zoi_deps.push(dep_str.clone());
                    }
                }

                let node = InstallNode {
                    description: pkg.description.clone(),
                    repo_type: repo_type.unwrap_or_else(|| "unofficial".to_string()),
                    pkg: pkg.clone(),
                    version: version_str,
                    revision: pkg.revision.clone(),
                    sub_package: name.sub_package.clone(),
                    reason: InstallReason::Direct,
                    source: pkg_lua_path.to_string_lossy().to_string(),
                    registry_handle: handle.unwrap_or_else(|| "zoidberg".to_string()),
                    chosen_options,
                    chosen_optionals,
                    dependencies: all_req_deps,
                    git_sha,
                };
                final_nodes.insert(pkg_id, node);
            }

            for (name, version) in solution.iter() {
                let from_id = if name.name == "$root" {
                    "$root".to_string()
                } else if let Some(sub) = &name.sub_package {
                    format!("{}@{}:{}", name.name, version, sub)
                } else {
                    format!("{}@{}", name.name, version)
                };

                if let Ok(pubgrub::Dependencies::Available(deps)) =
                    provider.get_dependencies(name, version)
                {
                    for (dep_name, _) in deps {
                        if let Some(dep_version) = solution.get(&dep_name) {
                            let to_id = if let Some(sub) = &dep_name.sub_package {
                                format!("{}@{}:{}", dep_name.name, dep_version, sub)
                            } else {
                                format!("{}@{}", dep_name.name, dep_version)
                            };
                            final_adj.entry(from_id.clone()).or_default().insert(to_id);
                        }
                    }
                }
            }
            let mut direct_ids = HashSet::new();
            if let Some(root_children) = final_adj.get("$root") {
                direct_ids = root_children.clone();
            }

            let mut parent_map = HashMap::new();
            for (from, to_set) in &final_adj {
                if from != "$root"
                    && let Some(parent_node) = final_nodes.get(from)
                {
                    let parent_id = format!(
                        "#{}@{}/{}@{}",
                        parent_node.registry_handle,
                        parent_node.pkg.repo,
                        parent_node.pkg.name,
                        parent_node.version
                    );
                    for to in to_set {
                        parent_map.entry(to.clone()).or_insert(parent_id.clone());
                    }
                }
            }

            let resolved_child_sources: HashMap<String, Vec<String>> = final_adj
                .iter()
                .map(|(pkg_id, children)| {
                    let deps = children
                        .iter()
                        .filter_map(|child| {
                            final_nodes.get(child).map(|child_node| {
                                format!(
                                    "zoi:{}",
                                    zoi_resolver::local::package_source_string(
                                        &child_node.registry_handle,
                                        &child_node.pkg.repo,
                                        &child_node.pkg.name,
                                        child_node.sub_package.as_deref(),
                                        &child_node.version,
                                    )
                                )
                            })
                        })
                        .collect::<Vec<_>>();
                    (pkg_id.clone(), deps)
                })
                .collect();

            for (pkg_id, node) in final_nodes.iter_mut() {
                let child_sources = resolved_child_sources
                    .get(pkg_id)
                    .cloned()
                    .unwrap_or_default();

                if direct_ids.contains(pkg_id) {
                    node.reason = InstallReason::Direct;
                } else {
                    let parent_id = parent_map
                        .get(pkg_id)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    node.reason = InstallReason::Dependency { parent: parent_id };
                }

                let mut resolved_deps = child_sources;
                for dep_str in &node.dependencies {
                    if let Ok(dep_req) = zoi_deps::parse_dependency_string(dep_str)
                        && dep_req.manager != "zoi"
                    {
                        resolved_deps.push(dep_str.clone());
                    }
                }
                node.dependencies = resolved_deps;
            }
        }
        Err(e) => return Err(anyhow!("Dependency resolution failed: {}", e)),
    }

    Ok((
        DependencyGraph {
            nodes: final_nodes,
            adj: final_adj,
        },
        non_zoi_deps,
    ))
}
