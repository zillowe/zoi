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

#[derive(Serialize, Deserialize, Debug)]
pub struct ResolvedInstallation {
    pub install_type: String,
    pub binary_path: Option<String>,
    pub assets: Vec<PlatformAsset>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlatformAsset {
    pub platform: String,
    pub url: String,
    pub checksum: Option<String>,
    pub signature_url: Option<String>,
}
