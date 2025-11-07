use crate::pkg::{
    config, dependencies, local, resolve,
    types::{self, InstallReason, Package},
};
use anyhow::{Result, anyhow};
use colored::*;
use semver::{Version, VersionReq};
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
    pub reason: InstallReason,
    pub source: String,
    pub registry_handle: String,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
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

fn check_policy(pkg: &Package, config: &types::Config) -> Result<()> {
    let policy = &config.policy;

    if let Some(denied) = &policy.denied_packages
        && denied.contains(&pkg.name)
    {
        return Err(anyhow!(
            "Installation of package '{}' is denied by system policy.",
            pkg.name
        ));
    }
    if let Some(allowed) = &policy.allowed_packages
        && !allowed.contains(&pkg.name)
    {
        return Err(anyhow!(
            "Installation of package '{}' is not allowed by system policy.",
            pkg.name
        ));
    }

    if let Some(denied) = &policy.denied_repos
        && denied.contains(&pkg.repo)
    {
        return Err(anyhow!(
            "Packages from repository '{}' are denied by system policy.",
            pkg.repo
        ));
    }
    if let Some(allowed) = &policy.allowed_repos
        && !allowed.contains(&pkg.repo)
    {
        return Err(anyhow!(
            "Packages from repository '{}' are not allowed by system policy.",
            pkg.repo
        ));
    }

    if !pkg.license.is_empty() {
        if let Ok(expr) = spdx::Expression::parse(&pkg.license) {
            if let Some(denied) = &policy.denied_licenses {
                for req in expr.requirements() {
                    if let spdx::LicenseItem::Spdx { id, .. } = req.req.license
                        && denied.contains(&id.name.to_string())
                    {
                        return Err(anyhow!(
                            "Package license '{}' is denied by system policy.",
                            pkg.license
                        ));
                    }
                }
            }
            if let Some(allowed) = &policy.allowed_licenses {
                for req in expr.requirements() {
                    if let spdx::LicenseItem::Spdx { id, .. } = req.req.license {
                        if !allowed.contains(&id.name.to_string()) {
                            return Err(anyhow!(
                                "Package license '{}' contains '{}', which is not in the list of allowed licenses.",
                                pkg.license,
                                id.name
                            ));
                        }
                    } else {
                        return Err(anyhow!(
                            "Package license '{}' contains a non-SPDX license, which is not allowed by system policy.",
                            pkg.license
                        ));
                    }
                }
            }
        } else if policy.allowed_licenses.is_some() || policy.denied_licenses.is_some() {
            return Err(anyhow!(
                "Could not parse package license '{}' to check against system policy.",
                pkg.license
            ));
        }
    }

    Ok(())
}

pub fn resolve_dependency_graph(
    initial_sources: &[String],
    scope_override: Option<types::Scope>,
    force: bool,
    yes: bool,
    all_optional: bool,
    build_type: Option<&str>,
    quiet: bool,
) -> Result<(DependencyGraph, Vec<String>)> {
    let config = config::read_config()?;
    let mut graph = DependencyGraph::new();
    let mut non_zoi_deps = Vec::new();
    let mut queue: VecDeque<(String, Option<String>)> =
        initial_sources.iter().map(|s| (s.clone(), None)).collect();
    let mut processed_sources = HashSet::new();

    while let Some((source, parent_id)) = queue.pop_front() {
        if processed_sources.contains(&source) {
            continue;
        }

        let parse_result = dependencies::parse_dependency_string(&source);

        if let Ok(dep) = parse_result
            && dep.manager != "zoi"
        {
            if !non_zoi_deps.contains(&source) {
                non_zoi_deps.push(source.clone());
            }
            processed_sources.insert(source.clone());
            continue;
        }

        let request = resolve::parse_source_string(&source)?;

        if request.sub_package.is_none()
            && let Ok(resolved) = resolve::resolve_source(&source, quiet)
        {
            let pkg_template = crate::pkg::lua::parser::parse_lua_package(
                resolved.path.to_str().unwrap(),
                None,
                quiet,
            )?;

            if pkg_template.sub_packages.is_some() {
                let subs_to_install = pkg_template.main_subs.clone().unwrap_or_default();

                if !subs_to_install.is_empty() {
                    println!(
                        "'{}' is a split package, queueing main sub-packages for installation: {}",
                        source,
                        subs_to_install.join(", ")
                    );

                    let mut base_source = String::new();
                    if let Some(h) = &request.handle {
                        base_source.push('#');
                        base_source.push_str(h);
                    }
                    if let Some(r) = &request.repo {
                        base_source.push('@');
                        base_source.push_str(r);
                        base_source.push('/');
                    }
                    base_source.push_str(&request.name);

                    for sub in subs_to_install {
                        let sub_source = format!("{}:{}", base_source, sub);
                        let final_source = if let Some(v) = &request.version_spec {
                            format!("{}@{}", sub_source, v)
                        } else {
                            sub_source
                        };
                        queue.push_back((final_source, parent_id.clone()));
                    }
                    processed_sources.insert(source.clone());
                    continue;
                }
            }
        }

        let (mut pkg, version, _, pkg_lua_path, registry_handle) =
            match resolve::resolve_package_and_version(&source, quiet) {
                Ok(res) => res,
                Err(e) => return Err(anyhow!("Failed to resolve '{}': {}", source, e)),
            };

        check_policy(&pkg, &config)?;

        let handle = registry_handle.as_deref().unwrap_or("local");
        if let Some(scope) = scope_override {
            pkg.scope = scope;
        }

        let pkg_id = if let Some(sub) = &request.sub_package {
            format!("{}@{}:{}", pkg.name, version, sub)
        } else {
            format!("{}@{}", pkg.name, version)
        };

        if let Some(parent_id) = &parent_id {
            graph
                .adj
                .entry(parent_id.clone())
                .or_default()
                .insert(pkg_id.clone());
        }

        if graph.nodes.contains_key(&pkg_id) {
            continue;
        }

        processed_sources.insert(source.clone());

        if !force {
            let installed_packages = local::get_installed_packages()?;
            let mut satisfied = false;

            if let Some(installed) = installed_packages.iter().find(|m| {
                m.name == pkg.name && m.sub_package.as_deref() == request.sub_package.as_deref()
            }) && let (Ok(installed_v), Ok(req_v)) = (
                Version::parse(&installed.version),
                VersionReq::parse(&version),
            ) && req_v.matches(&installed_v)
            {
                println!(
                    "Already installed: {} ({}) satisfies {}. Skipping.",
                    pkg.name.cyan(),
                    installed.version.yellow(),
                    version.yellow()
                );
                satisfied = true;
            }

            if !satisfied
                && let Some(provider) = installed_packages.iter().find(|m| {
                    m.provides
                        .as_ref()
                        .is_some_and(|p| p.contains(&request.name))
                })
                && let (Ok(installed_v), Ok(req_v)) = (
                    Version::parse(&provider.version),
                    VersionReq::parse(&version),
                )
                && req_v.matches(&installed_v)
            {
                println!(
                    "'{}' is provided by installed package '{}'. Skipping.",
                    request.name, provider.name
                );
                satisfied = true;
            }

            if satisfied {
                continue;
            }
        }

        let mut chosen_options = Vec::new();
        let mut chosen_optionals = Vec::new();
        let mut deps_to_process = Vec::new();

        if let Some(deps) = &pkg.dependencies {
            if let Some(runtime) = &deps.runtime {
                let (d, co, coo) = collect_dependencies_for_group(
                    runtime,
                    request.sub_package.as_deref(),
                    Some("runtime"),
                    yes,
                    all_optional,
                )?;
                deps_to_process.extend(d);
                chosen_options.extend(co);
                chosen_optionals.extend(coo);
            }

            if let Some(build) = &deps.build {
                if build_type.is_some() {
                    println!("Resolving build dependencies for {}", pkg.name.cyan());
                }

                let build_dep_groups = match build {
                    types::BuildDependencies::Group(group) => vec![group.clone()],
                    types::BuildDependencies::Typed(typed_build_deps) => {
                        if let Some(build_type_str) = build_type {
                            if let Some(group) = typed_build_deps.types.get(build_type_str) {
                                vec![group.clone()]
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    }
                };

                for group in build_dep_groups {
                    let (d, co, coo) = collect_dependencies_for_group(
                        &group,
                        request.sub_package.as_deref(),
                        Some("build"),
                        yes,
                        all_optional,
                    )?;
                    deps_to_process.extend(d);
                    chosen_options.extend(co);
                    chosen_optionals.extend(coo);
                }
            }
        }

        for dep_source in deps_to_process {
            queue.push_back((dep_source, Some(pkg_id.clone())));
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

    Ok((graph, non_zoi_deps))
}
