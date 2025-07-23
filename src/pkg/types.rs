use serde::{Deserialize, Deserializer, Serialize};
use serde_json;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Package {
    pub name: String,
    pub repo: String,
    #[serde(deserialize_with = "deserialize_version")]
    pub version: String,
    pub description: String,
    pub website: String,
    pub git: String,
    pub maintainer: Maintainer,
    pub license: String,
    pub installation: Vec<InstallationMethod>,
    pub dependencies: Option<Dependencies>,
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    if s.starts_with("http") {
        let resp = reqwest::blocking::get(&s)
            .map_err(serde::de::Error::custom)?
            .text()
            .map_err(serde::de::Error::custom)?;
        let json: serde_json::Value =
            serde_json::from_str(&resp).map_err(serde::de::Error::custom)?;
        if let Some(tag) = json
            .get("latest")
            .and_then(|l| l.get("production"))
            .and_then(|p| p.get("tag"))
            .and_then(|t| t.as_str())
        {
            Ok(tag.to_string())
        } else {
            Err(serde::de::Error::custom(
                "Failed to extract version from JSON URL",
            ))
        }
    } else {
        Ok(s)
    }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub repos: Vec<String>,
}
