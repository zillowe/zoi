use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Project-local configuration overrides.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProjectLocalConfig {
    /// Whether the project is isolated from the system registry.
    #[serde(default)]
    pub local: bool,
}

/// Shell configuration for the project.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct ShellSpec {
    /// Environment variables for the shell, potentially platform-specific.
    #[serde(default)]
    pub env: PlatformOrEnvMap,
}

/// Specification for a project-scoped registry.
#[derive(Debug, Deserialize, Clone)]
pub struct RegistrySpec {
    /// The URL of the registry.
    pub url: String,
    /// The git revision of the registry.
    pub revision: Option<String>,
    /// The type of registry (e.g. "git").
    #[serde(rename = "type")]
    pub registry_type: Option<String>,
}

/// Specification for a package dependency.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageSpec {
    /// The type of package.
    #[serde(rename = "type")]
    pub package_type: Option<String>,
    /// The method used to install the package.
    pub install_method: Option<String>,
    /// List of sub-packages to install.
    pub sub_packages: Option<Vec<String>>,
    /// The version requirement for the package.
    pub version: Option<String>,
    /// Dependencies specific to this package.
    pub dependencies: Option<zoi_core::types::Dependencies>,
    /// List of build/install options.
    pub options: Option<Vec<String>>,
    /// List of optional features to enable.
    pub optionals: Option<Vec<String>>,
}

/// Represents the combined evaluation of a project's `zoi.lua` and `zoi.yaml` configuration.
///
/// This struct acts as the central definition for a project environment. It unifies:
/// - The scriptable package and registry requirements defined in `zoi.lua`.
/// - The declarative task (`commands`) and `environments` defined in `zoi.yaml`.
#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProjectConfig {
    /// The name of the project.
    pub name: String,
    /// Registries scoped specifically to this project.
    #[serde(default)]
    pub registries: HashMap<String, RegistrySpec>,
    /// Declarative package checks (legacy v1).
    #[serde(default)]
    pub packages: Vec<PackageCheck>,
    /// A flat list of simple package dependencies.
    #[serde(default)]
    pub pkgs: Vec<String>,
    /// A map of packages defining explicit version requirements and options.
    #[serde(default)]
    pub pkgs_v2: HashMap<String, PackageSpec>,
    /// Project-local configuration overrides (e.g. `--local` isolation).
    #[serde(default)]
    pub config: ProjectLocalConfig,
    /// Declared task aliases and their underlying scripts.
    #[serde(default)]
    pub commands: Vec<CommandSpec>,
    /// Full environment setup groups.
    #[serde(default)]
    pub environments: Vec<EnvironmentSpec>,
    /// Ephemeral shell configurations.
    #[serde(default)]
    pub shell: Option<ShellSpec>,
}

/// A declarative package check.
#[derive(Debug, Deserialize, Clone)]
pub struct PackageCheck {
    /// The name of the package.
    pub name: String,
    /// The check command or requirement.
    pub check: String,
}

/// A value that can be a single string or a map of platform-specific strings.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrString {
    /// A simple string value.
    String(String),
    /// A map of platform names to string values.
    Platform(HashMap<String, String>),
}

/// A value that can be a list of strings or a map of platform-specific string lists.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    /// A simple list of strings.
    StringVec(Vec<String>),
    /// A map of platform names to lists of strings.
    Platform(HashMap<String, Vec<String>>),
}

/// A value that can be an environment map or a map of platform-specific environment maps.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrEnvMap {
    /// A simple environment map.
    EnvMap(HashMap<String, String>),
    /// A map of platform names to environment maps.
    Platform(HashMap<String, HashMap<String, String>>),
}

impl Default for PlatformOrEnvMap {
    fn default() -> Self {
        PlatformOrEnvMap::EnvMap(HashMap::new())
    }
}

/// Specification for a declarative task/command.
#[derive(Debug, Deserialize, Clone)]
pub struct CommandSpec {
    /// The name of the task.
    pub cmd: String,
    /// The command to run, potentially platform-specific.
    pub run: PlatformOrString,
    /// Environment variables for the task.
    #[serde(default)]
    pub env: PlatformOrEnvMap,
    /// List of task names this task depends on.
    #[serde(default)]
    pub depends_on: Option<Vec<String>>,
    /// List of files that contribute to the task's cache hash.
    #[serde(default)]
    pub cache_files: Option<Vec<String>>,
}

/// Specification for a project environment setup.
#[derive(Debug, Deserialize, Clone)]
pub struct EnvironmentSpec {
    /// The name of the environment.
    pub name: String,
    /// The command associated with this environment.
    pub cmd: String,
    /// The commands to run to setup the environment.
    pub run: PlatformOrStringVec,
    /// Environment variables for this setup.
    #[serde(default)]
    pub env: PlatformOrEnvMap,
}

/// Loads the project configuration from the current directory.
pub fn load() -> Result<ProjectConfig> {
    load_with_env(std::env::vars().collect())
}

/// Loads the project configuration with a custom set of environment variables.
pub fn load_with_env(env: HashMap<String, String>) -> Result<ProjectConfig> {
    let lua_path = Path::new("zoi.lua");
    if lua_path.exists() {
        return crate::lua_config::load_zoi_lua(lua_path, env);
    }

    let config_path = Path::new("zoi.yaml");
    if !config_path.exists() {
        return Err(anyhow!(
            "No 'zoi.lua' or 'zoi.yaml' file found in the current directory."
        ));
    }

    let content = fs::read_to_string(config_path)?;
    let config: ProjectConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Adds packages to the `zoi.yaml` configuration file.
pub fn add_packages_to_config(packages: &[String]) -> Result<()> {
    if Path::new("zoi.lua").exists() {
        return Err(anyhow!(
            "Project uses zoi.lua. Automatic saving is not supported for Lua configurations."
        ));
    }
    let config_path = Path::new("zoi.yaml");
    if !config_path.exists() {
        return Err(anyhow!(
            "No 'zoi.yaml' file found in the current directory."
        ));
    }

    let content = fs::read_to_string(config_path)?;
    let mut yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;

    if let Some(mapping) = yaml_value.as_mapping_mut() {
        let pkgs_key = serde_yaml::Value::String("pkgs".to_string());
        let pkgs_list = mapping
            .entry(pkgs_key)
            .or_insert_with(|| serde_yaml::Value::Sequence(Vec::new()));

        if let Some(sequence) = pkgs_list.as_sequence_mut() {
            for package in packages {
                let new_pkg_value = serde_yaml::Value::String(package.clone());
                if !sequence.contains(&new_pkg_value) {
                    sequence.push(new_pkg_value);
                }
            }
        }
    }

    let new_content = serde_yaml::to_string(&yaml_value)?;
    fs::write(config_path, new_content)?;

    Ok(())
}

/// Removes packages from the `zoi.yaml` configuration file.
pub fn remove_packages_from_config(packages_to_remove: &[String]) -> Result<()> {
    if Path::new("zoi.lua").exists() {
        return Ok(());
    }
    let config_path = Path::new("zoi.yaml");
    if !config_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(config_path)?;
    let mut yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;

    if let Some(mapping) = yaml_value.as_mapping_mut()
        && let Some(pkgs_list) = mapping.get_mut("pkgs")
        && let Some(sequence) = pkgs_list.as_sequence_mut()
    {
        let packages_to_remove_names: Vec<_> = packages_to_remove
            .iter()
            .map(|p| {
                zoi_resolver::resolve::parse_source_string(p)
                    .map(|req| req.name)
                    .unwrap_or_else(|_| p.to_string())
            })
            .collect();

        sequence.retain(|v| {
            if let Some(s) = v.as_str() {
                if let Ok(req) = zoi_resolver::resolve::parse_source_string(s) {
                    !packages_to_remove_names.contains(&req.name)
                } else {
                    true
                }
            } else {
                true
            }
        });
    }

    let new_content = serde_yaml::to_string(&yaml_value)?;
    fs::write(config_path, new_content)?;

    Ok(())
}
