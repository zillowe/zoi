use crate::pkg::types;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub website: Option<String>,
    pub license: String,
    pub git: String,
    pub man_url: Option<String>,
    pub maintainer: types::Maintainer,
    pub author: Option<types::Author>,
    pub installation: ResolvedInstallation,
    #[serde(default)]
    pub bins: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ResolvedInstallation {
    pub install_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_commands: Option<std::collections::HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assets: Vec<PlatformAsset>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlatformAsset {
    pub platform: String,
    pub url: String,
    pub checksum: Option<String>,
    pub signature_url: Option<String>,
}
