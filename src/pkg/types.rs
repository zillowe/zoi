use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub summary: String,
    pub severity: Severity,
    pub cvss: Option<String>,
    pub affected_range: String,
    pub fixed_in: Option<String>,
    pub description: String,
    pub references: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AdvisoryRegistry {
    pub last_id: u32,
    pub year: u32,
    pub advisories: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    User,
    System,
    Project,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    #[default]
    Package,
    Collection,
    App,
    Extension,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum UpdateType {
    Change,
    Vulnerability,
    Update,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateInfo {
    #[serde(rename = "type")]
    pub update_type: UpdateType,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionChange {
    RepoGit { add: String },
    RegistryRepo { add: String },
    RegistryAdd { add: String },
    RepoAdd { add: String },
    Project { add: String },
    Pgp { name: String, key: String },
    Plugin { name: String, script: String },
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub repo: String,
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_packages: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_subs: Option<Vec<String>>,
    pub versions: Option<HashMap<String, String>>,
    pub description: String,
    pub website: Option<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub man: Option<String>,
    #[serde(default)]
    pub git: String,
    pub maintainer: Maintainer,
    pub author: Option<Author>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub types: Vec<String>,
    pub dependencies: Option<Dependencies>,
    #[serde(rename = "type", default)]
    pub package_type: PackageType,
    pub alt: Option<String>,
    #[serde(default)]
    pub scope: Scope,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct DependencyOptionGroup {
    pub name: String,
    pub desc: String,
    #[serde(default)]
    pub all: bool,
    pub depends: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum DependencyGroup {
    Simple(Vec<String>),
    Complex(ComplexDependencyGroup),
}

impl DependencyGroup {
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TypedBuildDependencies {
    pub types: HashMap<String, DependencyGroup>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum BuildDependencies {
    Typed(TypedBuildDependencies),
    Group(DependencyGroup),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Dependencies {
    #[serde(default)]
    pub runtime: Option<DependencyGroup>,
    #[serde(default)]
    pub build: Option<BuildDependencies>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstallReason {
    Direct,
    Dependency { parent: String },
    Declarative,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstallManifest {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    pub repo: String,
    pub registry_handle: String,
    pub package_type: PackageType,
    pub reason: InstallReason,
    pub scope: Scope,
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
    pub chosen_options: Vec<String>,
    #[serde(default)]
    pub chosen_optionals: Vec<String>,
    #[serde(default)]
    pub install_method: Option<String>,
    #[serde(default)]
    pub service: Option<Service>,
    #[serde(default)]
    pub installed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BootConfig {
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub kernel_params: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HardwareConfig {
    #[serde(default)]
    pub drivers: Vec<String>,
    #[serde(default)]
    pub microcode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NetworkConfig {
    #[serde(default)]
    pub manager: Option<String>,
    #[serde(default)]
    pub firewall: Option<FirewallConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FirewallConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub allowed_tcp_ports: Vec<u16>,
    #[serde(default)]
    pub allowed_udp_ports: Vec<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeclarativeConfig {
    #[serde(default)]
    pub imports: Vec<String>,
    pub hostname: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub desktop: Option<String>,
    pub shell: Option<String>,
    #[serde(default)]
    pub boot: Option<BootConfig>,
    #[serde(default)]
    pub hardware: Option<HardwareConfig>,
    #[serde(default)]
    pub network: Option<NetworkConfig>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub programs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub files: HashMap<String, FileConfig>,
    #[serde(default)]
    pub users: HashMap<String, UserConfig>,
    #[serde(default)]
    pub groups: HashMap<String, GroupConfig>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Registry {
    pub handle: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorities: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub repos: Vec<String>,
    pub package_managers: Option<Vec<String>>,
    pub native_package_manager: Option<String>,
    #[serde(default)]
    pub telemetry_enabled: bool,
    #[serde(default)]
    pub audit_log_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_jobs: Option<usize>,
    #[serde(default)]
    pub protect_db: bool,
    #[serde(default)]
    pub max_resolution_depth: Option<u8>,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub pkg_dirs: Vec<String>,
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
            parallel_jobs: None,
            protect_db: false,
            max_resolution_depth: None,
            offline_mode: false,
            pkg_dirs: Vec::new(),
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_optionals: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZoiLockOld {
    pub packages: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZoiLock {
    pub version: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub registries: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub packages: HashMap<String, String>,
    #[serde(flatten)]
    pub details: HashMap<String, HashMap<String, LockPackageDetail>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockPackageDetail {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    pub integrity: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options_dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optionals_dependencies: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lockfile {
    pub version: String,
    pub packages: HashMap<String, LockfilePackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockfilePackage {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    pub repo: String,
    pub registry: String,
    pub version: String,
    pub date: String,
    pub reason: InstallReason,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_optionals: Vec<String>,
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
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    pub git: Vec<GitLink>,
    #[serde(default)]
    pub pkg: Vec<PkgLink>,
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
