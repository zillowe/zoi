use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct ProjectLocalConfig {
    #[serde(default)]
    pub local: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub packages: Vec<PackageCheck>,
    #[serde(default)]
    pub pkgs: Vec<String>,
    #[serde(default)]
    pub config: ProjectLocalConfig,
    #[serde(default)]
    pub commands: Vec<CommandSpec>,
    #[serde(default)]
    pub environments: Vec<EnvironmentSpec>,
}

#[derive(Debug, Deserialize)]
pub struct PackageCheck {
    pub name: String,
    pub check: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrString {
    String(String),
    Platform(HashMap<String, String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrStringVec {
    StringVec(Vec<String>),
    Platform(HashMap<String, Vec<String>>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformOrEnvMap {
    EnvMap(HashMap<String, String>),
    Platform(HashMap<String, HashMap<String, String>>),
}

impl Default for PlatformOrEnvMap {
    fn default() -> Self {
        PlatformOrEnvMap::EnvMap(HashMap::new())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommandSpec {
    pub cmd: String,
    pub run: PlatformOrString,
    #[serde(default)]
    pub env: PlatformOrEnvMap,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EnvironmentSpec {
    pub name: String,
    pub cmd: String,
    pub run: PlatformOrStringVec,
    #[serde(default)]
    pub env: PlatformOrEnvMap,
}

pub fn load() -> Result<ProjectConfig> {
    let config_path = Path::new("zoi.yaml");
    if !config_path.exists() {
        return Err(anyhow!(
            "No 'zoi.yaml' file found in the current directory."
        ));
    }

    let content = fs::read_to_string(config_path)?;
    let config: ProjectConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn add_packages_to_config(packages: &[String]) -> Result<()> {
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
