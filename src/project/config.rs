use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub packages: Vec<PackageCheck>,
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

pub fn load() -> Result<ProjectConfig, Box<dyn Error>> {
    let config_path = Path::new("zoi.yaml");
    if !config_path.exists() {
        return Err("No 'zoi.yaml' file found in the current directory.".into());
    }

    let content = fs::read_to_string(config_path)?;
    let config: ProjectConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}
