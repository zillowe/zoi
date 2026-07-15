use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Low => write!(f, "Low"),
            Severity::Medium => write!(f, "Medium"),
            Severity::High => write!(f, "High"),
            Severity::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    pub summary: String,
    pub severity: Severity,
    pub cvss: Option<String>,
    pub affected_range: String,
    pub fixed_in: Option<String>,
    pub description: String,
    pub references: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniVulnerability {
    pub id: String,
    pub severity: String,
    pub affected_range: String,
    pub fixed_in: Option<String>,
    pub summary: String,
}

fn default_version() -> String {
    "1".to_string()
}

fn default_revision() -> String {
    "1".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AdvisoryRegistry {
    pub version: String,
    pub last_id: u32,
    pub year: u32,
    pub advisories: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    User,
    System,
    Project,
}

/// Defines the category of a Zoi package.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    /// A standard software package containing binaries or libraries.
    #[default]
    Package,
    /// A meta-package that groups other packages together via dependencies.
    Collection,
    /// A project template used by `zoi create`.
    App,
    /// A configuration package that modifies Zoi's own settings.
    Extension,
}

/// The severity or category of an update notice.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum UpdateType {
    /// A general change notice.
    Change,
    /// A critical security vulnerability notice.
    Vulnerability,
    /// A standard software update notice.
    Update,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateInfo {
    #[serde(rename = "type")]
    pub update_type: UpdateType,
    pub message: String,
}

/// Defines a specific configuration change applied by an Extension.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionChange {
    /// Clones a third-party Git repository into Zoi's sources.
    RepoGit { add: String },
    /// Replaces the primary registry URL.
    RegistryRepo { add: String },
    /// Adds a supplementary registry.
    RegistryAdd { add: String },
    /// Activates an official repository tier (e.g. "test").
    RepoAdd { add: String },
    /// Creates a `zoi.yaml` project file in the current directory.
    Project { add: String },
    /// Imports a PGP public key for verification.
    Pgp { name: String, key: String },
    /// Registers a new global Lua plugin.
    Plugin { name: String, script: String },
    /// Registers a new global transaction hook.
    Hook { name: String, content: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtensionInfo {
    #[serde(rename = "type")]
    pub extension_type: String,
    pub changes: Vec<ExtensionChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Service {
    pub run: String,
    #[serde(default)]
    pub run_at_load: bool,
    pub working_dir: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub log_path: Option<String>,
    pub error_log_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShellCompletion {
    pub shell: String,
    pub filename: String,
}

impl InstallManifest {
    pub fn into_package(self) -> Package {
        Package {
            name: self.name,
            repo: self.repo,
            version: Some(self.version),
            sub_package: self.sub_package,
            package_type: self.package_type,
            registry_handle: Some(self.registry_handle),
            scope: self.scope,
            bins: self.bins,
            conflicts: self.conflicts,
            replaces: self.replaces,
            provides: self.provides,
            backup: self.backup,
            service: self.service,
            installed_size: self.installed_size,
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum ManSpec {
    Single(String),
    Multiple(Vec<String>),
    Map(BTreeMap<String, String>),
}

/// The core package definition blueprint.
///
/// This struct is the Rust representation of the `metadata({...})` block in a `.pkg.lua` file.
/// It defines what a package is, where it comes from, and how it can be installed,
/// but does not represent an actual installation on disk (see `InstallManifest` for that).
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Package {
    /// Unique name of the package.
    pub name: String,
    /// The repository tier this package belongs to (e.g. "core", "main").
    pub repo: String,
    /// The resolved version string.
    pub version: Option<String>,
    /// Incremental revision for the same upstream version (e.g. for packaging fixes).
    #[serde(default = "default_revision")]
    pub revision: String,
    /// List of available sub-packages for split-package definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_packages: Option<Vec<String>>,
    /// Default sub-packages to install if none are specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_subs: Option<Vec<String>>,
    /// Map of version channels (e.g. "stable", "nightly") to versions.
    pub versions: Option<HashMap<String, String>>,
    /// Short summary of the package.
    pub description: String,
    /// Project homepage URL.
    pub website: Option<String>,
    /// URL or path to the package README.
    #[serde(default)]
    pub readme: Option<String>,
    /// Manual page specification.
    #[serde(default)]
    pub man: Option<ManSpec>,
    /// Upstream Git repository URL.
    #[serde(default)]
    pub git: String,
    /// The maintainer responsible for the Zoi package definition.
    pub maintainer: Maintainer,
    /// The original software author.
    pub author: Option<Author>,
    /// SPDX license identifier.
    #[serde(default)]
    pub license: String,
    /// Supported build types (e.g. "source", "pre-compiled").
    #[serde(default)]
    pub types: Vec<String>,
    /// Supported OS/Arch platforms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<String>>,
    /// CI runner configuration for registry pipelines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiConfig>,
    /// Build and runtime dependencies.
    pub dependencies: Option<Dependencies>,
    /// The package category (Package, App, Extension, etc.).
    #[serde(rename = "type", default)]
    pub package_type: PackageType,
    /// Pointer to an alternative package definition.
    pub alt: Option<String>,
    /// The primary/default installation scope for this package.
    #[serde(default)]
    pub scope: Scope,
    /// List of allowed installation scopes. If provided, Zoi will enforce that
    /// the package is only installed into one of these targets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<Scope>>,
    /// Handle of the registry this package was resolved from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<InstallReason>,
    #[serde(default)]
    pub bins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub extension: Option<ExtensionInfo>,
    #[serde(default)]
    pub rollback: Option<bool>,
    #[serde(default)]
    pub updates: Option<Vec<UpdateInfo>>,
    #[serde(default)]
    pub hooks: Option<Hooks>,
    #[serde(default)]
    pub service: Option<Service>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Vec<ShellCompletion>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    StringVec(Vec<String>),
    Platform(HashMap<String, Vec<String>>),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Hooks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_install: Option<PlatformOrStringVec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_install: Option<PlatformOrStringVec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_upgrade: Option<PlatformOrStringVec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_upgrade: Option<PlatformOrStringVec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_remove: Option<PlatformOrStringVec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_remove: Option<PlatformOrStringVec>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Maintainer {
    pub name: String,
    pub email: String,
    pub website: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DependencyOptionGroup {
    pub name: String,
    pub desc: String,
    #[serde(default)]
    pub all: bool,
    pub depends: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyGroup {
    Simple(Vec<String>),
    Complex(ComplexDependencyGroup),
}

impl DependencyGroup {
    pub fn required(&self) -> &[String] {
        match self {
            DependencyGroup::Simple(deps) => deps.as_slice(),
            DependencyGroup::Complex(group) => group.required.as_slice(),
        }
    }

    pub fn options(&self) -> &[DependencyOptionGroup] {
        match self {
            DependencyGroup::Simple(_) => &[],
            DependencyGroup::Complex(group) => group.options.as_slice(),
        }
    }

    pub fn optional(&self) -> &[String] {
        match self {
            DependencyGroup::Simple(_) => &[],
            DependencyGroup::Complex(group) => group.optional.as_slice(),
        }
    }

    pub fn get_required_simple(&self) -> Vec<String> {
        match self {
            DependencyGroup::Simple(deps) => deps.clone(),
            DependencyGroup::Complex(group) => group.required.clone(),
        }
    }

    pub fn get_required_options(&self) -> Vec<DependencyOptionGroup> {
        match self {
            DependencyGroup::Simple(_) => Vec::new(),
            DependencyGroup::Complex(group) => group.options.clone(),
        }
    }

    pub fn get_optional(&self) -> &Vec<String> {
        match self {
            DependencyGroup::Simple(_) => {
                static EMPTY_VEC: Vec<String> = Vec::new();
                &EMPTY_VEC
            }
            DependencyGroup::Complex(group) => &group.optional,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct ComplexDependencyGroup {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub options: Vec<DependencyOptionGroup>,
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_packages: Option<HashMap<String, DependencyGroup>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct TypedBuildDependencies {
    pub types: HashMap<String, DependencyGroup>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum BuildDependencies {
    Typed(TypedBuildDependencies),
    Group(DependencyGroup),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Dependencies {
    #[serde(default)]
    pub runtime: Option<DependencyGroup>,
    #[serde(default)]
    pub build: Option<BuildDependencies>,
}

pub fn to_dependencies_v2(deps: Dependencies) -> DependenciesV2 {
    let mut runtime = Vec::new();
    if let Some(r) = deps.runtime {
        runtime = match r {
            DependencyGroup::Simple(d) => d,
            DependencyGroup::Complex(c) => {
                let mut all = c.required;
                all.extend(c.optional);
                for opt in c.options {
                    all.extend(opt.depends);
                }
                all
            }
        };
    }

    let mut build = Vec::new();
    if let Some(b) = deps.build {
        match b {
            BuildDependencies::Group(g) => {
                let packages = match g {
                    DependencyGroup::Simple(d) => d,
                    DependencyGroup::Complex(c) => {
                        let mut all = c.required;
                        all.extend(c.optional);
                        for opt in c.options {
                            all.extend(opt.depends);
                        }
                        all
                    }
                };
                build.push(BuildDependencyV2 {
                    build_type: "source".to_string(),
                    packages,
                });
            }
            BuildDependencies::Typed(t) => {
                for (bt, g) in t.types {
                    let packages = match g {
                        DependencyGroup::Simple(d) => d,
                        DependencyGroup::Complex(c) => {
                            let mut all = c.required;
                            all.extend(c.optional);
                            for opt in c.options {
                                all.extend(opt.depends);
                            }
                            all
                        }
                    };
                    build.push(BuildDependencyV2 {
                        build_type: bt,
                        packages,
                    });
                }
            }
        }
    }

    DependenciesV2 { runtime, build }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstallReason {
    Direct,
    Dependency { parent: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct SandboxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub cwd: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CiConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, String>>,
}

/// The record of an actual package installation on disk.
///
/// Unlike the `Package` blueprint, the `InstallManifest` is the "Source of Truth"
/// for what is currently installed. It is stored in `manifest.yaml` inside the
/// package's version directory in the Zoi store.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstallManifest {
    /// Name of the installed package.
    pub name: String,
    /// Exact version installed.
    pub version: String,
    /// Revision of the package definition used for this install.
    #[serde(default = "default_revision")]
    pub revision: String,
    /// For split packages, the name of the installed sub-package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    /// Repository tier the package came from.
    pub repo: String,
    /// Trust level of the repository (official, community, etc.).
    #[serde(default)]
    pub repo_type: String,
    /// Registry handle the package was installed from.
    pub registry_handle: String,
    /// Category of the installed package.
    pub package_type: PackageType,
    /// Short summary.
    #[serde(default)]
    pub description: String,
    /// Whether the user installed this directly or if it's a dependency.
    pub reason: InstallReason,
    /// The scope where this package was installed.
    pub scope: Scope,
    /// List of linked binary names.
    pub bins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Vec<String>>,
    #[serde(default)]
    pub installed_dependencies: Vec<String>,
    #[serde(default)]
    pub dependencies_v2: Option<DependenciesV2>,
    #[serde(default)]
    pub chosen_options: Vec<String>,
    #[serde(default)]
    pub chosen_optionals: Vec<String>,
    #[serde(default)]
    pub install_method: Option<String>,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub service: Option<Service>,
    #[serde(default)]
    pub installed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Vec<ShellCompletion>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TransactionOperation {
    Install {
        manifest: Box<InstallManifest>,
    },
    Uninstall {
        manifest: Box<InstallManifest>,
    },
    Upgrade {
        old_manifest: Box<InstallManifest>,
        new_manifest: Box<InstallManifest>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub start_time: String,
    pub operations: Vec<TransactionOperation>,
}

fn skip_authorities(a: &Option<Vec<String>>) -> bool {
    a.as_ref().is_none_or(|v| v.is_empty())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct Registry {
    #[serde(default)]
    pub handle: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "skip_authorities")]
    pub authorities: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct RemotePolicyConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub signature_url: String,
    #[serde(default)]
    pub trusted_keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub package_managers: Option<Vec<String>>,
    #[serde(default)]
    pub native_package_manager: Option<String>,
    #[serde(default)]
    pub telemetry_enabled: bool,
    #[serde(default)]
    pub audit_log_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(default)]
    pub default_registry: Option<Registry>,
    #[serde(default)]
    pub added_registries: Vec<Registry>,
    #[serde(default)]
    pub git_repos: Vec<String>,
    #[serde(default = "default_rollback_enabled")]
    pub rollback_enabled: bool,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default)]
    pub remote_policy: Option<RemotePolicyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<usize>,
    #[serde(default)]
    pub protect_db: bool,
    #[serde(default)]
    pub max_resolution_depth: Option<u8>,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub pkg_dirs: Vec<String>,
    #[serde(default)]
    pub cache_mirrors: Vec<String>,
    #[serde(default)]
    pub versions: HashMap<String, String>,
}

fn default_rollback_enabled() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            package_managers: None,
            native_package_manager: None,
            telemetry_enabled: false,
            audit_log_enabled: false,
            registry: None,
            default_registry: None,
            added_registries: Vec::new(),
            git_repos: Vec::new(),
            rollback_enabled: true,
            policy: Policy::default(),
            remote_policy: None,
            jobs: None,
            protect_db: false,
            max_resolution_depth: None,
            offline_mode: false,
            pkg_dirs: Vec::new(),
            cache_mirrors: Vec::new(),
            versions: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Policy {
    #[serde(default, skip_serializing_if = "is_false")]
    pub repos_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub telemetry_enabled_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub audit_log_enabled_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rollback_enabled_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default_registry_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub added_registries_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub git_repos_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_deny_lists_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_enforcement_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub protect_db_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub max_resolution_depth_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub offline_mode_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pkg_dirs_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_mirrors_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub jobs_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub advisory_enforcement_unoverridable: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_licenses: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_licenses: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_packages: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_packages: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_repos: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_repos: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_enforcement: Option<SignatureEnforcementPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SignatureEnforcementPolicy {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub trusted_keys: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SharableInstallManifest {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub registry_handle: String,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_optionals: Vec<String>,
}

/// The root structure of a `packages.json` registry index file (Specification v2).
///
/// This file is generated by `zoi registry generate-metadata` and acts as a highly
/// optimized, centralized index of every `.pkg.lua` file in a registry.
/// It allows Zoi clients to perform SAT resolution without downloading or
/// evaluating thousands of Lua scripts locally.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryIndexV2 {
    /// The registry index specification version (always "2").
    pub version: String,
    /// A map of package identifiers (`@repo/name`) to their pre-computed metadata.
    pub packages: BTreeMap<String, PurlPackageIndexV2>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurlPackageIndexV2 {
    pub repo: String,
    pub repo_type: String,
    pub version: String,
    pub revision: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<Scope>>,
    pub sub_packages: Vec<String>,
    pub main_sub_packages: Vec<String>,
    pub vuln: Vec<MiniVulnerability>,
    pub dependencies: Option<DependenciesV2>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct DependenciesV2 {
    #[serde(default)]
    pub runtime: Vec<String>,
    #[serde(default)]
    pub build: Vec<BuildDependencyV2>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BuildDependencyV2 {
    #[serde(rename = "type")]
    pub build_type: String,
    pub packages: Vec<String>,
}

/// The root structure of a `zoi.lock` file (Specification v2).
///
/// This lockfile guarantees absolute reproducibility for project environments.
/// Instead of just pinning versions, it pins:
/// - Local State Hashes: Cryptographic hashes of the actual store and database directories.
/// - Registry Revisions: The exact Git commit SHAs of the registries used to resolve packages.
/// - Package Hashes: Expected checksums for every installed package directory.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZoiLockV2 {
    /// The lockfile specification version (always "2").
    pub version: String,
    /// The SHA-512 hash of the `.zoi/pkgs/store` directory, capturing the exact file state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages_hash: Option<String>,
    /// The SHA-512 hash of the `.zoi/pkgs/db` directory, capturing the registry metadata state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registries_hash: Option<String>,
    /// A map of registry handles to their pinned Git URLs and commit revisions.
    pub registries: BTreeMap<String, LockRegistryV2>,
    /// A map of package identifiers to their fully resolved installation state.
    pub installed_packages: BTreeMap<String, LockPackageDetailV2>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockRegistryV2 {
    pub revision: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockPackageDetailV2 {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    pub repo: String,
    pub repo_type: String,
    pub version: String,
    pub revision: String,
    pub registry: String,
    pub why: String,
    pub description: String,
    #[serde(rename = "type")]
    pub package_type_install: String,
    pub install_method: String,
    pub installed_sub_packages: Vec<String>,
    pub platform: String,
    pub hash: String,
    pub dependencies: Option<DependenciesV2>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitLink {
    #[serde(rename = "type")]
    pub link_type: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PkgLink {
    #[serde(rename = "type")]
    pub link_type: String,
    pub url: String,
    pub pgp: Option<String>,
    pub hash: Option<String>,
    pub size: Option<String>,
    pub files: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PgpKey {
    pub name: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub repo_type: String,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    pub git: Vec<GitLink>,
    #[serde(default)]
    pub pkg: Vec<PkgLink>,
    #[serde(default)]
    pub db: Option<String>,
    #[serde(default)]
    pub pgp: Vec<PgpKey>,
    pub repos: Vec<RepoEntry>,
}

#[derive(Debug, Clone)]
pub struct PrebuiltInfo {
    pub final_url: String,
    pub pgp_url: Option<String>,
    pub hash_url: Option<String>,
    pub size_url: Option<String>,
    pub files_url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SourceType {
    OfficialRepo,
    UntrustedRepo(String),
    GitRepo(String),
    LocalFile,
    Url,
}
