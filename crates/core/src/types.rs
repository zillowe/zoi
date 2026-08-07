use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;

/// Represents the severity level of a security advisory.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Low severity.
    #[default]
    Low,
    /// Medium severity.
    Medium,
    /// High severity.
    High,
    /// Critical severity.
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

/// A security advisory containing information about a vulnerability.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Advisory {
    /// The unique identifier for the advisory.
    pub id: String,
    /// The package name affected by the advisory.
    pub package: String,
    /// The sub-package name affected, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    /// A short summary of the vulnerability.
    pub summary: String,
    /// The severity level of the vulnerability.
    pub severity: Severity,
    /// The CVSS score or vector, if available.
    pub cvss: Option<String>,
    /// The range of versions affected.
    pub affected_range: String,
    /// The version where the vulnerability was fixed.
    pub fixed_in: Option<String>,
    /// A detailed description of the vulnerability.
    pub description: String,
    /// References to external resources about the vulnerability.
    pub references: Option<Vec<String>>,
}

/// A simplified vulnerability representation used in package indices.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniVulnerability {
    /// The unique identifier for the advisory.
    pub id: String,
    /// The severity level as a string.
    pub severity: String,
    /// The range of versions affected.
    pub affected_range: String,
    /// The version where the vulnerability was fixed.
    pub fixed_in: Option<String>,
    /// A short summary of the vulnerability.
    pub summary: String,
}

/// Default version string "1".
fn default_version() -> String {
    "1".to_string()
}

/// Default revision string "1".
fn default_revision() -> String {
    "1".to_string()
}

/// A registry of security advisories.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AdvisoryRegistry {
    /// The version of the registry format.
    pub version: String,
    /// The last used numerical ID for generating new IDs.
    pub last_id: u32,
    /// The current year for ID generation.
    pub year: u32,
    /// A map of advisory IDs to their corresponding file paths.
    pub advisories: BTreeMap<String, String>,
}

/// The installation scope for a package.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// User-specific installation.
    #[default]
    User,
    /// System-wide installation.
    System,
    /// Project-local installation.
    Project,
}

/// A manifest for a pooled ZPA package.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PooledZpaManifest {
    /// The version of the manifest format.
    pub version: String,
    /// The pool of files included in the package.
    pub pool: BTreeMap<String, PoolFileEntry>,
    /// Mappings from sub-packages to their file entries.
    pub mappings: BTreeMap<String, SubPackageMapping>, // sub_package -> mapping
}

/// An entry in the file pool of a ZPA package.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PoolFileEntry {
    /// The size of the file in bytes.
    pub size: u64,
}

/// Mappings for a sub-package across different scopes.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubPackageMapping {
    /// Mappings for each scope.
    pub scopes: BTreeMap<Scope, ScopeMapping>, // scope -> mapping
}

/// Mappings of files, symlinks, and directories within a specific scope.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ScopeMapping {
    /// Files to be mapped in this scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<MappedFile>,
    /// Symlinks to be created in this scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symlinks: Vec<MappedSymlink>,
    /// Directories to be created in this scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs: Vec<MappedDir>,
}

/// A file to be mapped from the pool to a destination path.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedFile {
    /// The destination path relative to the scope root.
    pub dest: String,
    /// The hash of the file content in the pool.
    pub hash: String,
    /// The file mode (permissions).
    pub mode: u32,
    /// The owner of the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The group of the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A symlink to be created.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedSymlink {
    /// The path where the symlink will be created.
    pub link: String,
    /// The target path the symlink will point to.
    pub target: String,
}

/// A directory to be created.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MappedDir {
    /// The path of the directory.
    pub path: String,
    /// The directory mode (permissions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    /// The owner of the directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The group of the directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
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

/// Information about an update notice.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateInfo {
    /// The type of update.
    #[serde(rename = "type")]
    pub update_type: UpdateType,
    /// The update message.
    pub message: String,
}

/// Defines a specific configuration change applied by an Extension.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionChange {
    /// Clones a third-party Git repository into Zoi's sources.
    RepoGit {
        /// The URL of the repository to add.
        add: String,
    },
    /// Replaces the primary registry URL.
    RegistryRepo {
        /// The URL of the registry repository.
        add: String,
    },
    /// Adds a supplementary registry.
    RegistryAdd {
        /// The URL of the registry to add.
        add: String,
    },
    /// Activates an official repository tier (e.g. "test").
    RepoAdd {
        /// The name of the repository to add.
        add: String,
    },
    /// Creates a `zoi.yaml` project file in the current directory.
    Project {
        /// The content or configuration for the project.
        add: String,
    },
    /// Imports a PGP public key for verification.
    Pgp {
        /// The name of the PGP key.
        name: String,
        /// The PGP public key content.
        key: String,
    },
    /// Registers a new global Lua plugin.
    Plugin {
        /// The name of the plugin.
        name: String,
        /// The Lua script content.
        script: String,
    },
    /// Registers a new global transaction hook.
    Hook {
        /// The name of the hook.
        name: String,
        /// The hook script content.
        content: String,
    },
}

/// Information about an extension and its applied changes.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtensionInfo {
    /// The type of extension.
    #[serde(rename = "type")]
    pub extension_type: String,
    /// The list of changes applied by this extension.
    pub changes: Vec<ExtensionChange>,
}

/// Configuration for a background service managed by Zoi.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Service {
    /// The command to run the service.
    pub run: String,
    /// Whether the service should be started automatically when Zoi loads.
    #[serde(default)]
    pub run_at_load: bool,
    /// The working directory for the service.
    pub working_dir: Option<String>,
    /// Environment variables for the service.
    pub env: Option<HashMap<String, String>>,
    /// Path to the service's standard output log.
    pub log_path: Option<String>,
    /// Path to the service's error log.
    pub error_log_path: Option<String>,
}

/// Shell completion configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShellCompletion {
    /// The shell for which completions are provided (e.g. "bash", "zsh").
    pub shell: String,
    /// The filename of the completion script.
    pub filename: String,
}

impl InstallManifest {
    /// Converts the `InstallManifest` back into a `Package` blueprint.
    pub fn into_package(self) -> Package {
        Package {
            name: self.name,
            repo: self.repo,
            version: Some(self.version),
            epoch: self.epoch,
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
    /// Forced version precedence (default 0).
    #[serde(default)]
    pub epoch: u32,
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
    /// For split packages, the name of the specific sub-package to install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    /// The reason for installing this package (Direct or Dependency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<InstallReason>,
    /// List of executable binaries provided by the package.
    #[serde(default)]
    pub bins: Option<Vec<String>>,
    /// List of packages this package is incompatible with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    /// List of packages that this package replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    /// List of virtual packages or features that this package provides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Vec<String>>,
    /// Custom metadata tags for the package.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Configuration for extension-type packages.
    #[serde(default)]
    pub extension: Option<ExtensionInfo>,
    /// Whether this package supports rollback.
    #[serde(default)]
    pub rollback: Option<bool>,
    /// Update notices or warnings for the package.
    #[serde(default)]
    pub updates: Option<Vec<UpdateInfo>>,
    /// Lifecycle hooks for installation, upgrade, and removal.
    #[serde(default)]
    pub hooks: Option<Hooks>,
    /// Background service configuration.
    #[serde(default)]
    pub service: Option<Service>,
    /// Files or directories that should be backed up during upgrades.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Vec<String>>,
    /// Restricts installation to ZoiOS or non-ZoiOS systems.
    /// None (default): Works on both.
    /// Some(true): ZoiOS only.
    /// Some(false): Non-ZoiOS only.
    pub zoios: Option<bool>,

    /// The estimated size of the package after installation (in bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_size: Option<u64>,
    /// The size of the package archive (in bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_size: Option<u64>,
    /// Sandbox security configuration for the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// Shell completion scripts provided by the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Vec<ShellCompletion>>,
}

/// A list of commands to run, either as a simple list or mapped by platform.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    /// A simple list of commands.
    StringVec(Vec<String>),
    /// Commands mapped by platform.
    Platform(HashMap<String, Vec<String>>),
}

/// Lifecycle hooks that run during package installation, upgrade, or removal.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Hooks {
    /// Commands to run before installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_install: Option<PlatformOrStringVec>,
    /// Commands to run after installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_install: Option<PlatformOrStringVec>,
    /// Commands to run before upgrade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_upgrade: Option<PlatformOrStringVec>,
    /// Commands to run after upgrade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_upgrade: Option<PlatformOrStringVec>,
    /// Commands to run before removal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_remove: Option<PlatformOrStringVec>,
    /// Commands to run after removal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_remove: Option<PlatformOrStringVec>,
}

/// Information about a package maintainer.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Maintainer {
    /// The name of the maintainer.
    pub name: String,
    /// The email address of the maintainer.
    pub email: String,
    /// The website of the maintainer.
    pub website: Option<String>,
}

/// Information about a software author.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[allow(dead_code)]
pub struct Author {
    /// The name of the author.
    pub name: String,
    /// The email address of the author.
    pub email: Option<String>,
    /// The website of the author.
    pub website: Option<String>,
}

/// A group of optional dependencies.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DependencyOptionGroup {
    /// The name of the option group.
    pub name: String,
    /// A description of the option group.
    pub desc: String,
    /// Whether all dependencies in the group should be installed if the group is selected.
    #[serde(default)]
    pub all: bool,
    /// The list of dependencies in the group.
    pub depends: Vec<String>,
}

/// A group of dependencies, which can be simple or complex.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyGroup {
    /// A simple list of dependencies.
    Simple(Vec<String>),
    /// A complex group with required, optional, and sub-package dependencies.
    Complex(ComplexDependencyGroup),
}

impl DependencyGroup {
    /// Returns the required dependencies.
    pub fn required(&self) -> &[String] {
        match self {
            DependencyGroup::Simple(deps) => deps.as_slice(),
            DependencyGroup::Complex(group) => group.required.as_slice(),
        }
    }

    /// Returns the optional dependency groups.
    pub fn options(&self) -> &[DependencyOptionGroup] {
        match self {
            DependencyGroup::Simple(_) => &[],
            DependencyGroup::Complex(group) => group.options.as_slice(),
        }
    }

    /// Returns the individual optional dependencies.
    pub fn optional(&self) -> &[String] {
        match self {
            DependencyGroup::Simple(_) => &[],
            DependencyGroup::Complex(group) => group.optional.as_slice(),
        }
    }

    /// Returns a cloned list of required dependencies.
    pub fn get_required_simple(&self) -> Vec<String> {
        match self {
            DependencyGroup::Simple(deps) => deps.clone(),
            DependencyGroup::Complex(group) => group.required.clone(),
        }
    }

    /// Returns a cloned list of optional dependency groups.
    pub fn get_required_options(&self) -> Vec<DependencyOptionGroup> {
        match self {
            DependencyGroup::Simple(_) => Vec::new(),
            DependencyGroup::Complex(group) => group.options.clone(),
        }
    }

    /// Returns a reference to the individual optional dependencies.
    pub fn get_optional(&self) -> &Vec<String> {
        match self {
            DependencyGroup::Simple(_) => {
                static EMPTY_VEC: Vec<String> = Vec::new();
                &EMPTY_VEC
            }
            DependencyGroup::Complex(group) => &group.optional,
        }
    }

    /// Resolves the dependencies based on chosen options and optional packages.
    pub fn resolve(
        &self,
        chosen_options: &[String],
        chosen_optionals: &[String],
        sub_package: Option<&str>,
        all_optional: bool,
    ) -> Vec<String> {
        let mut result = Vec::new();
        match self {
            DependencyGroup::Simple(deps) => {
                result.extend(deps.clone());
            }
            DependencyGroup::Complex(group) => {
                result.extend(group.required.clone());

                for opt_group in &group.options {
                    for dep in &opt_group.depends {
                        if chosen_options.contains(dep) {
                            result.push(dep.clone());
                        }
                    }
                }

                for opt in &group.optional {
                    if all_optional || chosen_optionals.contains(opt) {
                        result.push(opt.clone());
                    }
                }

                if let Some(sub) = sub_package
                    && let Some(sub_map) = &group.sub_packages
                    && let Some(sub_group) = sub_map.get(sub)
                {
                    result.extend(sub_group.resolve(
                        chosen_options,
                        chosen_optionals,
                        None,
                        all_optional,
                    ));
                }
            }
        }
        result
    }
}

/// A complex dependency group with various types of dependencies.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct ComplexDependencyGroup {
    /// Required dependencies.
    #[serde(default)]
    pub required: Vec<String>,
    /// Optional dependency groups.
    #[serde(default)]
    pub options: Vec<DependencyOptionGroup>,
    /// Optional individual dependencies.
    #[serde(default)]
    pub optional: Vec<String>,
    /// Sub-package specific dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_packages: Option<HashMap<String, DependencyGroup>>,
}

/// Build dependencies categorized by build type.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct TypedBuildDependencies {
    /// Build dependencies for each build type.
    pub types: HashMap<String, DependencyGroup>,
}

/// Represents build dependencies, which can be typed or a single group.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum BuildDependencies {
    /// Build dependencies categorized by type.
    Typed(TypedBuildDependencies),
    /// A single group of build dependencies.
    Group(DependencyGroup),
}

/// Defines the dependencies of a package, including runtime, build, and test.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Dependencies {
    /// Runtime dependencies.
    #[serde(default)]
    pub runtime: Option<DependencyGroup>,
    /// Build-time dependencies.
    #[serde(default)]
    pub build: Option<BuildDependencies>,
    /// Test-time dependencies.
    #[serde(default)]
    pub test: Option<DependencyGroup>,
}

impl Dependencies {
    /// Resolves all dependency types based on user choices and build type.
    pub fn resolve(
        &self,
        chosen_options: &[String],
        chosen_optionals: &[String],
        sub_package: Option<&str>,
        all_optional: bool,
        build_type: Option<&str>,
    ) -> DependenciesV2 {
        let runtime = self
            .runtime
            .as_ref()
            .map(|g| g.resolve(chosen_options, chosen_optionals, sub_package, all_optional))
            .unwrap_or_default();

        let mut build = Vec::new();
        if let Some(b) = &self.build {
            match b {
                BuildDependencies::Group(g) => {
                    let packages =
                        g.resolve(chosen_options, chosen_optionals, sub_package, all_optional);
                    build.push(BuildDependencyV2 {
                        build_type: "source".to_string(),
                        packages,
                    });
                }
                BuildDependencies::Typed(t) => {
                    for (bt, g) in &t.types {
                        if build_type.is_none() || build_type == Some(bt) {
                            let packages = g.resolve(
                                chosen_options,
                                chosen_optionals,
                                sub_package,
                                all_optional,
                            );
                            build.push(BuildDependencyV2 {
                                build_type: bt.clone(),
                                packages,
                            });
                        }
                    }
                }
            }
        }

        let test = self
            .test
            .as_ref()
            .map(|g| g.resolve(chosen_options, chosen_optionals, sub_package, all_optional))
            .unwrap_or_default();

        DependenciesV2 {
            runtime,
            build,
            test,
        }
    }
}

/// Converts a `Dependencies` struct to a `DependenciesV2` struct.
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

    let mut test = Vec::new();
    if let Some(t) = deps.test {
        test = match t {
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

    DependenciesV2 {
        runtime,
        build,
        test,
    }
}

/// The reason for a package installation.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstallReason {
    /// The user explicitly requested to install this package.
    Direct,
    /// The package was installed as a dependency of another package.
    Dependency {
        /// The name of the package that depends on this one.
        parent: String,
    },
}

/// Configuration for sandboxing a package's execution.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct SandboxConfig {
    /// Whether sandboxing is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Whether network access is allowed within the sandbox.
    #[serde(default)]
    pub network: bool,
    /// Whether access to system files is allowed.
    #[serde(default)]
    pub system: bool,
    /// Whether access to the current working directory is allowed.
    #[serde(default)]
    pub cwd: bool,
    /// Paths allowed for reading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    /// Paths allowed for writing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write: Vec<String>,
    /// Environment variables allowed within the sandbox.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}

/// Configuration for Continuous Integration (CI) runners.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CiConfig {
    /// Tags for selecting appropriate CI runners.
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
    /// Forced version precedence.
    #[serde(default)]
    pub epoch: u32,
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
    /// List of packages this package conflicts with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<Vec<String>>,
    /// List of packages this package replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Vec<String>>,
    /// List of virtual packages this package provides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Vec<String>>,
    /// List of files to backup during upgrades.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<Vec<String>>,
    /// List of names of packages that were installed as dependencies.
    #[serde(default)]
    pub installed_dependencies: Vec<String>,
    /// Resolved dependencies at the time of installation.
    #[serde(default)]
    pub dependencies_v2: Option<DependenciesV2>,
    /// Dependency options chosen by the user.
    #[serde(default)]
    pub chosen_options: Vec<String>,
    /// Optional dependencies chosen by the user.
    #[serde(default)]
    pub chosen_optionals: Vec<String>,
    /// The method used to install the package (e.g. "source", "prebuilt").
    #[serde(default)]
    pub install_method: Option<String>,
    /// The platform for which the package was installed.
    #[serde(default)]
    pub platform: String,
    /// Background service configuration for this package.
    #[serde(default)]
    pub service: Option<Service>,
    /// List of all files installed by this package.
    #[serde(default)]
    pub installed_files: Vec<String>,
    /// The total size of all installed files in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_size: Option<u64>,
    /// Sandboxing configuration used for this installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,
    /// Shell completion scripts installed by this package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Vec<ShellCompletion>>,
}

/// Represents a single operation within a transaction.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TransactionOperation {
    /// Installing a new package.
    Install {
        /// The manifest of the package to be installed.
        manifest: Box<InstallManifest>,
    },
    /// Uninstalling an existing package.
    Uninstall {
        /// The manifest of the package to be uninstalled.
        manifest: Box<InstallManifest>,
    },
    /// Upgrading an existing package to a new version.
    Upgrade {
        /// The manifest of the currently installed version.
        old_manifest: Box<InstallManifest>,
        /// The manifest of the version to be installed.
        new_manifest: Box<InstallManifest>,
    },
}

/// A transaction containing a set of operations to be performed atomically.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    /// The unique identifier for the transaction.
    pub id: String,
    /// The time when the transaction was started.
    pub start_time: String,
    /// The list of operations to be performed in this transaction.
    pub operations: Vec<TransactionOperation>,
}

/// Helper function for serde to skip serializing empty authorities.
fn skip_authorities(a: &Option<Vec<String>>) -> bool {
    a.as_ref().is_none_or(|v| v.is_empty())
}

/// Configuration for a package registry.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct Registry {
    /// The handle or name of the registry.
    #[serde(default)]
    pub handle: String,
    /// The URL of the registry.
    #[serde(default)]
    pub url: String,
    /// The prefix used for security advisories in this registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    /// The list of trusted authorities for this registry.
    #[serde(default, skip_serializing_if = "skip_authorities")]
    pub authorities: Option<Vec<String>>,
}

/// Configuration for remote policy enforcement.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct RemotePolicyConfig {
    /// The URL of the remote policy file.
    #[serde(default)]
    pub url: String,
    /// The URL of the signature for the remote policy file.
    #[serde(default)]
    pub signature_url: String,
    /// The list of trusted PGP keys for verifying the remote policy signature.
    #[serde(default)]
    pub trusted_keys: Vec<String>,
}

/// The global Zoi configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// The list of repository tiers enabled globally.
    #[serde(default)]
    pub repos: Vec<String>,
    /// The list of supported package managers.
    #[serde(default)]
    pub package_managers: Option<Vec<String>>,
    /// The primary native package manager for the system.
    #[serde(default)]
    pub native_package_manager: Option<String>,
    /// Whether telemetry collection is enabled.
    pub telemetry_enabled: bool,
    /// Whether audit logging is enabled.
    pub audit_log_enabled: bool,
    /// The handle of the primary registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    /// The default registry configuration.
    #[serde(default)]
    pub default_registry: Option<Registry>,
    /// The list of additional registries.
    #[serde(default)]
    pub added_registries: Vec<Registry>,
    /// The list of third-party Git repositories to include in resolution.
    #[serde(default)]
    pub git_repos: Vec<String>,
    /// Whether rollback functionality is enabled.
    #[serde(default = "default_rollback_enabled")]
    pub rollback_enabled: bool,
    /// The local policy configuration.
    #[serde(default)]
    pub policy: Policy,
    /// The remote policy configuration.
    #[serde(default)]
    pub remote_policy: Option<RemotePolicyConfig>,
    /// The maximum number of concurrent jobs to use for builds and downloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<usize>,
    /// Whether to protect the Zoi database from unauthorized modifications.
    pub protect_db: bool,
    /// The maximum depth for recursive dependency resolution.
    #[serde(default)]
    pub max_resolution_depth: Option<u8>,
    /// Whether Zoi should operate in offline mode.
    pub offline_mode: bool,
    /// The list of directories where packages are stored.
    #[serde(default)]
    pub pkg_dirs: Vec<String>,
    /// The list of mirror URLs for the package cache.
    #[serde(default)]
    pub cache_mirrors: Vec<String>,
    /// A map of package names to pinned versions.
    #[serde(default)]
    pub versions: HashMap<String, String>,
    /// The maximum number of system generations to keep for rollbacks.
    #[serde(default = "default_system_generations_limit")]
    pub system_generations_limit: u32,
}

/// Default system generations limit.
fn default_system_generations_limit() -> u32 {
    4
}

/// Default rollback enabled status.
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
            system_generations_limit: 4,
        }
    }
}

/// Policy settings for enforcing constraints on Zoi's operation.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Policy {
    /// Whether repository settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub repos_unoverridable: bool,
    /// Whether telemetry settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub telemetry_enabled_unoverridable: bool,
    /// Whether audit logging settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub audit_log_enabled_unoverridable: bool,
    /// Whether rollback settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rollback_enabled_unoverridable: bool,
    /// Whether default registry settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub default_registry_unoverridable: bool,
    /// Whether added registries settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub added_registries_unoverridable: bool,
    /// Whether Git repository settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub git_repos_unoverridable: bool,
    /// Whether allow/deny lists are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_deny_lists_unoverridable: bool,
    /// Whether signature enforcement settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_enforcement_unoverridable: bool,
    /// Whether database protection settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub protect_db_unoverridable: bool,
    /// Whether maximum resolution depth settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub max_resolution_depth_unoverridable: bool,
    /// Whether offline mode settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub offline_mode_unoverridable: bool,
    /// Whether package directory settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pkg_dirs_unoverridable: bool,
    /// Whether cache mirror settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_mirrors_unoverridable: bool,
    /// Whether concurrent job settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub jobs_unoverridable: bool,
    /// Whether security advisory enforcement settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub advisory_enforcement_unoverridable: bool,
    /// Whether system generations limit settings are unoverridable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub system_generations_limit_unoverridable: bool,

    /// List of allowed package licenses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_licenses: Option<Vec<String>>,
    /// List of denied package licenses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_licenses: Option<Vec<String>>,

    /// List of explicitly allowed packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_packages: Option<Vec<String>>,
    /// List of explicitly denied packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_packages: Option<Vec<String>>,

    /// List of allowed repository URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_repos: Option<Vec<String>>,
    /// List of denied repository URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_repos: Option<Vec<String>>,

    /// Policy for enforcing cryptographic signatures on packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_enforcement: Option<SignatureEnforcementPolicy>,
}

/// Policy for enforcing cryptographic signatures on packages.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SignatureEnforcementPolicy {
    /// Whether signature enforcement is enabled.
    #[serde(default)]
    pub enable: bool,
    /// List of trusted PGP keys.
    #[serde(default)]
    pub trusted_keys: Vec<String>,
}

/// Helper function for serde to skip serializing false booleans.
fn is_false(b: &bool) -> bool {
    !*b
}

/// A simplified version of an install manifest that can be easily shared or compared.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SharableInstallManifest {
    /// Name of the package.
    pub name: String,
    /// Version of the package.
    pub version: String,
    /// Repository tier.
    pub repo: String,
    /// Registry handle.
    pub registry_handle: String,
    /// Installation scope.
    pub scope: Scope,
    /// Sub-package name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    /// Chosen dependency options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chosen_options: Vec<String>,
    /// Chosen optional dependencies.
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

/// Pre-computed metadata for a package in the registry index.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurlPackageIndexV2 {
    /// Repository tier.
    pub repo: String,
    /// Repository trust type.
    pub repo_type: String,
    /// Version string.
    pub version: String,
    /// Version precedence epoch.
    #[serde(default)]
    pub epoch: u32,
    /// Revision string.
    pub revision: String,
    /// Short description.
    pub description: String,
    /// Default scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    /// Allowed scopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<Scope>>,
    /// Available sub-packages.
    pub sub_packages: Vec<String>,
    /// Main sub-packages.
    pub main_sub_packages: Vec<String>,
    /// Known vulnerabilities.
    pub vuln: Vec<MiniVulnerability>,
    /// Resolved dependencies.
    pub dependencies: Option<DependenciesV2>,
}

/// Resolved dependencies in a simplified format (V2).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct DependenciesV2 {
    /// Runtime dependencies.
    #[serde(default)]
    pub runtime: Vec<String>,
    /// Build-time dependencies.
    #[serde(default)]
    pub build: Vec<BuildDependencyV2>,
    /// Test-time dependencies.
    #[serde(default)]
    pub test: Vec<String>,
}

/// A build dependency with its associated build type.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BuildDependencyV2 {
    /// The build type (e.g. "source", "prebuilt").
    #[serde(rename = "type")]
    pub build_type: String,
    /// The list of required packages for this build type.
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

/// Detailed lock information for a registry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockRegistryV2 {
    /// The exact Git revision of the registry.
    pub revision: String,
    /// The URL of the registry.
    pub url: String,
}

/// Detailed lock information for an installed package.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockPackageDetailV2 {
    /// The name of the package.
    pub name: String,
    /// The sub-package name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_package: Option<String>,
    /// The repository tier the package came from.
    pub repo: String,
    /// The trust level of the repository.
    pub repo_type: String,
    /// The version of the package.
    pub version: String,
    /// The version precedence epoch.
    #[serde(default)]
    pub epoch: u32,
    /// The revision of the package definition.
    pub revision: String,
    /// The handle of the registry the package was resolved from.
    pub registry: String,
    /// The reason why this package was installed.
    pub why: String,
    /// A short description of the package.
    pub description: String,
    /// The category of the installed package.
    #[serde(rename = "type")]
    pub package_type_install: String,
    /// The method used to install the package.
    pub install_method: String,
    /// The list of installed sub-packages.
    pub installed_sub_packages: Vec<String>,
    /// The platform for which the package was installed.
    pub platform: String,
    /// The SHA-512 hash of the installed package content.
    pub hash: String,
    /// The resolved dependencies of the package.
    pub dependencies: Option<DependenciesV2>,
}

/// A link to a Git repository in a repository configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitLink {
    /// The type of link (e.g. "official").
    #[serde(rename = "type")]
    pub link_type: String,
    /// The URL of the Git repository.
    pub url: String,
}

/// A link to a package index or archive in a repository configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PkgLink {
    /// The type of link (e.g. "official").
    #[serde(rename = "type")]
    pub link_type: String,
    /// The URL of the package index or archive.
    pub url: String,
    /// Optional URL for PGP signature.
    pub pgp: Option<String>,
    /// Optional URL for checksum.
    pub hash: Option<String>,
    /// Optional URL for size information.
    pub size: Option<String>,
    /// Optional URL for file list.
    pub files: Option<String>,
}

/// A PGP public key in a repository configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PgpKey {
    /// The name or identifier of the key.
    pub name: String,
    /// The PGP public key content.
    pub key: String,
}

/// An entry for a sub-repository in a repository configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoEntry {
    /// The name of the sub-repository.
    pub name: String,
    /// The type of the sub-repository.
    #[serde(rename = "type")]
    pub repo_type: String,
    /// Whether this sub-repository is active by default.
    pub active: bool,
}

/// The structure of a `repo.yaml` repository configuration file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoConfig {
    /// The version of the repository configuration format.
    #[serde(default = "default_version")]
    pub version: String,
    /// The name of the repository.
    pub name: String,
    /// A short description of the repository.
    pub description: String,
    /// The prefix used for security advisories in this repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory_prefix: Option<String>,
    /// The list of Git repositories associated with this repository.
    pub git: Vec<GitLink>,
    /// The list of package indices or archives.
    #[serde(default)]
    pub pkg: Vec<PkgLink>,
    /// Optional URL to a pre-compiled database.
    #[serde(default)]
    pub db: Option<String>,
    /// The list of PGP keys for verification.
    #[serde(default)]
    pub pgp: Vec<PgpKey>,
    /// The list of sub-repositories.
    pub repos: Vec<RepoEntry>,
}

/// Information about a pre-built package.
#[derive(Debug, Clone)]
pub struct PrebuiltInfo {
    /// The final download URL for the package archive.
    pub final_url: String,
    /// Optional URL for the PGP signature.
    pub pgp_url: Option<String>,
    /// Optional URL for the checksum.
    pub hash_url: Option<String>,
    /// Optional URL for the size information.
    pub size_url: Option<String>,
    /// Optional URL for the file list.
    pub files_url: Option<String>,
}

/// The type of source from which a package is installed.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SourceType {
    /// An official repository tier.
    OfficialRepo,
    /// A third-party repository URL.
    UntrustedRepo(String),
    /// A Git repository URL.
    GitRepo(String),
    /// A local `.pkg.lua` or `.zpa` file.
    LocalFile,
    /// A direct URL to a package definition or archive.
    Url,
}
