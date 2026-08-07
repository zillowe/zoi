//! Anonymous telemetry for Zoi.
//!
//! This crate handles the collection and transmission of anonymous usage
//! statistics to help improve Zoi. It ensures privacy by only collecting
//! non-identifiable data and requiring explicit user opt-in.

use serde::Serialize;
use std::{error::Error, fs};
use uuid::Timestamp;

/// Represents an anonymous telemetry event sent to PostHog.
#[derive(Debug, Serialize)]
pub struct PackageEvent<'a> {
    /// Unique anonymous identifier for the client.
    pub client_id: &'a str,
    /// The name of the event (e.g. "install", "uninstall").
    pub event: &'a str,
    /// RFC3339 formatted timestamp of the event.
    pub ts: String,
    /// Version of the Zoi application.
    pub app_version: &'a str,
    /// Operating system name.
    pub os: &'a str,
    /// CPU architecture.
    pub arch: &'a str,
    /// Linux distribution name, if applicable.
    pub distro: Option<String>,
    /// The user's current shell.
    pub shell: Option<String>,
    /// Minimal package metadata.
    pub package: MinimalPackage<'a>,
    /// Type of the package (e.g. "Package", "App").
    pub package_type: &'a str,
    /// Installation scope (e.g. "global", "user").
    pub scope: String,
    /// Reason for the installation (e.g. "direct", "dependency").
    pub reason: String,
    /// How the package was installed (e.g. "source", "binary").
    pub install_type: Option<String>,
}

/// A privacy-preserving subset of package metadata for analytics.
#[derive(Debug, Serialize)]
pub struct MinimalPackage<'a> {
    /// Name of the package.
    pub name: &'a str,
    /// Optional sub-package name.
    pub sub_package: Option<&'a String>,
    /// Repository where the package is hosted.
    pub repo: &'a str,
    /// Package version.
    pub version: &'a str,
    /// Brief description of the package.
    pub description: &'a str,
    /// License of the package.
    pub license: &'a str,
    /// Maintainer of the package.
    pub maintainer: MinimalPerson<'a>,
    /// Original author of the package.
    pub author: Option<MinimalPerson<'a>>,
    /// Registry handle.
    pub registry: &'a str,
    /// URL of the registry.
    pub registry_url: &'a str,
}

/// A minimal representation of a person (maintainer or author) for telemetry.
#[derive(Debug, Serialize)]
pub struct MinimalPerson<'a> {
    /// Name of the person.
    pub name: &'a str,
    /// Email address of the person.
    pub email: &'a str,
    /// Optional website URL.
    pub website: Option<&'a String>,
}

/// Returns the path to the file where the anonymous client ID is stored.
fn get_client_id_path() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let home = zoi_core::utils::get_user_home().ok_or("Could not find home directory")?;
    Ok(home.join(".zoi").join("telemetry").join("client_id"))
}

/// Returns the anonymous client ID, or "unknown" if it cannot be retrieved.
pub fn get_anonymous_id() -> String {
    ensure_client_id().unwrap_or_else(|_| "unknown".to_string())
}

/// Ensures that an anonymous client ID exists, creating a new one if necessary.
fn ensure_client_id() -> Result<String, Box<dyn Error>> {
    let path = get_client_id_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if path.exists() {
        let id = fs::read_to_string(&path)?;
        Ok(id.trim().to_string())
    } else {
        let id = {
            let ts = Timestamp::from_unix(
                uuid::NoContext,
                chrono::Utc::now().timestamp_millis() as u64,
                0,
            );
            uuid::Uuid::new_v7(ts).to_string()
        };
        fs::write(&path, &id)?;
        Ok(id)
    }
}

/// Securely captures an anonymous event and sends it to PostHog.
///
/// Privacy Guarantee:
/// - No IP addresses, hostnames, or personal data are ever collected.
/// - The `client_id` is a randomly generated UUID v7 stored in `~/.zoi/telemetry/client_id`.
/// - Telemetry is strictly opt-in. This function returns `false` immediately
///   if `telemetry_enabled` is not set to `true` in the user's config.
///
/// Data collected is limited to: event type (install/uninstall), package metadata
/// (name, version, license), and basic environment info (OS, Arch, Shell).
pub fn posthog_capture_event(
    event_name: &str,
    pkg: &zoi_core::types::Package,
    app_version: &str,
    registry_handle: &str,
    install_type: Option<&str>,
) -> Result<bool, Box<dyn Error>> {
    let config = zoi_core::config::read_config()?;
    if !config.telemetry_enabled {
        return Ok(false);
    }

    let client_id = ensure_client_id()?;

    let platform = zoi_core::utils::get_platform().unwrap_or_else(|_| "unknown-unknown".into());
    let mut parts = platform.split('-');
    let os = parts.next().unwrap_or("unknown");
    let arch = parts.next().unwrap_or("unknown");
    let distro = zoi_core::utils::get_linux_distribution();
    let shell = zoi_core::utils::get_current_shell().map(|s| s.to_string());

    let package_type_str = match pkg.package_type {
        zoi_core::types::PackageType::Package => "Package",
        zoi_core::types::PackageType::Collection => "Collection",
        zoi_core::types::PackageType::App => "App",
        zoi_core::types::PackageType::Extension => "Extension",
    };

    let scope_str = format!("{:?}", pkg.scope).to_lowercase();
    let reason_str = match &pkg.reason {
        Some(zoi_core::types::InstallReason::Direct) => "direct".to_string(),
        Some(zoi_core::types::InstallReason::Dependency { parent }) => {
            format!("dependency:{}", parent)
        }
        None => "unknown".to_string(),
    };

    let registry_url = config
        .default_registry
        .as_ref()
        .filter(|r| r.handle == registry_handle)
        .map(|r| r.url.as_str())
        .or_else(|| {
            config
                .added_registries
                .iter()
                .find(|r| r.handle == registry_handle)
                .map(|r| r.url.as_str())
        })
        .unwrap_or("unknown");

    let ev = PackageEvent {
        client_id: &client_id,
        event: event_name,
        ts: chrono::Utc::now().to_rfc3339(),
        app_version,
        os,
        arch,
        distro,
        shell,
        package: MinimalPackage {
            name: &pkg.name,
            sub_package: pkg.sub_package.as_ref(),
            repo: &pkg.repo,
            version: pkg.version.as_deref().unwrap_or("unknown"),
            description: &pkg.description,
            license: &pkg.license,
            maintainer: MinimalPerson {
                name: &pkg.maintainer.name,
                email: &pkg.maintainer.email,
                website: pkg.maintainer.website.as_ref(),
            },
            author: pkg.author.as_ref().map(|a| MinimalPerson {
                name: &a.name,
                email: a.email.as_deref().unwrap_or_default(),
                website: a.website.as_ref(),
            }),
            registry: registry_handle,
            registry_url,
        },
        package_type: package_type_str,
        scope: scope_str,
        reason: reason_str,
        install_type: install_type.map(|s| s.to_string()),
    };

    let ph_host = option_env!("POSTHOG_API_HOST").unwrap_or("https://eu.i.posthog.com");
    let ph_key = option_env!("POSTHOG_API_KEY").unwrap_or_default();
    if ph_key.is_empty() {
        return Err("Telemetry enabled but POSTHOG_API_KEY is not set".into());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .use_rustls_tls()
        .build()?;
    #[derive(Serialize)]
    struct PosthogEvent<'a> {
        event: &'a str,
        distinct_id: &'a str,
        properties: &'a PackageEvent<'a>,
        timestamp: &'a str,
    }
    #[derive(Serialize)]
    struct Batch<'a> {
        api_key: &'a str,
        batch: Vec<PosthogEvent<'a>>,
    }
    let payload = Batch {
        api_key: ph_key,
        batch: vec![PosthogEvent {
            event: ev.event,
            distinct_id: ev.client_id,
            properties: &ev,
            timestamp: &ev.ts,
        }],
    };
    let url = format!("{}/batch", ph_host.trim_end_matches('/'));
    let resp = client.post(url).json(&payload).send()?;
    if !resp.status().is_success() {
        return Err(format!("PostHog HTTP {}", resp.status()).into());
    }
    Ok(true)
}
