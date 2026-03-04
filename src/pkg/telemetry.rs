use serde::Serialize;
use std::{error::Error, fs};

#[derive(Debug, Serialize)]
pub struct PackageEvent<'a> {
    pub client_id: &'a str,
    pub event: &'a str,
    pub ts: String,
    pub app_version: &'a str,
    pub os: &'a str,
    pub arch: &'a str,
    pub distro: Option<String>,
    pub shell: Option<String>,
    pub package: MinimalPackage<'a>,
    pub package_type: &'a str,
    pub scope: String,
    pub reason: String,
    pub install_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MinimalPackage<'a> {
    pub name: &'a str,
    pub sub_package: Option<&'a String>,
    pub repo: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    pub license: &'a str,
    pub maintainer: MinimalPerson<'a>,
    pub author: Option<MinimalPerson<'a>>,
    pub registry: &'a str,
    pub registry_url: &'a str,
}

#[derive(Debug, Serialize)]
pub struct MinimalPerson<'a> {
    pub name: &'a str,
    pub email: &'a str,
    pub website: Option<&'a String>,
}

fn get_client_id_path() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let home = home::home_dir().ok_or("Could not find home directory")?;
    Ok(home.join(".zoi").join("telemetry").join("client_id"))
}

pub fn get_anonymous_id() -> String {
    ensure_client_id().unwrap_or_else(|_| "unknown".to_string())
}

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
            use uuid::Timestamp;
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

pub fn posthog_capture_event(
    event_name: &str,
    pkg: &crate::pkg::types::Package,
    app_version: &str,
    registry_handle: &str,
    install_type: Option<&str>,
) -> Result<bool, Box<dyn Error>> {
    let config = crate::pkg::config::read_config()?;
    if !config.telemetry_enabled {
        return Ok(false);
    }

    let client_id = ensure_client_id()?;

    let platform = crate::utils::get_platform().unwrap_or_else(|_| "unknown-unknown".into());
    let mut parts = platform.split('-');
    let os = parts.next().unwrap_or("unknown");
    let arch = parts.next().unwrap_or("unknown");
    let distro = crate::utils::get_linux_distribution();
    let shell = crate::utils::get_current_shell().map(|s| s.to_string());

    let package_type_str = match pkg.package_type {
        crate::pkg::types::PackageType::Package => "Package",
        crate::pkg::types::PackageType::Collection => "Collection",
        crate::pkg::types::PackageType::App => "App",
        crate::pkg::types::PackageType::Extension => "Extension",
    };

    let scope_str = format!("{:?}", pkg.scope).to_lowercase();
    let reason_str = match &pkg.reason {
        Some(crate::pkg::types::InstallReason::Direct) => "direct".to_string(),
        Some(crate::pkg::types::InstallReason::Dependency { parent }) => {
            format!("dependency:{}", parent)
        }
        Some(crate::pkg::types::InstallReason::Declarative) => "declarative".to_string(),
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
