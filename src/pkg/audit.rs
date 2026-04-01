use crate::pkg::{config, types};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AuditLogLine {
    #[serde(flatten)]
    entry: AuditEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prev_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuditVerification {
    pub valid: bool,
    pub total_entries: usize,
    pub hashed_entries: usize,
    pub legacy_entries: usize,
    pub message: String,
}

fn get_audit_log_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let zoi_dir = home_dir.join(".zoi");
    if !zoi_dir.exists() {
        fs::create_dir_all(&zoi_dir)?;
    }
    Ok(zoi_dir.join("audit.json"))
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

fn calculate_entry_hash(entry: &AuditEntry, prev_hash: Option<&str>) -> Result<String> {
    #[derive(Serialize)]
    struct HashPayload<'a> {
        entry: &'a AuditEntry,
        prev_hash: Option<&'a str>,
    }

    let payload = HashPayload { entry, prev_hash };
    let json = serde_json::to_string(&payload)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

fn get_last_hashed_entry(log_path: &PathBuf) -> Result<Option<AuditLogLine>> {
    if !log_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(log_path)?;
    for line in content.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: AuditLogLine = serde_json::from_str(line)?;
        if parsed.hash.is_some() {
            return Ok(Some(parsed));
        }
    }

    Ok(None)
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
    let prev_hash = get_last_hashed_entry(&log_path)?.and_then(|line| line.hash);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let hash = Some(calculate_entry_hash(&entry, prev_hash.as_deref())?);
    let line = AuditLogLine {
        entry,
        prev_hash,
        hash,
    };
    let json = serde_json::to_string(&line)?;
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
            let parsed: AuditLogLine = serde_json::from_str(line)?;
            entries.push(parsed.entry);
        }
    }
    Ok(entries)
}

pub fn export_history(export_path: &Path, ndjson: bool) -> Result<usize> {
    let log_path = get_audit_log_path()?;
    if !log_path.exists() {
        return Err(anyhow!(
            "No history recorded. Audit logging might be disabled."
        ));
    }

    let content = fs::read_to_string(log_path)?;
    let raw_lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if raw_lines.is_empty() {
        return Err(anyhow!(
            "No history recorded. Audit logging might be disabled."
        ));
    }

    if let Some(parent) = export_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    if ndjson {
        fs::write(export_path, format!("{}\n", raw_lines.join("\n")))?;
        return Ok(raw_lines.len());
    }

    let mut entries = Vec::new();
    for (index, line) in raw_lines.iter().enumerate() {
        let parsed: AuditLogLine = serde_json::from_str(line)
            .map_err(|e| anyhow!("Invalid audit log JSON at line {}: {}", index + 1, e))?;
        entries.push(parsed);
    }

    let json = serde_json::to_string_pretty(&entries)?;
    fs::write(export_path, json)?;
    Ok(entries.len())
}

pub fn verify_chain() -> Result<AuditVerification> {
    let log_path = get_audit_log_path()?;
    if !log_path.exists() {
        return Ok(AuditVerification {
            valid: true,
            total_entries: 0,
            hashed_entries: 0,
            legacy_entries: 0,
            message: "No audit history found.".to_string(),
        });
    }

    let content = fs::read_to_string(log_path)?;
    let mut total_entries = 0usize;
    let mut hashed_entries = 0usize;
    let mut legacy_entries = 0usize;
    let mut previous_hash: Option<String> = None;
    let mut seen_hashed = false;

    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        total_entries += 1;

        let parsed: AuditLogLine = serde_json::from_str(line)
            .map_err(|e| anyhow!("Invalid audit log JSON at line {}: {}", index + 1, e))?;

        if let Some(stored_hash) = parsed.hash.as_deref() {
            seen_hashed = true;
            hashed_entries += 1;

            if parsed.prev_hash != previous_hash {
                return Ok(AuditVerification {
                    valid: false,
                    total_entries,
                    hashed_entries,
                    legacy_entries,
                    message: format!(
                        "Audit hash chain is broken at line {} (prev_hash mismatch).",
                        index + 1
                    ),
                });
            }

            let expected_hash = calculate_entry_hash(&parsed.entry, parsed.prev_hash.as_deref())?;
            if stored_hash != expected_hash {
                return Ok(AuditVerification {
                    valid: false,
                    total_entries,
                    hashed_entries,
                    legacy_entries,
                    message: format!(
                        "Audit hash mismatch at line {} (entry appears modified).",
                        index + 1
                    ),
                });
            }

            previous_hash = Some(stored_hash.to_string());
        } else {
            legacy_entries += 1;
            if seen_hashed {
                return Ok(AuditVerification {
                    valid: false,
                    total_entries,
                    hashed_entries,
                    legacy_entries,
                    message: format!(
                        "Legacy audit entry detected after chained entries at line {}.",
                        index + 1
                    ),
                });
            }
        }
    }

    let message = if hashed_entries == 0 && legacy_entries > 0 {
        "Audit log is valid but uses legacy non-chained entries.".to_string()
    } else {
        "Audit hash chain is valid.".to_string()
    };

    Ok(AuditVerification {
        valid: true,
        total_entries,
        hashed_entries,
        legacy_entries,
        message,
    })
}
