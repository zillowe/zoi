use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub repo: String,
    pub version: String,
    pub description: String,
    pub website: String,
    pub git: String,
    pub maintainer: Maintainer,
    pub license: String,
    pub installation: Vec<InstallationMethod>,
    pub dependencies: Option<Dependencies>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Maintainer {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct InstallationMethod {
    #[serde(rename = "type")]
    pub install_type: String,
    pub url: String,
    pub platforms: Vec<String>,
    pub commands: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
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
}
