use crate::pkg::{config, types};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AuditAction {
    Install,
    Uninstall,
    Upgrade,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub action: AuditAction,
    pub package_name: String,
    pub version: String,
    pub repo: String,
    pub package_type: types::PackageType,
    pub scope: types::Scope,
    pub registry: String,
}

fn get_audit_log_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let zoi_dir = home_dir.join(".zoi");
    if !zoi_dir.exists() {
        fs::create_dir_all(&zoi_dir)?;
    }
    Ok(zoi_dir.join("audit.jsonl"))
}

fn get_username() -> String {
    #[cfg(unix)]
    {
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(windows)]
    {
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
    }
}

pub fn log_event(action: AuditAction, manifest: &types::InstallManifest) -> Result<()> {
    let config = config::read_config()?;
    if !config.audit_log_enabled {
        return Ok(());
    }

    let user = get_username();
    let entry = AuditEntry {
        timestamp: Utc::now(),
        user,
        action,
        package_name: manifest.name.clone(),
        version: manifest.version.clone(),
        repo: manifest.repo.clone(),
        package_type: manifest.package_type,
        scope: manifest.scope,
        registry: manifest.registry_handle.clone(),
    };

    let log_path = get_audit_log_path()?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let json = serde_json::to_string(&entry)?;
    writeln!(file, "{}", json)?;

    Ok(())
}

pub fn get_history() -> Result<Vec<AuditEntry>> {
    let log_path = get_audit_log_path()?;
    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(log_path)?;
    let mut entries = Vec::new();
    for line in content.lines() {
        if !line.trim().is_empty() {
            let entry: AuditEntry = serde_json::from_str(line)?;
            entries.push(entry);
        }
    }
    Ok(entries)
}
