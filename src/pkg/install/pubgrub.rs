use crate::pkg::{db, resolve, types};
use pubgrub::{Dependencies, DependencyProvider, Ranges};
use rusqlite::params;
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

pub fn semver_to_range(req_str: &str) -> Ranges<SemVersion> {
    let req_str = req_str.trim_start_matches('@');

    if let Ok(version) = Version::parse(req_str) {
        return Ranges::singleton(SemVersion(version));
    }

    if let Ok(req) = semver::VersionReq::parse(req_str) {
        let mut range = Ranges::full();
        for comparator in &req.comparators {
            let v = SemVersion(Version {
                major: comparator.major,
                minor: comparator.minor.unwrap_or(0),
                patch: comparator.patch.unwrap_or(0),
                pre: comparator.pre.clone(),
                build: semver::BuildMetadata::EMPTY,
            });

            let comp_range = match comparator.op {
                semver::Op::Exact => Ranges::singleton(v),
                semver::Op::Greater => Ranges::strictly_higher_than(v),
                semver::Op::GreaterEq => Ranges::higher_than(v),
                semver::Op::Less => Ranges::strictly_lower_than(v),
                semver::Op::LessEq => Ranges::lower_than(v),
                semver::Op::Tilde => {
                    let next_minor = SemVersion(Version {
                        major: comparator.major,
                        minor: comparator.minor.unwrap_or(0) + 1,
                        patch: 0,
                        pre: semver::Prerelease::EMPTY,
                        build: semver::BuildMetadata::EMPTY,
                    });
                    Ranges::higher_than(v).intersection(&Ranges::strictly_lower_than(next_minor))
                }
                semver::Op::Caret => {
                    let next_major = if comparator.major > 0 {
                        SemVersion(Version {
                            major: comparator.major + 1,
                            minor: 0,
                            patch: 0,
                            pre: semver::Prerelease::EMPTY,
                            build: semver::BuildMetadata::EMPTY,
                        })
                    } else if let Some(minor) = comparator.minor {
                        SemVersion(Version {
                            major: 0,
                            minor: minor + 1,
                            patch: 0,
                            pre: semver::Prerelease::EMPTY,
                            build: semver::BuildMetadata::EMPTY,
                        })
                    } else {
                        SemVersion(Version {
                            major: 0,
                            minor: 1,
                            patch: 0,
                            pre: semver::Prerelease::EMPTY,
                            build: semver::BuildMetadata::EMPTY,
                        })
                    };
                    Ranges::higher_than(v).intersection(&Ranges::strictly_lower_than(next_major))
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
        quiet: bool,
        yes: bool,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            root_deps,
            quiet,
            yes,
        })
    }

    fn semver_to_range(&self, req_str: &str) -> Ranges<SemVersion> {
        semver_to_range(req_str)
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
                    && let Ok(v) = Version::parse(&v_str)
                {
                    all_versions.push(SemVersion(v));
                }
            }
        }

        all_versions.sort();
        all_versions.dedup();
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

        let version_str = version.to_string();

        let dependencies_opt = db::get_package_dependencies(
            &package.registry,
            &package.name,
            &version_str,
            package.sub_package.as_deref(),
            &package.repo,
        )
        .ok()
        .flatten();

        let package_deps = if let Some(deps_json) = dependencies_opt
            && !deps_json.is_empty()
        {
            serde_json::from_str::<types::Dependencies>(&deps_json).ok()
        } else {
            let source = format!("{}", package);
            let pkg_res = resolve::resolve_package_and_version(
                &format!("{}@{}", source, version_str),
                self.quiet,
                self.yes,
            );

            match pkg_res {
                Ok((pkg, _, _, _, _)) => pkg.dependencies,
                Err(_) => None,
            }
        };

        let mut deps = FxHashMap::default();

        if let Some(dependencies) = package_deps
            && let Some(runtime) = &dependencies.runtime
        {
            let (req_deps, _, _) = crate::pkg::install::resolver::collect_dependencies_for_group(
                runtime,
                package.sub_package.as_deref(),
                Some("runtime"),
                self.yes,
                true,
            )
            .map_err(|e| ZoiSolverError::Dependency(e.to_string()))?;

            for dep_str in req_deps {
                let dep_req = crate::pkg::dependencies::parse_dependency_string(&dep_str)
                    .map_err(|e| ZoiSolverError::Dependency(e.to_string()))?;

                if dep_req.manager == "zoi" {
                    let req = resolve::parse_source_string(dep_req.package)
                        .map_err(|e| ZoiSolverError::Dependency(e.to_string()))?;
                    let resolved_dep =
                        resolve::resolve_source(dep_req.package, self.quiet, self.yes)
                            .map_err(|e| ZoiSolverError::Dependency(e.to_string()))?;

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
