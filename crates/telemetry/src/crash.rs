//! Local-first crash reports in Sentry envelope format.
//!
//! Crash files are always written locally (no network, no opt-in needed) so
//! users own their data. Upload to Sentry only happens when telemetry is
//! opted-in and the user explicitly confirms.
//!
//! Files live in `$XDG_STATE_HOME/zoi/crash` (or the platform equivalent via
//! `zoi_core::utils::get_user_state_dir`) with a `.zoicrash` extension. Each
//! file is a minimal Sentry envelope: one JSON header line, one item-header
//! line, one event-payload line.

use std::fs;
use std::path::PathBuf;

/// File extension for locally stored crash reports.
pub const CRASH_EXTENSION: &str = "zoicrash";

/// Returns the directory where crash reports are stored.
///
/// Creates the directory if it does not exist. Crash reports can contain
/// backtraces and runtime context, so the directory is restricted to the
/// owner on Unix (repairing permissions of pre-existing directories too).
fn crash_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = zoi_core::utils::get_user_state_dir()?.join("crash");
    fs::create_dir_all(&dir)?;
    restrict_dir(&dir)?;
    Ok(dir)
}

/// Restricts a directory to owner-only access on Unix.
#[cfg(unix)]
fn restrict_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Non-Unix platforms manage access via ACLs; nothing to do.
#[cfg(not(unix))]
fn restrict_dir(_dir: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Writes bytes to a new file with owner-only access.
///
/// On Unix the file is created with mode `0o600` from the start, so report
/// contents are never briefly world-readable under a permissive umask.
/// `create_new` also refuses to clobber an existing file.
#[cfg(unix)]
fn write_restricted(
    path: &std::path::Path,
    contents: &[u8]
) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents)
}

/// Non-Unix platforms manage access via ACLs; plain write is equivalent.
#[cfg(not(unix))]
fn write_restricted(
    path: &std::path::Path,
    contents: &[u8]
) -> std::io::Result<()> {
    fs::write(path, contents)
}

/// Lists locally stored crash reports, newest first.
pub fn list_crashes() -> Vec<PathBuf> {
    let dir = zoi_core::utils::get_user_state_dir()
        .map(|d| d.join("crash"))
        .unwrap_or_default();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str())
                        == Some(CRASH_EXTENSION)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files.reverse();
    files
}

/// A single parsed stack frame for the Sentry envelope.
#[derive(Debug, PartialEq, Eq)]
struct ParsedFrame {
    /// Symbol name, e.g. `zoi_telemetry::crash::write_crash_report`.
    function: String,
    /// Source file, when the backtrace line provides one.
    filename: Option<String>,
    /// Source line number, when provided.
    lineno: Option<u32>,
    /// Source column number, when provided.
    colno: Option<u32>
}

/// Parses `std::backtrace::Backtrace` text into structured frames.
///
/// Handles both short (`   0: func` / `             at path:line:col`) and
/// full (`stack backtrace:` header with addresses) formats. Headers and other
/// non-frame lines are skipped; a frame whose location won't parse keeps its
/// symbol name with no file info, so nothing is silently dropped. Frames are
/// returned oldest-first, per Sentry's frame ordering convention.
fn parse_backtrace_frames(backtrace: &str) -> Vec<ParsedFrame> {
    let mut frames: Vec<ParsedFrame> = Vec::new();
    let mut lines = backtrace.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some((index, function)) = trimmed.split_once(':') else {
            continue;
        };
        if index.trim().parse::<u32>().is_err() || function.trim().is_empty() {
            continue;
        }
        let mut frame = ParsedFrame {
            function: function.trim().to_string(),
            filename: None,
            lineno: None,
            colno: None
        };
        if let Some(next) = lines.peek().map(|l| l.trim_start())
            && let Some(location) = next.strip_prefix("at ")
        {
            let (filename, lineno, colno) = parse_frame_location(location);
            frame.filename = filename;
            frame.lineno = lineno;
            frame.colno = colno;
            lines.next();
        }
        frames.push(frame);
    }
    frames.reverse();
    frames
}

/// Splits a `path:line:col` frame location from the right, tolerating colons
/// inside the path itself. Returns `(filename, lineno, colno)`, with numbers
/// left as `None` when they don't parse.
fn parse_frame_location(
    location: &str
) -> (Option<String>, Option<u32>, Option<u32>) {
    let mut parts = location.rsplit(':');
    let (Some(col), Some(line)) = (parts.next(), parts.next()) else {
        return (Some(location.to_string()), None, None);
    };
    let (Ok(colno), Ok(lineno)) = (col.parse(), line.parse()) else {
        return (Some(location.to_string()), None, None);
    };
    let filename: String = parts
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(":");
    if filename.is_empty() {
        return (None, Some(lineno), Some(colno));
    }
    (Some(filename), Some(lineno), Some(colno))
}

/// Converts parsed frames to Sentry envelope frame objects.
///
/// Frames from Zoi code are marked `in_app` to aid issue grouping; runtime
/// frames are left as context.
fn frames_to_json(frames: &[ParsedFrame]) -> serde_json::Value {
    frames
        .iter()
        .map(|frame| {
            serde_json::json!({
                "function": frame.function,
                "filename": frame.filename,
                "lineno": frame.lineno,
                "colno": frame.colno,
                "in_app": frame.function.contains("zoi"),
            })
        })
        .collect()
}

/// Builds a minimal Sentry-envelope payload for a panic message.
fn build_envelope(
    event_id: &str,
    timestamp: &str,
    environment: &str,
    release: &str,
    panic_message: &str,
    backtrace: &str
) -> String {
    let header = serde_json::json!({
        "event_id": event_id.replace('-', ""),
        "dsn": super::resolve_sentry_dsn().unwrap_or_default(),
        "sdk": {"name": "zoi.telemetry", "version": env!("CARGO_PKG_VERSION")},
    });
    let item_header = serde_json::json!({"type": "event"});
    let command = std::env::var("ZOI_CRASH_COMMAND").ok();
    let payload = serde_json::json!({
        "event_id": event_id.replace('-', ""),
        "timestamp": timestamp,
        "platform": "other",
        "level": "fatal",
        "environment": environment,
        "release": release,
        "tags": {"command": command},
        "exception": {"values": [{
            "type": "panic",
            "value": panic_message,
            "stacktrace": {"frames": frames_to_json(&parse_backtrace_frames(backtrace))},
        }]},
        "contexts": {
            "os": {"name": std::env::consts::OS, "arch": std::env::consts::ARCH},
            "runtime": {"name": "zoi", "version": release},
            "zoi": {
                "distro": zoi_core::utils::get_linux_distribution(),
                "shell": zoi_core::utils::get_current_shell().map(|s| s.to_string()),
            },
        },
    });
    format!("{header}\n{item_header}\n{payload}")
}

/// Writes a crash report file and returns its path.
///
/// This never touches the network and never checks the telemetry opt-in flag:
/// local crash files are always allowed, mirroring Ghostty's approach.
///
/// # Errors
///
/// Returns an error if the crash directory cannot be created or the report
/// file cannot be written.
pub fn write_crash_report(
    panic_message: &str
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = crash_dir()?;
    let now = chrono::Utc::now();
    let event_id = {
        let ts = uuid::Timestamp::from_unix(
            uuid::NoContext,
            now.timestamp_millis().cast_unsigned(),
            0
        );
        uuid::Uuid::new_v7(ts).to_string()
    };
    let filename = format!(
        "zoi-{}-{}.{}",
        now.format("%Y%m%dT%H%M%SZ"),
        event_id.split('-').next().unwrap_or("00000000"),
        CRASH_EXTENSION
    );
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let contents = build_envelope(
        &event_id,
        &now.to_rfc3339(),
        &super::resolve_environment(),
        super::app_release().as_str(),
        panic_message,
        &backtrace
    );
    let path = dir.join(filename);
    write_restricted(&path, contents.as_bytes())?;
    Ok(path)
}

/// Installs a panic hook that saves a local `.zoicrash` file on panic.
///
/// The previous hook (including Sentry's, if initialized) is chained so no
/// behavior is lost. Saving is best-effort: hook failures are swallowed to
/// avoid masking the original panic.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map_or_else(String::new, |l| format!(" at {l}"));
        let _ = write_crash_report(&format!("{message}{location}"));
        previous(info);
    }));
}

/// Uploads a saved crash envelope to Sentry.
///
/// Returns `Ok(false)` without touching the network when telemetry is not
/// opted-in.
///
/// # Errors
///
/// Returns an error when the DSN is missing or the upload fails.
pub fn send_crash_report(
    path: &std::path::Path
) -> Result<bool, Box<dyn std::error::Error>> {
    let config = zoi_core::config::read_config()?;
    if !config.telemetry_enabled {
        return Ok(false);
    }
    if zoi_core::offline::is_offline() {
        return Err("Cannot send crash report while offline".into());
    }
    let dsn = super::resolve_sentry_dsn()
        .filter(|s| !s.is_empty())
        .ok_or("Telemetry enabled but SENTRY_DSN is not set")?;
    let (envelope_url, key) = envelope_endpoint(&dsn)?;
    let body = fs::read(path)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .use_rustls_tls()
        .build()?;
    let response = client
        .post(envelope_url)
        .header("X-Sentry-Auth", sentry_auth_header(&key))
        .header("Content-Type", "application/x-sentry-envelope")
        .body(body)
        .send()?;
    if !response.status().is_success() {
        return Err(format!("Sentry HTTP {}", response.status()).into());
    }
    Ok(true)
}

/// Derives the Sentry envelope ingest URL and public key from a DSN.
///
/// DSN format: `https://<key>@<host>/<project_id>`.
fn envelope_endpoint(
    dsn: &str
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let without_scheme = dsn.split("://").nth(1).ok_or("Invalid SENTRY_DSN")?;
    let (key, rest) =
        without_scheme.split_once('@').ok_or("Invalid SENTRY_DSN")?;
    let (host, project) = rest.rsplit_once('/').ok_or("Invalid SENTRY_DSN")?;
    Ok((
        format!("https://{host}/api/{project}/envelope/"),
        key.to_string()
    ))
}

/// Builds the `X-Sentry-Auth` header value for envelope uploads.
fn sentry_auth_header(key: &str) -> String {
    format!(
        "Sentry sentry_version=7, sentry_client=zoi.telemetry/{}, \
         sentry_key={key}",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_endpoint_parses_dsn() {
        let (url, key) = envelope_endpoint("https://abc123@o999.sentry.io/42")
            .expect("valid DSN should parse");
        assert_eq!(url, "https://o999.sentry.io/api/42/envelope/");
        assert_eq!(key, "abc123");
    }

    #[test]
    fn envelope_endpoint_rejects_garbage() {
        assert!(envelope_endpoint("not-a-dsn").is_err());
    }

    #[test]
    fn crash_extension_constant() {
        assert_eq!(CRASH_EXTENSION, "zoicrash");
    }

    #[test]
    fn parses_short_backtrace_oldest_first() {
        let backtrace = "   0: zoi_cli::cli::run\n             at \
                         ./crates/cli/src/cli.rs:900:5\n   1: zoi::main\n     \
                         at ./crates/zoi-rs/src/main.rs:105:5\n";
        let frames = parse_backtrace_frames(backtrace);
        assert_eq!(frames.len(), 2);
        let first = frames.first().expect("two frames parsed");
        let second = frames.get(1).expect("two frames parsed");
        assert_eq!(first.function, "zoi::main");
        assert_eq!(
            first.filename.as_deref(),
            Some("./crates/zoi-rs/src/main.rs")
        );
        assert_eq!(first.lineno, Some(105));
        assert_eq!(first.colno, Some(5));
        assert_eq!(second.function, "zoi_cli::cli::run");
        assert_eq!(second.lineno, Some(900));
    }

    #[test]
    fn skips_headers_and_keeps_locationless_frames() {
        let backtrace =
            "stack backtrace:\n   0: some_symbol\n   1: other::symbol\n";
        let frames = parse_backtrace_frames(backtrace);
        assert_eq!(frames.len(), 2);
        let first = frames.first().expect("two frames parsed");
        let second = frames.get(1).expect("two frames parsed");
        assert_eq!(first.function, "other::symbol");
        assert_eq!(first.filename, None);
        assert_eq!(second.function, "some_symbol");
    }

    #[test]
    fn tolerates_colons_inside_paths() {
        let (filename, lineno, colno) =
            parse_frame_location("./we:ird/path.rs:12:3");
        assert_eq!(filename.as_deref(), Some("./we:ird/path.rs"));
        assert_eq!(lineno, Some(12));
        assert_eq!(colno, Some(3));
    }
}
