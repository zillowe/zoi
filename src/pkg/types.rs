use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    System,
}

impl Default for Scope {
    fn default() -> Self {
        Scope::User
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Package,
    Collection,
    Service,
    Config,
}

impl Default for PackageType {
    fn default() -> Self {
        PackageType::Package
    }
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ServiceMethod {
    pub platforms: Vec<String>,
    pub start: Vec<String>,
    pub stop: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ConfigCommands {
    pub platforms: Vec<String>,
    pub install: Vec<String>,
    pub uninstall: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub repo: String,
    pub version: Option<String>,
    pub versions: Option<HashMap<String, String>>,
    pub description: String,
    pub website: Option<String>,
    #[serde(default)]
    pub git: String,
    pub maintainer: Maintainer,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub installation: Vec<InstallationMethod>,
    pub dependencies: Option<Dependencies>,
    pub updater: Option<String>,
    #[serde(rename = "type", default)]
    pub package_type: PackageType,
    pub alt: Option<String>,
    #[serde(default)]
    pub scope: Scope,
    pub service: Option<Vec<ServiceMethod>>,
    pub config: Option<Vec<ConfigCommands>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Maintainer {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Checksums {
    Url(String),
    List(Vec<ChecksumInfo>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChecksumInfo {
    pub file: String,
    pub checksum: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct InstallationMethod {
    #[serde(rename = "type")]
    pub install_type: String,
    pub url: String,
    pub platforms: Vec<String>,
    pub commands: Option<Vec<String>>,
    #[serde(rename = "platformComExt")]
    pub platform_com_ext: Option<HashMap<String, String>>,
    pub checksums: Option<Checksums>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Dependencies {
    pub runtime: Option<Vec<String>>,
    pub build: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallReason {
    Direct,
    Dependency,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallManifest {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub installed_at: String,
    pub reason: InstallReason,
    pub scope: Scope,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub repos: Vec<String>,
}
