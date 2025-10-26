use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Scope {
    #[default]
    User,
    System,
    Project,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtensionInfo {
    #[serde(rename = "type")]
    pub extension_type: String,
    pub changes: Vec<ExtensionChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    #[serde(default)]
    pub bins: Option<Vec<String>>,
    #[serde(default)]
    pub conflicts: Option<Vec<String>>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Maintainer {
    pub name: String,
    pub email: String,
    pub website: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Dependencies {
    #[serde(default)]
    pub runtime: Option<DependencyGroup>,
    #[serde(default)]
    pub build: Option<DependencyGroup>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstallReason {
    Direct,
    Dependency { parent: String },
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
    pub conflicts: Option<Vec<String>>,
    #[serde(default)]
    pub installed_dependencies: Vec<String>,
    #[serde(default)]
    pub chosen_options: Vec<String>,
    #[serde(default)]
    pub chosen_optionals: Vec<String>,
    #[serde(default)]
    pub install_method: Option<String>,
    #[serde(default)]
    pub installed_files: Vec<String>,
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
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub repos: Vec<String>,
    pub package_managers: Option<Vec<String>>,
    pub native_package_manager: Option<String>,
    #[serde(default)]
    pub telemetry_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(default)]
    pub default_registry: Option<Registry>,
    #[serde(default)]
    pub added_registries: Vec<Registry>,
    #[serde(default)]
    pub git_repos: Vec<String>,
    #[serde(default)]
    pub rollback_enabled: bool,
    #[serde(default)]
    pub policy: Policy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_jobs: Option<usize>,
    #[serde(default)]
    pub protect_db: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Policy {
    #[serde(default, skip_serializing_if = "is_false")]
    pub repos_unoverridable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub telemetry_enabled_unoverridable: bool,
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
    pub git: Vec<GitLink>,
    #[serde(default)]
    pub pkg: Vec<PkgLink>,
    #[serde(default)]
    pub pgp: Vec<PgpKey>,
    pub repos: Vec<RepoEntry>,
}
