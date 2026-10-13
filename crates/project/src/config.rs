use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

/// Project-local configuration overrides.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ProjectLocalConfig {
    /// Whether the project is isolated from the system registry.
    #[serde(default)]
    pub local: bool
}

/// Shell configuration for the project.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShellSpec {
    /// Environment variables for the shell, potentially platform-specific.
    #[serde(default)]
    pub env: PlatformOrEnvMap
}

/// Specification for a project-scoped registry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistrySpec {
    /// The URL of the registry.
    pub url: String,
    /// The git revision of the registry.
    pub revision: Option<String>,
    /// The type of registry (e.g. "git").
    #[serde(rename = "type")]
    pub registry_type: Option<String>
}

/// Specification for a repository import.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ImportSpec {
    /// The repository in `owner/repo` form.
    pub repo: String,
    /// The git revision to import.
    #[serde(default)]
    pub rev: Option<String>
}

/// Specification for exported package definitions.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PackageExportSpec {
    /// The main package definition.
    #[serde(default)]
    pub main: Option<String>,
    /// Named additional package definitions.
    #[serde(default)]
    pub packages: BTreeMap<String, String>
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub shell: Option<ShellSpec>,
    /// Repository imports declared by the configuration.
    #[serde(default)]
    pub imports: BTreeMap<String, ImportSpec>,
    /// Package definitions exported by a repository configuration.
    #[serde(default)]
    pub package_export: Option<PackageExportSpec>
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageCheck {
    /// The name of the package.
    pub name: String,
    /// The check command or requirement.
    pub check: String
}

/// A value that can be a single string or a map of platform-specific strings.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrString {
    /// A simple string value.
    String(String),
    /// A map of platform names to string values.
    Platform(HashMap<String, String>)
}

/// A value that can be a list of strings or a map of platform-specific string
/// lists.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    /// A simple list of strings.
    StringVec(Vec<String>),
    /// A map of platform names to lists of strings.
    Platform(HashMap<String, Vec<String>>)
}

/// A value that can be an environment map or a map of platform-specific
/// environment maps.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// Extracts the package spec from a plain `packages({...})` entry line.
///
/// Only simple string entries (e.g. `"@core/eza",`) are considered. Keyed
/// entries like `["@core/fzf"] = {...}` are handled separately by
/// [`keyed_package_entry`], because they span multiple lines.
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

/// Extracts the package spec from a keyed entry's opening line.
///
/// A keyed entry opens with the package spec as a quoted key, e.g.
/// `["@core/fzf"] = {`, and its body continues until the matching close. Only
/// the opening line carries the spec, so callers that need to match on the key
/// must read this instead of [`plain_package_entry`].
fn keyed_package_entry(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    let rest = trimmed.strip_prefix('[')?;
    let (key, after) = rest.split_once(']')?;
    // A `= {` after the key is what makes this an entry rather than some other
    // indexed construct in the block.
    let after = after.trim();
    if !(after.starts_with('=') && after[1..].trim_start().starts_with('{')) {
        return None;
    }
    let key = key.trim();
    let inner = key.strip_prefix('"')?.strip_suffix('"')?;
    if inner.is_empty() {
        return None;
    }
    Some(inner.to_string())
}

/// Extracts the package spec from a `packages({...})` entry, in either shape.
fn any_package_entry(line: &str) -> Option<String> {
    plain_package_entry(line).or_else(|| keyed_package_entry(line))
}

/// Compares two package specs for identity.
///
/// Specs are compared on the parsed name and sub-package rather than as raw
/// strings, so `@core/eza:1.0.0` matches `@core/eza`. The sub-package is
/// significant: `eza` and `eza:icons` are different installable things, and
/// the loader keeps them as separate entries.
fn same_package(a: &str, b: &str) -> bool {
    match (
        zoi_resolver::resolve::parse_source_string(a),
        zoi_resolver::resolve::parse_source_string(b)
    ) {
        (Ok(a_req), Ok(b_req)) => {
            a_req.name == b_req.name && a_req.sub_package == b_req.sub_package
        }
        // Unparseable specs (bare paths, for instance) fall back to an exact
        // match rather than guessing.
        _ => a == b
    }
}

/// Decides whether an entry should be removed given what was uninstalled.
///
/// Uninstalling a base package removes every sub-package belonging to it,
/// matching `zoi uninstall` on disk. Uninstalling `name:sub` removes only that
/// sub-package and leaves both the base entry and its sibling sub-packages
/// alone.
fn entry_matches_removal(entry: &str, removal: &str) -> bool {
    let Ok(entry_req) = zoi_resolver::resolve::parse_source_string(entry)
    else {
        return entry == removal;
    };
    let Ok(removal_req) = zoi_resolver::resolve::parse_source_string(removal)
    else {
        return entry == removal;
    };

    if entry_req.name != removal_req.name {
        return false;
    }

    match removal_req.sub_package {
        // A base-package uninstall cascades to its sub-packages.
        None => true,
        // A targeted sub-package uninstall only touches that sub-package.
        Some(ref sub) => entry_req.sub_package.as_deref() == Some(sub.as_str())
    }
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

    // A package counts as present in either entry shape. Checking only plain
    // string entries would add a duplicate next to an existing keyed entry,
    // leaving the same package declared twice in a Lua table - where the
    // later key silently wins.
    let existing: Vec<String> =
        lines.iter().filter_map(|l| any_package_entry(l)).collect();

    let mut missing: Vec<&String> = packages
        .iter()
        .filter(|package| !existing.iter().any(|e| same_package(e, package)))
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

/// Removes package entries from the `packages({...})` block of the `zoi.lua`
/// configuration file.
///
/// Handles both entry shapes: plain string entries (`"@core/eza",`) and keyed
/// entries (`["@core/fzf"] = { ... }`) whose body spans several lines.
///
/// Uninstalling a base package removes all of its sub-package entries, which
/// mirrors what the uninstall does on disk. Uninstalling `name:sub` removes
/// only that sub-package.
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

    let is_inside_block = |idx: usize| {
        open_idx.is_some_and(|open| {
            idx > open
                && !lines.get(open + 1..idx).is_some_and(|window| {
                    window.iter().any(|l| is_block_end(l))
                })
        })
    };

    let should_remove = |entry: &str| {
        packages_to_remove
            .iter()
            .any(|removal| entry_matches_removal(entry, removal))
    };

    let mut kept = Vec::with_capacity(lines.len());
    let mut idx = 0;
    while let Some(line) = lines.get(idx).copied() {
        if Some(idx) == open_idx
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
                    part.starts_with('"')
                        && !part.trim_matches('"').contains('"')
                        && !should_remove(part.trim_matches('"'))
                })
                .collect();
            let indent_len = line.len() - line.trim_start_matches(' ').len();
            kept.push(format!(
                "{}packages({{{}}})",
                line.get(..indent_len).unwrap_or_default(),
                remaining.join(", ")
            ));
            idx += 1;
            continue;
        }

        // A keyed entry opens with `["spec"] = {` and its body continues until
        // the braces balance. The whole entry has to go, not just the opening
        // line, so track the nesting depth and skip the body too.
        if is_inside_block(idx)
            && let Some(entry) = keyed_package_entry(line)
        {
            if should_remove(&entry) {
                let mut depth = 0i32;
                while let Some(entry_line) = lines.get(idx) {
                    depth += entry_line.matches('{').count() as i32;
                    depth -= entry_line.matches('}').count() as i32;
                    idx += 1;
                    if depth <= 0 {
                        break;
                    }
                }
                continue;
            }
            kept.push((*line).to_string());
            idx += 1;
            continue;
        }

        if is_inside_block(idx)
            && let Some(entry) = plain_package_entry(line)
            && should_remove(&entry)
        {
            idx += 1;
            continue;
        }

        kept.push((*line).to_string());
        idx += 1;
    }

    let mut new_content = kept.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }
    fs::write(config_path, new_content)?;

    Ok(())
}
