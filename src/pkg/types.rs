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
    Service,
    Config,
    App,
    Extension,
    Library,
    Script,
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
pub enum DockerType {
    Compose { file: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum ServiceMethod {
    Command {
        platforms: Vec<String>,
        start: Vec<String>,
        stop: Vec<String>,
    },
    Docker {
        platforms: Vec<String>,
        docker: Vec<DockerType>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ConfigCommands {
    pub platforms: Vec<String>,
    pub install: Vec<String>,
    pub uninstall: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AppCommands {
    pub platforms: Vec<String>,
    #[serde(rename = "appCreate")]
    pub app_create: String,
    #[serde(default)]
    pub commands: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct PostInstallHook {
    pub platforms: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionChange {
    RepoGit { add: String },
    RegistryRepo { add: String },
    RepoAdd { add: String },
    Project { add: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtensionInfo {
    #[serde(rename = "type")]
    pub extension_type: String,
    pub changes: Vec<ExtensionChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PkgConfig {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub libs: String,
    #[serde(default)]
    pub cflags: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub repo: String,
    pub version: Option<String>,
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
    pub service: Option<Vec<ServiceMethod>>,
    pub config: Option<Vec<ConfigCommands>>,
    pub app: Option<Vec<AppCommands>>,
    pub script: Option<Vec<ConfigCommands>>,
    #[serde(default)]
    pub post_install: Option<Vec<PostInstallHook>>,
    #[serde(default)]
    pub post_uninstall: Option<Vec<PostInstallHook>>,
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
    pub pkg_config: Option<PkgConfig>,
    #[serde(default)]
    pub updates: Option<Vec<UpdateInfo>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Maintainer {
    pub name: String,
    pub email: String,
    pub website: Option<String>,
    #[serde(default)]
    pub one_time: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
    pub website: Option<String>,
    #[serde(default)]
    pub one_time: bool,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallManifest {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub registry_handle: String,
    pub package_type: PackageType,
    pub installed_at: String,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Registry {
    pub handle: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
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
pub struct Lockfile {
    pub version: String,
    pub packages: HashMap<String, LockfilePackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockfilePackage {
    pub name: String,
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
