use crate::pkg::{
    dependencies,
    install::pubgrub::{PkgName, SemVersion, ZoiDependencyProvider},
    resolve,
    types::{self, InstallReason, Package},
};
use anyhow::{Result, anyhow};
use pubgrub::{DependencyProvider, Ranges, resolve as pubgrub_resolve};
use rustc_hash::FxHashMap;
use semver::Version;
use std::collections::{HashMap, HashSet, VecDeque};

pub fn collect_dependencies_for_group(
    group: &types::DependencyGroup,
    sub_package_name: Option<&str>,
    dep_type: Option<&str>,
    yes: bool,
    all_optional: bool,
) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let mut deps = Vec::new();
    let mut chosen_options = Vec::new();
    let mut chosen_optionals = Vec::new();

    match group {
        types::DependencyGroup::Simple(d) => {
            deps.extend(d.clone());
        }
        types::DependencyGroup::Complex(g) => {
            deps.extend(g.required.clone());

            let options = dependencies::prompt_for_options(&g.options, yes)?;
            chosen_options.extend(options.clone());
            deps.extend(options);

            let optionals =
                dependencies::prompt_for_optionals(&g.optional, dep_type, yes, all_optional)?;
            chosen_optionals.extend(optionals.clone());
            deps.extend(optionals);

            if let Some(sub_name) = sub_package_name
                && let Some(sub_deps_map) = &g.sub_packages
                && let Some(sub_dep_group) = sub_deps_map.get(sub_name)
            {
                let (sub_d, sub_co, sub_coo) = collect_dependencies_for_group(
                    sub_dep_group,
                    None,
                    dep_type,
                    yes,
                    all_optional,
                )?;
                deps.extend(sub_d);
                chosen_options.extend(sub_co);
                chosen_optionals.extend(sub_coo);
            }
        }
    }
    Ok((deps, chosen_options, chosen_optionals))
}

#[derive(Debug, Clone)]
pub struct InstallNode {
    pub pkg: Package,
    pub version: String,
    pub sub_package: Option<String>,
    pub reason: InstallReason,
    pub source: String,
    pub registry_handle: String,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
    pub dependencies: Vec<String>,
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

        for to_set in self.adj.values() {
            for to in to_set {
                *in_degree.get_mut(to).unwrap() += 1;
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

        stages.reverse();

        Ok(stages)
    }
}

pub fn resolve_dependency_graph(
    initial_sources: &[String],
    _scope_override: Option<types::Scope>,
    _force: bool,
    _yes: bool,
    _all_optional: bool,
    _build_type: Option<&str>,
    _quiet: bool,
) -> Result<(DependencyGraph, Vec<String>)> {
    println!(":: Resolving dependencies using PubGrub SAT solver...");

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
        let resolved = resolve::resolve_source(source, true)?;

        let pkg_name = PkgName {
            name: request.name,
            sub_package: request.sub_package,
            repo: resolved.repo_name.unwrap_or_default(),
            registry: resolved
                .registry_handle
                .unwrap_or_else(|| "zoidberg".to_string()),
        };

        root_deps.insert(pkg_name, Ranges::full());
    }

    let provider = ZoiDependencyProvider::new(root_deps)?;
    let root_pkg = PkgName {
        name: "$root".to_string(),
        sub_package: None,
        repo: "".to_string(),
        registry: "".to_string(),
    };
    let root_version = SemVersion(Version::new(0, 0, 0));

    let mut final_nodes = HashMap::new();
    let mut final_adj: HashMap<String, HashSet<String>> = HashMap::new();

    match pubgrub_resolve::<ZoiDependencyProvider>(&provider, root_pkg, root_version) {
        Ok(solution) => {
            for (name, version) in &solution {
                if name.name == "$root" {
                    continue;
                }

                let source = format!("{}", name);
                let (pkg, version_str, _, pkg_lua_path, handle) =
                    resolve::resolve_package_and_version(&format!("{}@{}", source, version), true)?;

                let pkg_id = if let Some(sub) = &name.sub_package {
                    format!("{}@{}:{}", pkg.name, version_str, sub)
                } else {
                    format!("{}@{}", pkg.name, version_str)
                };

                let node = InstallNode {
                    pkg: pkg.clone(),
                    version: version_str,
                    sub_package: name.sub_package.clone(),
                    reason: InstallReason::Direct,
                    source: pkg_lua_path.to_string_lossy().to_string(),
                    registry_handle: handle.unwrap_or_else(|| "zoidberg".to_string()),
                    chosen_options: Vec::new(),
                    chosen_optionals: Vec::new(),
                    dependencies: Vec::new(),
                };
                final_nodes.insert(pkg_id, node);
            }

            for (name, version) in &solution {
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
                    for dep_name in deps.keys() {
                        if let Some(dep_version) = solution.get(dep_name) {
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
