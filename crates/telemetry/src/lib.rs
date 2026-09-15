//! Anonymous telemetry for Zoi.
//!
//! This crate handles the collection and transmission of anonymous usage
//! statistics to help improve Zoi. It ensures privacy by only collecting
//! non-identifiable data and requiring explicit user opt-in.
//!
//! Backends:
//! - `PostHog` via the official `posthog-rs` Rust SDK (blocking client) for
//!   success-only analytics. Every event carries the anonymous client ID as
//!   `distinct_id`, so DAU/WAU/MAU fall out of any event stream.
//! - Sentry via the official `sentry` Rust SDK (with network transport) for
//!   error tracking: panics via the `panic` integration plus explicit
//!   `capture_error` calls on command failures.
//! - Local crash reports in `$XDG_STATE_HOME/zoi/crash/*.zoicrash` (Sentry
//!   envelope format), see [`crash`]. These are the reliable crash channel:
//!   they are written synchronously by the panic hook even under `panic =
//!   "abort"`, where in-memory SDK events may never flush.

use std::error::Error;
use std::fs;

use serde::Serialize;
use uuid::Timestamp;

pub mod crash;

/// Represents an anonymous telemetry event sent to `PostHog`.
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
    pub install_type: Option<String>
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
    pub registry_url: &'a str
}

/// A minimal representation of a person (maintainer or author) for telemetry.
#[derive(Debug, Serialize)]
pub struct MinimalPerson<'a> {
    /// Name of the person.
    pub name: &'a str,
    /// Email address of the person.
    pub email: &'a str,
    /// Optional website URL.
    pub website: Option<&'a String>
}

/// Returns the path to the file where the anonymous client ID is stored.
fn get_client_id_path() -> Result<std::path::PathBuf, Box<dyn Error>> {
    Ok(zoi_core::utils::get_user_state_dir()?
        .join("telemetry")
        .join("client_id"))
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
                chrono::Utc::now().timestamp_millis().cast_unsigned(),
                0
            );
            uuid::Uuid::new_v7(ts).to_string()
        };
        fs::write(&path, &id)?;
        Ok(id)
    }
}

/// Returns `true` when the user has opted-in to telemetry.
pub fn telemetry_enabled() -> bool {
    zoi_core::config::read_config().is_ok_and(|c| c.telemetry_enabled)
}

/// Resolves the `PostHog` project API key.
///
/// Precedence: runtime `POSTHOG_API_KEY` env, then build-time
/// `option_env!("POSTHOG_API_KEY")`.
pub fn resolve_posthog_key() -> String {
    std::env::var("POSTHOG_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            option_env!("POSTHOG_API_KEY")
                .unwrap_or_default()
                .to_string()
        })
}

/// Resolves the `PostHog` ingest host.
///
/// Precedence: runtime `POSTHOG_API_HOST` env, then build-time
/// `option_env!("POSTHOG_API_HOST")`, then the EU default.
pub fn resolve_posthog_host() -> String {
    std::env::var("POSTHOG_API_HOST")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            option_env!("POSTHOG_API_HOST")
                .unwrap_or("https://eu.i.posthog.com")
                .to_string()
        })
}

/// Resolves the Sentry DSN.
///
/// Precedence: runtime `SENTRY_DSN` env, then build-time
/// `option_env!("SENTRY_DSN")`. Returns `None` when unset.
pub fn resolve_sentry_dsn() -> Option<String> {
    std::env::var("SENTRY_DSN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            option_env!("SENTRY_DSN").map(std::string::ToString::to_string)
        })
        .filter(|v| !v.trim().is_empty())
}

/// `SENTRY_ENVIRONMENT` is a free-form tag attached to every Sentry
/// event/envelope (e.g. `production`, `development`, `ci`).
pub fn resolve_environment() -> String {
    if let Ok(v) = std::env::var("SENTRY_ENVIRONMENT")
        && !v.trim().is_empty()
    {
        return v;
    }
    if let Some(v) = option_env!("SENTRY_ENVIRONMENT")
        && !v.trim().is_empty()
    {
        return v.to_string();
    }
    if std::env::var("GITLAB_CI").is_ok() || std::env::var("CI").is_ok() {
        return "ci".to_string();
    }
    if cfg!(debug_assertions) {
        "development".to_string()
    } else {
        "production".to_string()
    }
}

/// Returns the release string attached to telemetry events.
pub fn app_release() -> String {
    option_env!("ZOI_COMMIT_HASH").map_or_else(
        || format!("zoi@{}", env!("CARGO_PKG_VERSION")),
        |hash| format!("zoi@{}+{}", env!("CARGO_PKG_VERSION"), hash)
    )
}

/// Builds a `posthog-rs` blocking client, or `None` when telemetry must stay
/// silent (opted-out, offline, missing API key, or running as `zoi-mini`,
/// which is never tracked).
fn posthog_client() -> Option<posthog_rs::Client> {
    if !telemetry_enabled()
        || zoi_core::offline::is_offline()
        || zoi_core::utils::is_mini_mode()
    {
        return None;
    }
    let key = resolve_posthog_key();
    if key.trim().is_empty() {
        return None;
    }
    let host = resolve_posthog_host();
    let options = posthog_rs::ClientOptionsBuilder::default()
        .api_key(key)
        .host(host)
        .request_timeout_seconds(4)
        .flush_at(1)
        .shutdown_timeout_ms(4000)
        .disable_geoip(true)
        .build()
        .ok()?;
    Some(posthog_rs::client(options))
}

/// Initializes Sentry for this process when telemetry is opted-in.
///
/// Returns a guard that must be kept alive for the process lifetime (dropping
/// it flushes queued events). Returns `None` when opted-out, offline, when no
/// DSN is configured, or when running as `zoi-mini`, which is never tracked.
/// `send_default_pii` is always `false` to honor the no-IP privacy guarantee.
///
/// Release health is enabled: each CLI invocation is tracked as one
/// `Application` session linked to the release, so Sentry reports adoption,
/// crash-free users, and crash-free sessions per release. Note the release
/// name is the custom [`app_release`] value (`zoi@<version>+<commit>`, which
/// the `sentry:release` CI job reproduces), not `sentry::release_name!`,
/// which would resolve to this crate's name without the commit hash.
pub fn init_sentry() -> Option<sentry::ClientInitGuard> {
    if !telemetry_enabled()
        || zoi_core::offline::is_offline()
        || zoi_core::utils::is_mini_mode()
    {
        return None;
    }
    let dsn = resolve_sentry_dsn().filter(|s| !s.is_empty())?;
    if dsn.parse::<sentry::types::Dsn>().is_err() {
        return None;
    }
    let guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(dsn.as_str())
            .environment(resolve_environment())
            .release(app_release())
            .auto_session_tracking(true)
            .session_mode(sentry::SessionMode::Application)
    );
    Some(guard)
}

/// Guards held for the process lifetime by [`init_telemetry`].
pub struct TelemetryGuards {
    /// Sentry guard, present only when opted-in with a DSN.
    pub sentry_guard: Option<sentry::ClientInitGuard>
}

/// Installs crash handling and initializes opt-in backends.
///
/// Behavior:
/// - Always installs the local panic hook so crashes are saved to
///   `$XDG_STATE_HOME/zoi/crash/*.zoicrash` (no network, no opt-in needed).
/// - Initializes Sentry only when opted-in.
/// - When `prompt_pending` is set, the user is opted-in, and the process runs
///   interactively with pending crash files, asks the user whether to upload
///   them now. Otherwise files stay local and the user is never prompted.
///   Callers should pass `false` for telemetry-management commands (e.g. `zoi
///   telemetry disable`), so opting out is never gated behind an upload prompt
///   for potentially sensitive reports.
pub fn init_telemetry(prompt_pending: bool) -> TelemetryGuards {
    crash::install_panic_hook();
    let sentry_guard = init_sentry();
    if prompt_pending && telemetry_enabled() && sentry_guard.is_some() {
        prompt_pending_crashes();
    }
    TelemetryGuards { sentry_guard }
}

/// Prompts to upload pending crash reports (opt-in only, TTY only).
fn prompt_pending_crashes() {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return;
    }
    let pending = crash::list_crashes();
    if pending.is_empty() {
        return;
    }
    eprintln!(
        "Found {} unsent crash report(s) in $XDG_STATE_HOME/zoi/crash.",
        pending.len()
    );
    for path in &pending {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        if zoi_core::utils::ask_for_confirmation(
            &format!("Upload crash report {name} to Sentry?"),
            false
        ) {
            match crash::send_crash_report(path) {
                Ok(true) => {
                    eprintln!("Uploaded {name}. Removing local copy.");
                    let _ = fs::remove_file(path);
                }
                Ok(false) => {}
                Err(e) => eprintln!("Failed to upload {name}: {e}")
            }
        }
    }
}

/// Securely captures an anonymous event and sends it via the `posthog-rs` SDK.
///
/// Privacy Guarantee:
/// - No IP addresses, hostnames, or personal data are ever collected
///   (`disable_geoip(true)` is set on the SDK client).
/// - The `client_id` is a randomly generated UUID v7 stored in the Zoi user
///   state directory under `telemetry/client_id`.
/// - Telemetry is strictly opt-in. This function returns `Ok(false)`
///   immediately if `telemetry_enabled` is not set to `true` in the user's
///   config, or when offline, or when no API key is configured.
/// - `zoi-mini` is never tracked: this function returns `Ok(false)` for it even
///   when telemetry is enabled. (`zoid` follows the user's opt-in like the main
///   CLI.)
///
/// Data collected is limited to: event type (install/uninstall), package
/// metadata (name, version, license), and basic environment info (OS, Arch,
/// Shell).
///
/// # Errors
///
/// Returns an error if:
/// - The Zoi configuration cannot be read.
/// - The anonymous client ID cannot be ensured.
/// - The `posthog-rs` client cannot be built or delivery fails.
pub fn posthog_capture_event(
    event_name: &str,
    pkg: &zoi_core::types::Package,
    app_version: &str,
    registry_handle: &str,
    install_type: Option<&str>
) -> Result<bool, Box<dyn Error>> {
    let config = zoi_core::config::read_config()?;
    if !config.telemetry_enabled {
        return Ok(false);
    }
    // `zoi-mini` (zero-sync helper) is never tracked, even when the user
    // opted-in on the main CLI. (`zoid` follows the user's opt-in.)
    if zoi_core::utils::is_mini_mode() {
        return Ok(false);
    }

    let client_id = ensure_client_id()?;

    let platform = zoi_core::utils::get_platform()
        .unwrap_or_else(|_| "unknown-unknown".into());
    let mut parts = platform.split('-');
    let os = parts.next().unwrap_or("unknown");
    let arch = parts.next().unwrap_or("unknown");
    let distro = zoi_core::utils::get_linux_distribution();
    let shell = zoi_core::utils::get_current_shell().map(|s| s.to_string());

    let package_type_str = match pkg.package_type {
        zoi_core::types::PackageType::Package => "Package",
        zoi_core::types::PackageType::Collection => "Collection",
        zoi_core::types::PackageType::App => "App",
        zoi_core::types::PackageType::Extension => "Extension"
    };

    let scope_str = format!("{:?}", pkg.scope).to_lowercase();
    let reason_str = match &pkg.reason {
        Some(zoi_core::types::InstallReason::Direct) => "direct".to_string(),
        Some(zoi_core::types::InstallReason::Dependency { parent }) => {
            format!("dependency:{parent}")
        }
        None => "unknown".to_string()
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
                website: pkg.maintainer.website.as_ref()
            },
            author: pkg.author.as_ref().map(|a| MinimalPerson {
                name: &a.name,
                email: a.email.as_deref().unwrap_or_default(),
                website: a.website.as_ref()
            }),
            registry: registry_handle,
            registry_url
        },
        package_type: package_type_str,
        scope: scope_str,
        reason: reason_str,
        install_type: install_type.map(std::string::ToString::to_string)
    };

    let Some(client) = posthog_client() else {
        if resolve_posthog_key().trim().is_empty() {
            return Err(
                "Telemetry enabled but POSTHOG_API_KEY is not set".into()
            );
        }
        return Ok(false);
    };

    let props = serde_json::to_value(&ev)
        .map_err(|e| format!("Failed to serialize telemetry event: {e}"))?;
    let props_map = props.as_object().cloned().unwrap_or_default();
    let mut event = posthog_rs::Event::new(event_name, client_id.as_str());
    for (key, value) in props_map {
        let _ = event.insert_prop(key, value);
    }

    client
        .capture_immediate(event)
        .map_err(|e| format!("PostHog delivery failed: {e}"))?;
    Ok(true)
}

/// A successful CLI command invocation, sent to `PostHog` for analytics.
///
/// `PostHog` derives DAU/WAU/MAU from the `distinct_id` (the anonymous
/// client ID) present on every event, so no separate active-user ping is
/// needed: any command event marks the client active for that period.
#[derive(Debug)]
pub struct CommandEvent {
    /// Subcommand name (e.g. `"install"`, `"update"`, `"exec"`).
    pub command: String,
    /// Wall-clock execution time in milliseconds.
    pub duration_ms: u128
}

/// Collects the environment properties shared by analytics events.
fn environment_props(
    app_version: &str
) -> serde_json::Map<String, serde_json::Value> {
    let platform = zoi_core::utils::get_platform()
        .unwrap_or_else(|_| "unknown-unknown".into());
    let mut parts = platform.split('-');
    let os = parts.next().unwrap_or("unknown");
    let arch = parts.next().unwrap_or("unknown");
    let mut map = serde_json::Map::new();
    map.insert("app_version".into(), app_version.into());
    map.insert("os".into(), os.into());
    map.insert("arch".into(), arch.into());
    map.insert(
        "distro".into(),
        zoi_core::utils::get_linux_distribution().into()
    );
    map.insert(
        "shell".into(),
        zoi_core::utils::get_current_shell()
            .map(|s| s.to_string())
            .into()
    );
    map.insert("environment".into(), resolve_environment().into());
    map
}

/// Captures a successful command run in `PostHog`.
///
/// Success-only by design: failures go to Sentry via
/// [`sentry_capture_error`] instead, keeping analytics (usage, DAU/WAU/MAU)
/// separate from error tracking.
///
/// Returns `Ok(false)` without touching the network when telemetry is
/// opted-out, offline, unconfigured, or running as `zoi-mini`.
///
/// # Errors
///
/// Returns an error when telemetry is enabled but misconfigured (e.g. no API
/// key) or when delivery fails.
pub fn posthog_capture_command(
    event: &CommandEvent
) -> Result<bool, Box<dyn Error>> {
    let config = zoi_core::config::read_config()?;
    if !config.telemetry_enabled {
        return Ok(false);
    }
    if zoi_core::utils::is_mini_mode() || zoi_core::offline::is_offline() {
        return Ok(false);
    }

    let client_id = ensure_client_id()?;
    let Some(client) = posthog_client() else {
        if resolve_posthog_key().trim().is_empty() {
            return Err(
                "Telemetry enabled but POSTHOG_API_KEY is not set".into()
            );
        }
        return Ok(false);
    };

    let mut props = environment_props(env!("CARGO_PKG_VERSION"));
    props.insert("command".into(), event.command.clone().into());
    props.insert(
        "duration_ms".into(),
        u64::try_from(event.duration_ms).unwrap_or(u64::MAX).into()
    );

    let mut ph_event = posthog_rs::Event::new("command", client_id.as_str());
    for (key, value) in props {
        let _ = ph_event.insert_prop(key, value);
    }
    client
        .capture_immediate(ph_event)
        .map_err(|e| format!("PostHog delivery failed: {e}"))?;
    Ok(true)
}

/// Reports a command failure to Sentry for error tracking.
///
/// This is a no-op when Sentry is not initialized (opted-out, offline,
/// unconfigured, or `zoi-mini`). The full error chain is attached, so error
/// strings may contain local paths - never call this with user secrets.
///
/// Note: delivery goes through the SDK's background transport thread. On the
/// normal return path the [`TelemetryGuards`] drop flushes it, but code that
/// ends the process via `std::process::exit` must call [`shutdown_sentry`]
/// first, otherwise queued events are lost.
pub fn sentry_capture_error(command: &str, error: &dyn Error) {
    if !telemetry_enabled()
        || zoi_core::offline::is_offline()
        || zoi_core::utils::is_mini_mode()
    {
        return;
    }
    sentry::configure_scope(|scope| {
        scope.set_tag("command", command);
    });
    let mut error_chain = format!("{error}");
    let mut source = error.source();
    while let Some(cause) = source {
        use std::fmt::Write as _;
        let _ = writeln!(error_chain, "\nCaused by: {cause}");
        source = cause.source();
    }
    sentry::capture_message(&error_chain, sentry::Level::Error);
}

/// Drains queued Sentry events and shuts down the transport.
///
/// Must be called before any `std::process::exit`, which skips destructors
/// and would otherwise abandon events still in the SDK's background queue
/// (notably the failure report from [`sentry_capture_error`]). This also
/// flushes the release-health session, so the invocation is counted even on
/// the error path. Waits at most 2 seconds so a stalled network can never
/// hang the CLI. Returns `true` when the queue drained in time, `false` on
/// timeout or when Sentry was never initialized.
pub fn shutdown_sentry() -> bool {
    sentry::Hub::current().client().is_some_and(|client| {
        client.close(Some(std::time::Duration::from_secs(2)))
    })
}
