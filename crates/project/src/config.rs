use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

/// Project-local configuration overrides.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProjectLocalConfig {
    /// Whether the project is isolated from the system registry.
    #[serde(default)]
    pub local: bool
}

/// Shell configuration for the project.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct ShellSpec {
    /// Environment variables for the shell, potentially platform-specific.
    #[serde(default)]
    pub env: PlatformOrEnvMap
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
    pub registry_type: Option<String>
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
    pub optionals: Option<Vec<String>>
}

/// Represents the evaluation of a project's `zoi.lua` configuration.
///
/// This struct acts as the central definition for a project environment. It
/// unifies the scriptable package and registry requirements, task aliases
/// (`tasks`), environment setups (`environments`), ephemeral shell
/// configuration (`shell`), and declarative package checks (`checks`)
/// defined in `zoi.lua`.
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
    /// A flat list of simple package dependencies. Each entry may be a plain
    /// package name (e.g. `eza`) or a version requirement expressed as a map
    /// (e.g. `fzf: "0.44.1"`, which is normalized to `fzf@0.44.1`).
    #[serde(default, deserialize_with = "deserialize_pkgs")]
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
    pub shell: Option<ShellSpec>
}

/// Deserializes the flat `pkgs` list where each entry can either be a plain
/// package name or a map of `name -> version` (e.g. `fzf: "0.44.1"`). Versioned
/// entries are normalized to the canonical `name@version` form.
fn deserialize_pkgs<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>
{
    let entries = Vec::<serde_json::Value>::deserialize(deserializer)
        .map_err(serde::de::Error::custom)?;

    let mut pkgs = Vec::new();
    for entry in entries {
        match entry {
            serde_json::Value::String(name) => pkgs.push(name),
            serde_json::Value::Object(map) => {
                for (name, version) in map {
                    if let Some(ver) = version.as_str() {
                        pkgs.push(format!("{name}@{ver}"));
                    } else if let serde_json::Value::Number(num) = version {
                        pkgs.push(format!("{name}@{num}"));
                    } else {
                        pkgs.push(name);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(pkgs)
}

/// A declarative package check.
#[derive(Debug, Deserialize, Clone)]
pub struct PackageCheck {
    /// The name of the package.
    pub name: String,
    /// The check command or requirement.
    pub check: String
}

/// A value that can be a single string or a map of platform-specific strings.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrString {
    /// A simple string value.
    String(String),
    /// A map of platform names to string values.
    Platform(HashMap<String, String>)
}

/// A value that can be a list of strings or a map of platform-specific string
/// lists.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    /// A simple list of strings.
    StringVec(Vec<String>),
    /// A map of platform names to lists of strings.
    Platform(HashMap<String, Vec<String>>)
}

/// A value that can be an environment map or a map of platform-specific
/// environment maps.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrEnvMap {
    /// A simple environment map.
    EnvMap(HashMap<String, String>),
    /// A map of platform names to environment maps.
    Platform(HashMap<String, HashMap<String, String>>)
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
    pub cache_files: Option<Vec<String>>
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
    pub env: PlatformOrEnvMap
}

/// Loads the project configuration from the current directory.
///
/// # Errors
///
/// Returns an error if no configuration file is found or if the configuration
/// is invalid.
pub fn load() -> Result<ProjectConfig> {
    let env: HashMap<String, String> = std::env::vars().collect();
    load_with_env(&env)
}

/// Loads the project configuration with a custom set of environment variables.
///
/// # Errors
///
/// Returns an error if no configuration file is found or if the configuration
/// is invalid.
pub fn load_with_env<S: ::std::hash::BuildHasher>(
    env: &HashMap<String, String, S>
) -> Result<ProjectConfig> {
    let lua_path = Path::new("zoi.lua");
    if !lua_path.exists() {
        return Err(anyhow!(
            "No 'zoi.lua' file found in the current directory."
        ));
    }

    crate::lua_config::load_zoi_lua(lua_path, env)
}

/// Finds the line index that opens the top-level `packages({...})` block.
///
/// Matches the opening line (e.g. `packages({`) as well as an empty
/// single-line block (`packages({})`).
fn find_packages_block_open(lines: &[&str]) -> Option<usize> {
    lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("packages(")
            && trimmed.contains('{')
            && !trimmed.contains('=')
    })
}

/// Extracts the plain package spec from a `packages({...})` entry line.
///
/// Only simple string entries (e.g. `"@core/eza",`) are considered; keyed
/// entries like `["@core/fzf"] = {...}` span multiple lines and are left
/// untouched, mirroring how versioned maps were preserved.
fn plain_package_entry(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if !trimmed.starts_with('"') || trimmed.contains('=') {
        return None;
    }
    let inner = trimmed.trim_matches('"');
    if inner.is_empty() || inner.contains('"') {
        return None;
    }
    Some(inner.to_string())
}

/// Adds packages to the `packages({...})` block of the `zoi.lua`
/// configuration file, creating the block when missing.
///
/// # Errors
///
/// Returns an error if no `zoi.lua` file exists or if it cannot be read or
/// written.
pub fn add_packages_to_config(packages: &[String]) -> Result<()> {
    let config_path = Path::new("zoi.lua");
    if !config_path.exists() {
        return Err(anyhow!(
            "No 'zoi.lua' file found in the current directory."
        ));
    }

    let content = fs::read_to_string(config_path)?;
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

    let mut missing: Vec<&String> = packages
        .iter()
        .filter(|package| {
            !lines
                .iter()
                .any(|line| plain_package_entry(line).as_ref() == Some(package))
        })
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let new_entries: Vec<String> = missing
        .drain(..)
        .map(|package| format!("    \"{package}\","))
        .collect();

    if let Some(open_idx) = find_packages_block_open(
        &lines.iter().map(String::as_str).collect::<Vec<_>>()
    ) && let Some(open_line) =
        lines.get(open_idx).map(|line| line.trim().to_string())
    {
        if open_line.contains('}') {
            // Single-line block (e.g. `packages({"a"})`): expand it,
            // preserving the existing entries verbatim.
            let start = open_line.find('{').map_or(0, |i| i + 1);
            let end = open_line.rfind('}').unwrap_or(open_line.len());
            let mut inner = open_line
                .get(start..end)
                .unwrap_or_default()
                .trim()
                .to_string();
            if !inner.is_empty() && !inner.ends_with(',') {
                inner.push(',');
            }
            let mut replacement = vec!["packages({".to_string()];
            if !inner.is_empty() {
                replacement.push(format!("    {inner}"));
            }
            replacement.extend(new_entries);
            replacement.push("})".to_string());
            lines.splice(open_idx..=open_idx, replacement);
        } else {
            for (insert_at, entry) in (open_idx + 1..).zip(new_entries) {
                lines.insert(insert_at, entry);
            }
        }
    } else {
        if !lines.is_empty()
            && !lines.last().is_none_or(std::string::String::is_empty)
        {
            lines.push(String::new());
        }
        lines.push("packages({".to_string());
        lines.extend(new_entries);
        lines.push("})".to_string());
    }

    let mut new_content = lines.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }
    fs::write(config_path, new_content)?;

    Ok(())
}

/// Removes plain package entries from the `packages({...})` block of the
/// `zoi.lua` configuration file.
///
/// # Errors
///
/// Returns an error if the `zoi.lua` file cannot be read or written. A
/// missing file is a no-op so uninstalls outside of projects keep working.
pub fn remove_packages_from_config(
    packages_to_remove: &[String]
) -> Result<()> {
    let config_path = Path::new("zoi.lua");
    if !config_path.exists() {
        return Ok(());
    }

    let packages_to_remove_names: Vec<_> = packages_to_remove
        .iter()
        .map(|p| {
            zoi_resolver::resolve::parse_source_string(p)
                .map_or_else(|_| p.clone(), |req| req.name)
        })
        .collect();

    let content = fs::read_to_string(config_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let open_idx = find_packages_block_open(&lines);

    // A line ends the packages block when it closes the call (`})`) or
    // starts another top-level block call (`tasks({`, ...).
    let is_block_end = |line: &str| {
        let trimmed = line.trim();
        trimmed == "})"
            || (trimmed.starts_with(|c: char| c.is_ascii_alphabetic())
                && trimmed.contains('('))
    };

    let mut kept = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        if open_idx.is_some_and(|open| idx == open)
            && line.trim().contains('}')
            && !line.contains('=')
        {
            // Single-line block: drop matching entries inline.
            let trimmed = line.trim().to_string();
            let start = trimmed.find('{').map_or(0, |i| i + 1);
            let end = trimmed.rfind('}').unwrap_or(trimmed.len());
            let inner = trimmed.get(start..end).unwrap_or_default();
            let remaining: Vec<&str> = inner
                .split(',')
                .map(str::trim)
                .filter(|part| {
                    let entry = part.trim_matches('"');
                    part.starts_with('"')
                        && !entry.contains('"')
                        && zoi_resolver::resolve::parse_source_string(entry)
                            .is_ok_and(|req| {
                                !packages_to_remove_names.contains(&req.name)
                            })
                })
                .collect();
            let indent_len = line.len() - line.trim_start_matches(' ').len();
            kept.push(format!(
                "{}packages({{{}}})",
                line.get(..indent_len).unwrap_or_default(),
                remaining.join(", ")
            ));
            continue;
        }

        let in_block = open_idx.is_some_and(|open| {
            idx > open
                && !lines.get(open + 1..idx).is_some_and(|window| {
                    window.iter().any(|l| is_block_end(l))
                })
        });
        if in_block
            && let Some(entry) = plain_package_entry(line)
            && let Ok(req) = zoi_resolver::resolve::parse_source_string(&entry)
            && packages_to_remove_names.contains(&req.name)
        {
            continue;
        }
        kept.push((*line).to_string());
    }

    let mut new_content = kept.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }
    fs::write(config_path, new_content)?;

    Ok(())
}
