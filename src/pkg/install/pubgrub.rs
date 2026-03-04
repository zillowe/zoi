use crate::pkg::{db, resolve};
use pubgrub::{Dependencies, DependencyProvider, Ranges};
use rustc_hash::FxHashMap;
use semver::Version;
use std::fmt::Display;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PkgName {
    pub name: String,
    pub sub_package: Option<String>,
    pub repo: String,
    pub registry: String,
}

impl Display for PkgName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sub) = &self.sub_package {
            write!(f, "#{}@{}/{}:{}", self.registry, self.repo, self.name, sub)
        } else {
            write!(f, "#{}@{}/{}", self.registry, self.repo, self.name)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SemVersion(pub Version);

impl Display for SemVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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

pub struct ZoiDependencyProvider {
    pub root_deps: FxHashMap<PkgName, Ranges<SemVersion>>,
    pub quiet: bool,
    pub yes: bool,
}

impl ZoiDependencyProvider {
    pub fn new(
        root_deps: FxHashMap<PkgName, Ranges<SemVersion>>,
        quiet: bool,
        yes: bool,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            root_deps,
            quiet,
            yes,
        })
    }

    fn semver_to_range(&self, _req_str: &str) -> Ranges<SemVersion> {
        Ranges::full()
    }

    pub fn get_versions(&self, package: &PkgName) -> Result<Vec<SemVersion>, ZoiSolverError> {
        let mut all_versions = Vec::new();
        if let Ok(version_strings) =
            db::get_all_versions(&package.registry, &package.name, &package.repo)
        {
            for v_str in version_strings {
                if let Ok(v) = Version::parse(&v_str) {
                    all_versions.push(SemVersion(v));
                }
            }
        }

        all_versions.sort();
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
            return Ok(Dependencies::Available(self.root_deps.clone()));
        }

        let source = format!("{}", package);
        let version_str = version.to_string();

        let (pkg, _, _, _, _) = resolve::resolve_package_and_version(
            &format!("{}@{}", source, version_str),
            self.quiet,
            self.yes,
        )?;

        let mut deps = FxHashMap::default();

        if let Some(dependencies) = &pkg.dependencies
            && let Some(runtime) = &dependencies.runtime
        {
            let (req_deps, _, _) = crate::pkg::install::resolver::collect_dependencies_for_group(
                runtime,
                package.sub_package.as_deref(),
                Some("runtime"),
                self.yes,
                true,
            )?;

            for dep_str in req_deps {
                let dep_req = crate::pkg::dependencies::parse_dependency_string(&dep_str)?;

                if dep_req.manager == "zoi" {
                    let req = resolve::parse_source_string(dep_req.package)?;
                    let resolved_dep =
                        resolve::resolve_source(dep_req.package, self.quiet, self.yes)?;

                    let dep_name = PkgName {
                        name: req.name,
                        sub_package: req.sub_package,
                        repo: resolved_dep.repo_name.unwrap_or_default(),
                        registry: resolved_dep
                            .registry_handle
                            .unwrap_or_else(|| "zoidberg".to_string()),
                    };

                    let range = if let Some(v_spec) = &req.version_spec {
                        self.semver_to_range(v_spec)
                    } else {
                        Ranges::full()
                    };

                    deps.insert(dep_name, range);
                }
            }
        }

        Ok(Dependencies::Available(deps))
    }

    fn choose_version(
        &self,
        package: &Self::P,
        versions: &pubgrub::Ranges<Self::V>,
    ) -> Result<Option<Self::V>, Self::Err> {
        if package.name == "$root" {
            return Ok(Some(SemVersion(Version::new(0, 0, 0))));
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
