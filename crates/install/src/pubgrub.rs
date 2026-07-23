use colored::Colorize;
use pubgrub::{Dependencies, DependencyProvider, Ranges};
use rusqlite::params;
use rustc_hash::FxHashMap;
use semver::Version;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use thiserror::Error;
use zoi_core::types;
use zoi_db as db;
use zoi_resolver::resolve;

fn parse_pkgs_v2_key(key: &str) -> (String, String) {
    let key = key.trim_start_matches('#');
    let key = key.trim_start_matches('@');
    if let Some((repo, name)) = key.split_once('/') {
        let name = name.split(':').next().unwrap_or(name);
        (repo.to_string(), name.to_string())
    } else {
        (String::new(), key.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PkgName {
    pub name: String,
    pub sub_package: Option<String>,
    pub repo: String,
    pub registry: String,
    pub explicit_source: Option<String>,
}

impl Display for PkgName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(source) = &self.explicit_source {
            return write!(f, "{}", source);
        }
        if let Some(sub) = &self.sub_package {
            write!(f, "#{}@{}/{}:{}", self.registry, self.repo, self.name, sub)
        } else {
            write!(f, "#{}@{}/{}", self.registry, self.repo, self.name)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SemVersion {
    pub v: Version,
    pub original: String,
}

impl SemVersion {
    pub fn new(v: Version, original: String) -> Self {
        Self { v, original }
    }

    pub fn parse(v: &str) -> Result<Self, anyhow::Error> {
        let clean = sanitize_version_string(v);
        match Version::parse(&clean) {
            Ok(parsed) => Ok(SemVersion {
                v: parsed,
                original: v.to_string(),
            }),
            Err(_) => {
                // Fallback for extremely weird versions: use 0.0.0+original
                Ok(SemVersion {
                    v: Version::new(0, 0, 0),
                    original: v.to_string(),
                })
            }
        }
    }
}

impl Ord for SemVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.v.cmp(&other.v)
    }
}

impl PartialOrd for SemVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for SemVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.original)
    }
}

/// Sanitizes non-standard version strings into valid SemVer format.
///
/// Logic:
/// - Padds missing parts (e.g. "2.41" -> "2.41.0").
/// - Converts extra parts into build metadata (e.g. "7.1.4.arch1" -> "7.1.4+arch1").
/// - Cleans up prefixes and suffixes.
fn sanitize_version_string(v: &str) -> String {
    let v = v.trim_start_matches('v').replace('-', "+");
    let parts: Vec<&str> = v.split('.').collect();

    if parts.len() == 1 {
        format!("{}.0.0", parts[0])
    } else if parts.len() == 2 {
        format!("{}.{}.0", parts[0], parts[1])
    } else if parts.len() > 3 {
        format!(
            "{}.{}.{}+{}",
            parts[0],
            parts[1],
            parts[2],
            parts[3..].join(".")
        )
    } else {
        v.to_string()
    }
}

#[derive(Error, Debug)]
pub enum ZoiSolverError {
    #[error("Dependency error: {0}")]
    Dependency(String),
    #[error("Version error: {0}")]
    Version(String),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("Other error: {0}")]
    Other(String),
}

/// Adapts Zoi's package and registry model to the PubGrub SAT solver.
///
/// This is the most performance-critical part of the resolution logic.
/// It implements the `DependencyProvider` trait, which allows the solver
/// to "ask" Zoi:
/// - "What versions are available for this package?"
/// - "What are the dependencies for this specific version?"
///
/// The adapter handles querying the local SQLite index, remote registries,
/// and project-local `zoi.lua` overrides in a unified way.
pub struct ZoiDependencyProvider {
    /// The initial set of direct dependencies requested by the user.
    pub root_deps: FxHashMap<PkgName, Ranges<SemVersion>>,
    /// The raw string sources (e.g. "#reg@repo/pkg@ver") provided to the CLI.
    pub initial_sources: Vec<String>,
    /// The target installation scope (User, System, Project).
    pub scope: Option<types::Scope>,
    /// Whether to suppress non-critical warnings during resolution.
    pub quiet: bool,
    /// Automatically accept default choices for interactive dependency options.
    pub yes: bool,
    /// Automatically include all optional dependencies.
    pub all_optional: bool,
    /// An in-memory cache of the registry used in 'mini' mode to avoid disk I/O.
    pub mini_index: Option<zoi_resolver::mini_resolve::MiniRegistryIndex>,
    /// The loaded `zoi.lua` or `zoi.yaml` project configuration, if applicable.
    pub project_config: Option<zoi_project::config::ProjectConfig>,
    /// Hard version constraints enforced by a project's lockfile or configuration.
    pub pkgs_v2_constraints: HashMap<(String, String), String>,
    /// Memoization cache mapping a package+version to its resolved dependency requirements.
    /// The `RefCell` allows interior mutability since the PubGrub solver requires `&self`.
    pub deps_cache:
        RefCell<FxHashMap<(PkgName, SemVersion), FxHashMap<PkgName, Ranges<SemVersion>>>>,
    /// Memoization cache storing the explicit options and optional dependencies chosen by the user.
    pub chosen_cache:
        RefCell<FxHashMap<(PkgName, SemVersion), (Vec<String>, Vec<String>, Vec<String>)>>,
}

/// Converts a SemVer requirement string into a PubGrub version range.
///
/// This function bridges the gap between the `semver` crate's flexible requirement
/// strings and `pubgrub`'s mathematical version ranges.
///
/// Mapping Examples:
/// - `1.2.3` -> `Ranges::singleton(1.2.3)` (Exact match)
/// - `^1.2.3` -> `[1.2.3, 2.0.0)` (Caret: compatible updates)
/// - `~1.2.3` -> `[1.2.3, 1.3.0)` (Tilde: patch-level updates)
/// - `>=1.0.0, <2.0.0` -> `[1.0.0, 2.0.0)` (Intersection of ranges)
///
/// If a version string is not a valid SemVer requirement (e.g. a channel name like
/// `@stable`), it is treated as a `Ranges::full()` to let Zoi's higher-level
/// resolver handle the channel-to-version mapping.
pub fn semver_to_range(req_str: &str) -> Ranges<SemVersion> {
    let req_str_clean = req_str.trim_start_matches('@').trim_start_matches('v');

    if let Ok(version) = SemVersion::parse(req_str_clean) {
        return Ranges::singleton(version);
    }

    if let Ok(req) = semver::VersionReq::parse(req_str_clean) {
        let mut range = Ranges::full();
        for comparator in &req.comparators {
            let v_str = format!(
                "{}.{}.{}",
                comparator.major,
                comparator.minor.unwrap_or(0),
                comparator.patch.unwrap_or(0)
            );
            let v = SemVersion {
                v: Version {
                    major: comparator.major,
                    minor: comparator.minor.unwrap_or(0),
                    patch: comparator.patch.unwrap_or(0),
                    pre: comparator.pre.clone(),
                    build: semver::BuildMetadata::EMPTY,
                },
                original: v_str,
            };

            let comp_range = match comparator.op {
                semver::Op::Exact => Ranges::singleton(v),
                semver::Op::Greater => Ranges::strictly_higher_than(v),
                semver::Op::GreaterEq => Ranges::higher_than(v),
                semver::Op::Less => Ranges::strictly_lower_than(v),
                semver::Op::LessEq => Ranges::lower_than(v),
                semver::Op::Tilde => {
                    let next_minor_v = Version {
                        major: comparator.major,
                        minor: comparator.minor.unwrap_or(0) + 1,
                        patch: 0,
                        pre: semver::Prerelease::EMPTY,
                        build: semver::BuildMetadata::EMPTY,
                    };
                    let next_minor = SemVersion {
                        v: next_minor_v.clone(),
                        original: next_minor_v.to_string(),
                    };
                    Ranges::higher_than(v).intersection(&Ranges::strictly_lower_than(next_minor))
                }
                semver::Op::Caret => {
                    let next_v = if comparator.major > 0 {
                        Version {
                            major: comparator.major + 1,
                            minor: 0,
                            patch: 0,
                            pre: semver::Prerelease::EMPTY,
                            build: semver::BuildMetadata::EMPTY,
                        }
                    } else if let Some(minor) = comparator.minor {
                        if minor > 0 {
                            Version {
                                major: 0,
                                minor: minor + 1,
                                patch: 0,
                                pre: semver::Prerelease::EMPTY,
                                build: semver::BuildMetadata::EMPTY,
                            }
                        } else {
                            Version {
                                major: 0,
                                minor: 0,
                                patch: comparator.patch.unwrap_or(0) + 1,
                                pre: semver::Prerelease::EMPTY,
                                build: semver::BuildMetadata::EMPTY,
                            }
                        }
                    } else {
                        Version {
                            major: 1,
                            minor: 0,
                            patch: 0,
                            pre: semver::Prerelease::EMPTY,
                            build: semver::BuildMetadata::EMPTY,
                        }
                    };
                    let next = SemVersion {
                        v: next_v.clone(),
                        original: next_v.to_string(),
                    };
                    Ranges::higher_than(v).intersection(&Ranges::strictly_lower_than(next))
                }
                _ => Ranges::full(),
            };
            range = range.intersection(&comp_range);
        }
        return range;
    }

    Ranges::full()
}

impl ZoiDependencyProvider {
    pub fn new(
        root_deps: FxHashMap<PkgName, Ranges<SemVersion>>,
        initial_sources: Vec<String>,
        scope: Option<types::Scope>,
        quiet: bool,
        yes: bool,
        all_optional: bool,
        project_config: Option<zoi_project::config::ProjectConfig>,
    ) -> Result<Self, anyhow::Error> {
        let mini_index = if zoi_core::utils::is_mini_mode() {
            Some(zoi_resolver::mini_resolve::fetch_registry_index()?)
        } else {
            None
        };

        let pkgs_v2_constraints = project_config
            .as_ref()
            .map(|config| {
                config
                    .pkgs_v2
                    .iter()
                    .filter_map(|(key, spec)| {
                        spec.version
                            .as_ref()
                            .map(|v| (parse_pkgs_v2_key(key), v.clone()))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        Ok(Self {
            root_deps,
            initial_sources,
            scope,
            quiet,
            yes,
            all_optional,
            mini_index,
            project_config,
            pkgs_v2_constraints,
            deps_cache: RefCell::new(FxHashMap::default()),
            chosen_cache: RefCell::new(FxHashMap::default()),
        })
    }

    fn semver_to_range(&self, req_str: &str) -> Ranges<SemVersion> {
        semver_to_range(req_str)
    }

    fn source_matches_package(&self, package: &PkgName, source: &str) -> bool {
        if let Some(explicit_source) = &package.explicit_source {
            let explicit_base = explicit_source
                .rsplit_once('@')
                .map(|(base, _)| base)
                .unwrap_or(explicit_source.as_str());
            let source_base = source
                .rsplit_once('@')
                .map(|(base, _)| base)
                .unwrap_or(source);
            return explicit_base == source_base;
        }

        let Ok(req) = resolve::parse_source_string(source) else {
            return false;
        };

        if req.name != package.name || req.sub_package != package.sub_package {
            return false;
        }

        let Ok(resolved_source) = resolve::resolve_source(source, self.scope, true, true) else {
            return false;
        };

        resolved_source.repo_name.unwrap_or_default() == package.repo
            && resolved_source
                .registry_handle
                .unwrap_or_else(|| "zoidberg".to_string())
                == package.registry
    }

    pub fn get_versions(&self, package: &PkgName) -> Result<Vec<SemVersion>, ZoiSolverError> {
        let mut all_versions = Vec::new();

        if let Some(index) = &self.mini_index
            && let Some(pkg_info) = index.packages.get(&package.name)
            && let Ok(v) = SemVersion::parse(pkg_info.version.trim_start_matches('v'))
        {
            all_versions.push(v);
        }

        if let Ok(version_strings) =
            db::get_all_versions(&package.registry, &package.name, &package.repo)
        {
            for v_str in version_strings {
                if let Ok(v) = SemVersion::parse(&v_str) {
                    all_versions.push(v);
                }
            }
        }

        for source in &self.initial_sources {
            if self.source_matches_package(package, source)
                && let Ok(req) = resolve::parse_source_string(source)
                && let Some(v_spec) = req.version_spec
                && let Ok(v) = SemVersion::parse(&v_spec)
            {
                all_versions.push(v);
            }
        }

        let source_str = package.explicit_source.clone().unwrap_or_else(|| {
            if let Some(sub) = &package.sub_package {
                format!(
                    "#{}@{}/{}:{}",
                    package.registry, package.repo, package.name, sub
                )
            } else {
                format!("#{}@{}/{}", package.registry, package.repo, package.name)
            }
        });

        if let Ok(resolved) = resolve::resolve_source(&source_str, self.scope, true, true) {
            let path_str = resolved.path.to_string_lossy();
            if let Ok(pkg) = zoi_lua::parser::parse_lua_package(&path_str, None, self.scope, true) {
                if let Some(v_str) = &pkg.version
                    && let Ok(v) = SemVersion::parse(v_str)
                {
                    all_versions.push(v);
                }
                if let Some(versions_map) = &pkg.versions {
                    for channel in versions_map.keys() {
                        if let Ok(v_str) = resolve::resolve_channel(versions_map, channel)
                            && let Ok(v) = SemVersion::parse(&v_str)
                        {
                            all_versions.push(v);
                        }
                    }
                }
            }
        }

        if all_versions.is_empty()
            && let Ok(conn) = db::open_connection(&package.registry)
        {
            let mut stmt = conn
                .prepare("SELECT version FROM packages WHERE name = ?1")
                .map_err(|e| ZoiSolverError::Other(e.to_string()))?;
            let rows = stmt
                .query_map(params![package.name], |row| row.get::<_, Option<String>>(0))
                .map_err(|e| ZoiSolverError::Other(e.to_string()))?;

            for v_res in rows {
                if let Ok(Some(v_str)) = v_res
                    && let Ok(v) = SemVersion::parse(&v_str)
                {
                    all_versions.push(v);
                }
            }
        }

        all_versions.sort();
        all_versions.dedup();

        if let Some(version_spec) = self
            .pkgs_v2_constraints
            .get(&(package.repo.clone(), package.name.clone()))
        {
            let range = semver_to_range(version_spec);
            all_versions.retain(|v| range.contains(v));
        }

        Ok(all_versions)
    }
}

impl DependencyProvider for ZoiDependencyProvider {
    type P = PkgName;
    type V = SemVersion;
    type VS = Ranges<SemVersion>;
    type M = String;
    type Priority = i32;
    type Err = ZoiSolverError;

    fn get_dependencies(
        &self,
        package: &Self::P,
        version: &Self::V,
    ) -> Result<Dependencies<Self::P, Self::VS, Self::M>, Self::Err> {
        if package.name == "$root" {
            return Ok(Dependencies::Available(
                pubgrub::DependencyConstraints::from_iter(self.root_deps.clone()),
            ));
        }

        let cache_key = (package.clone(), version.clone());
        if let Some(cached) = self.deps_cache.borrow().get(&cache_key) {
            return Ok(Dependencies::Available(
                pubgrub::DependencyConstraints::from_iter(cached.clone()),
            ));
        }

        let version_str = version.to_string();

        let mut package_deps: Option<types::DependenciesV2> = None;

        if let Some(config) = &self.project_config {
            let packages_key = if let Some(sub) = &package.sub_package {
                format!("@{}/{}:{}", package.repo, package.name, sub)
            } else {
                format!("@{}/{}", package.repo, package.name)
            };

            if let Some(spec) = config.pkgs_v2.get(&packages_key)
                && spec.version.as_ref().is_none_or(|v| {
                    let range = semver_to_range(v);
                    if let Ok(pv) = SemVersion::parse(&version_str) {
                        range.contains(&pv) || v == &version_str
                    } else {
                        v == &version_str
                    }
                })
            {
                package_deps = spec.dependencies.clone().map(types::to_dependencies_v2);
            }
        }

        if package_deps.is_none() {
            let dependencies_opt = db::get_package_dependencies(
                &package.registry,
                &package.name,
                &version_str,
                package.sub_package.as_deref(),
                &package.repo,
            )
            .ok()
            .flatten();

            let v1_deps = if let Some(deps_json) = dependencies_opt
                && !deps_json.is_empty()
            {
                serde_json::from_str::<types::Dependencies>(&deps_json).ok()
            } else {
                let source = package.explicit_source.clone().unwrap_or_else(|| {
                    if let Some(sub) = &package.sub_package {
                        format!(
                            "#{}@{}/{}:{}@{}",
                            package.registry, package.repo, package.name, sub, version_str
                        )
                    } else {
                        format!(
                            "#{}@{}/{}@{}",
                            package.registry, package.repo, package.name, version_str
                        )
                    }
                });

                let pkg_res =
                    resolve::resolve_package_and_version(&source, self.scope, self.quiet, self.yes);

                match pkg_res {
                    Ok((pkg, _, _, _, _, _, _)) => pkg.dependencies,
                    Err(e) => {
                        println!(
                            "{} Failed to resolve source for deps: {}",
                            "::".bold().red(),
                            e
                        );
                        None
                    }
                }
            };
            package_deps = v1_deps.map(types::to_dependencies_v2);
        }

        let mut deps = FxHashMap::default();

        let chosen_opts = Vec::new();
        let chosen_opts_opt = Vec::new();
        let mut all_req = Vec::new();

        if let Some(dependencies) = package_deps {
            let mut groups = Vec::new();
            groups.push(&dependencies.runtime);
            for b in &dependencies.build {
                groups.push(&b.packages);
            }

            for group_pkgs in groups {
                for dep_str in group_pkgs {
                    let dep_req = zoi_deps::parse_dependency_string(dep_str).map_err(|e| {
                        ZoiSolverError::Dependency(format!("parse fail for '{}': {}", dep_str, e))
                    })?;

                    if dep_req.manager == "zoi" {
                        let req = match resolve::parse_source_string(dep_req.package) {
                            Ok(r) => r,
                            Err(e) => {
                                println!(
                                    "{} Dependency parse failed for '{}': {}",
                                    "::".bold().red(),
                                    dep_req.package,
                                    e
                                );
                                return Err(ZoiSolverError::Dependency(format!(
                                    "parse source fail for '{}': {}",
                                    dep_req.package, e
                                )));
                            }
                        };

                        let resolved_dep = match resolve::resolve_source(
                            dep_req.package,
                            self.scope,
                            false,
                            self.yes,
                        ) {
                            Ok(r) => r,
                            Err(e) => {
                                println!(
                                    "{} Dependency resolution failed for '{}': {}",
                                    "::".bold().red(),
                                    dep_req.package,
                                    e
                                );
                                return Err(ZoiSolverError::Dependency(format!(
                                    "resolve fail for '{}': {}",
                                    dep_req.package, e
                                )));
                            }
                        };

                        let dep_name = PkgName {
                            name: req.name,
                            sub_package: req.sub_package,
                            repo: resolved_dep.repo_name.clone().unwrap_or_default(),
                            registry: resolved_dep
                                .registry_handle
                                .clone()
                                .unwrap_or_else(|| "zoidberg".to_string()),
                            explicit_source: matches!(
                                resolved_dep.source_type,
                                zoi_core::types::SourceType::LocalFile
                                    | zoi_core::types::SourceType::Url
                                    | zoi_core::types::SourceType::GitRepo(_)
                            )
                            .then(|| dep_req.package.to_string()),
                        };

                        let range = if req.version_spec.is_some() {
                            match resolve::resolve_requested_version_spec(
                                dep_req.package,
                                self.scope,
                                false,
                                true,
                            ) {
                                Ok(Some(v)) => self.semver_to_range(&v),
                                Ok(None) => Ranges::full(),
                                Err(e) => {
                                    println!(
                                        "{} Version resolution failed for '{}': {}",
                                        "::".bold().red(),
                                        dep_req.package,
                                        e
                                    );
                                    return Err(ZoiSolverError::Dependency(format!(
                                        "version resolve fail for '{}': {}",
                                        dep_req.package, e
                                    )));
                                }
                            }
                        } else {
                            Ranges::full()
                        };

                        deps.insert(dep_name, range);
                    }
                }
            }
            all_req = dependencies.runtime.clone();
        }

        self.deps_cache
            .borrow_mut()
            .insert(cache_key.clone(), deps.clone());
        self.chosen_cache
            .borrow_mut()
            .insert(cache_key, (chosen_opts, chosen_opts_opt, all_req));
        Ok(Dependencies::Available(
            pubgrub::DependencyConstraints::from_iter(deps),
        ))
    }

    fn choose_version(
        &self,
        package: &Self::P,
        versions: &pubgrub::Ranges<Self::V>,
    ) -> Result<Option<Self::V>, Self::Err> {
        if package.name == "$root" {
            return Ok(Some(SemVersion {
                v: Version::new(0, 0, 0),
                original: "0.0.0".to_string(),
            }));
        }
        let all_versions = self.get_versions(package)?;
        let best_version = all_versions.into_iter().rfind(|v| versions.contains(v));
        Ok(best_version)
    }

    fn prioritize(
        &self,
        _package: &Self::P,
        _range: &Self::VS,
        _stats: &pubgrub::PackageResolutionStatistics,
    ) -> Self::Priority {
        0
    }
}
