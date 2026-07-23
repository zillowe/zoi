use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use zoi_core::{config, types, utils};

/// Manages Zoi's tamper-evident audit log.
///
/// The audit log records all state-changing operations (Install, Uninstall, Upgrade).
/// It uses a "Hash Chain" mechanism where each new entry contains a SHA-256 hash
/// of its contents PLUS the hash of the previous entry. This makes it
/// mathematically impossible to modify or delete a historical entry without
/// breaking the chain.

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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AuditLog {
    pub version: String,
    pub entries: Vec<AuditLogLine>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditLogLine {
    #[serde(flatten)]
    pub entry: AuditEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
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
    let home_dir =
        utils::get_user_home().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let zoi_dir = zoi_core::sysroot::apply_sysroot(home_dir.join(".zoi"));
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

fn read_audit_log() -> Result<AuditLog> {
    let path = get_audit_log_path()?;
    if !path.exists() {
        return Ok(AuditLog {
            version: env!("CARGO_PKG_VERSION").to_string(),
            entries: Vec::new(),
        });
    }

    let content = fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(AuditLog {
            version: env!("CARGO_PKG_VERSION").to_string(),
            entries: Vec::new(),
        });
    }

    if let Ok(log) = serde_json::from_str::<AuditLog>(&content) {
        return Ok(log);
    }

    let mut entries = Vec::new();
    for line in content.lines() {
        if !line.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<AuditLogLine>(line)
        {
            entries.push(parsed);
        }
    }

    if !entries.is_empty() {
        return Ok(AuditLog {
            version: env!("CARGO_PKG_VERSION").to_string(),
            entries,
        });
    }

    Ok(AuditLog {
        version: env!("CARGO_PKG_VERSION").to_string(),
        entries: Vec::new(),
    })
}

fn write_audit_log(log: &AuditLog) -> Result<()> {
    let path = get_audit_log_path()?;
    let content = serde_json::to_string_pretty(log)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn log_event(action: AuditAction, manifest: &types::InstallManifest) -> Result<()> {
    let config = config::read_config()?;
    if !config.audit_log_enabled {
        return Ok(());
    }

    let mut log = read_audit_log()?;
    let prev_hash = log.entries.last().and_then(|l| l.hash.clone());

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

    let hash = Some(calculate_entry_hash(&entry, prev_hash.as_deref())?);
    log.entries.push(AuditLogLine {
        entry,
        prev_hash,
        hash,
    });
    log.version = env!("CARGO_PKG_VERSION").to_string();

    write_audit_log(&log)?;
    Ok(())
}

pub fn get_history() -> Result<Vec<AuditEntry>> {
    let log = read_audit_log()?;
    Ok(log.entries.into_iter().map(|l| l.entry).collect())
}

pub fn export_history(export_path: &Path, ndjson: bool) -> Result<usize> {
    let log = read_audit_log()?;
    if log.entries.is_empty() {
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
        let mut content = String::new();
        for entry in &log.entries {
            content.push_str(&serde_json::to_string(entry)?);
            content.push('\n');
        }
        fs::write(export_path, content)?;
    } else {
        let json = serde_json::to_string_pretty(&log.entries)?;
        fs::write(export_path, json)?;
    }

    Ok(log.entries.len())
}

pub fn verify_chain() -> Result<AuditVerification> {
    let log = read_audit_log()?;
    let mut total_entries = 0usize;
    let mut hashed_entries = 0usize;
    let mut legacy_entries = 0usize;
    let mut previous_hash: Option<String> = None;
    let mut seen_hashed = false;

    for (index, parsed) in log.entries.iter().enumerate() {
        total_entries += 1;

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
                        "Audit hash chain is broken at entry {} (prev_hash mismatch).",
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
                        "Audit hash mismatch at entry {} (entry appears modified).",
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
                        "Legacy audit entry detected after chained entries at entry {}.",
                        index + 1
                    ),
                });
            }
        }
    }

    let message = if total_entries == 0 {
        "No audit history found.".to_string()
    } else if hashed_entries == 0 && legacy_entries > 0 {
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
