//! A minimal Sentry client: error events, panics and release health.
//!
//! Zoi reports at most a few events per command, so this keeps them in
//! memory and sends them when the command ends, together with the command's
//! release-health session (one session per invocation, as the official
//! SDK's "application" session mode does). Everything goes through Sentry's
//! envelope endpoint, the same one [`crate::crash`] uploads saved crash
//! reports to.
//!
//! Sending is bounded by a timeout so telemetry never holds up the CLI: on a
//! slow network, events and the session are dropped rather than waited for.
//!
//! Privacy: no user, IP address or host name is ever sent; events carry the
//! message, tags, release, environment and OS, architecture and runtime
//! contexts only.

use std::fmt::Write as _;
use std::sync::{Mutex, Once, PoisonError, TryLockError};
use std::time::{Duration, Instant};

/// Identifies this client to Sentry (`sdk.name`, `sentry_client`).
const SDK_NAME: &str = "zoi.telemetry";

/// How long the panic hook may spend sending the crash before the process
/// continues to abort or unwind.
const PANIC_SEND_TIMEOUT: Duration = Duration::from_secs(2);

/// A parsed Sentry DSN (`<scheme>://<public key>@<host>/<project id>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dsn {
    /// The DSN as given, sent in envelope headers.
    raw: String,
    /// The project's envelope endpoint.
    envelope_url: String,
    /// The public key, used in the `X-Sentry-Auth` header.
    public_key: String
}

impl Dsn {
    /// Parses a DSN.
    ///
    /// # Errors
    ///
    /// Returns an error when the DSN lacks a scheme (`http` or `https`), a
    /// public key, a host or a project id.
    pub fn parse(dsn: &str) -> Result<Self, String> {
        let dsn = dsn.trim();
        let invalid = || format!("Invalid Sentry DSN: {dsn}");
        let (scheme, rest) = dsn.split_once("://").ok_or_else(invalid)?;
        if scheme != "https" && scheme != "http" {
            return Err(invalid());
        }
        let (public_key, location) =
            rest.split_once('@').ok_or_else(invalid)?;
        // A secret key (`public:secret@`) is deprecated; only the public part
        // is used.
        let public_key = public_key.split(':').next().unwrap_or_default();
        let (host, project) = location
            .trim_end_matches('/')
            .rsplit_once('/')
            .ok_or_else(invalid)?;
        if public_key.is_empty()
            || host.is_empty()
            || project.is_empty()
            || !project.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }
        Ok(Self {
            raw: dsn.to_string(),
            envelope_url: format!("{scheme}://{host}/api/{project}/envelope/"),
            public_key: public_key.to_string()
        })
    }

    /// The project's envelope endpoint.
    #[must_use]
    pub fn envelope_url(&self) -> &str {
        &self.envelope_url
    }

    /// The value of the `X-Sentry-Auth` header for this DSN.
    #[must_use]
    pub fn auth_header(&self) -> String {
        format!(
            "Sentry sentry_version=7, sentry_client={SDK_NAME}/{}, \
             sentry_key={}",
            env!("CARGO_PKG_VERSION"),
            self.public_key
        )
    }
}

/// Sends an envelope to the DSN's project.
///
/// # Errors
///
/// Returns an error when the request fails or Sentry answers with an error
/// status.
pub fn send_envelope(
    dsn: &Dsn,
    envelope: Vec<u8>,
    timeout: Duration
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .use_rustls_tls()
        .build()?;
    let response = client
        .post(dsn.envelope_url())
        .header("X-Sentry-Auth", dsn.auth_header())
        .header("Content-Type", "application/x-sentry-envelope")
        .body(envelope)
        .send()?;
    if !response.status().is_success() {
        return Err(format!("Sentry HTTP {}", response.status()).into());
    }
    Ok(())
}

/// How a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// The command ran to its end, successfully or with an error.
    Exited,
    /// The command panicked.
    Crashed
}

impl SessionStatus {
    /// The status as Sentry names it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::Crashed => "crashed"
        }
    }
}

/// The state of the running client.
struct Client {
    /// Where events go.
    dsn: Dsn,
    /// `environment` of every event and the session.
    environment: String,
    /// `release` of every event and the session.
    release: String,
    /// Events captured so far, as JSON payloads.
    pending: Vec<serde_json::Value>,
    /// The session id.
    session_id: String,
    /// When the session started, as sent to Sentry.
    started: String,
    /// When the session started, for its duration.
    started_at: Instant,
    /// Error events captured during the session.
    errors: u32
}

/// The running client, if Sentry is initialized.
static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

/// Locks the client slot, recovering from a poisoned lock (a panic while
/// held must not stop the crash report).
fn client_slot() -> std::sync::MutexGuard<'static, Option<Client>> {
    CLIENT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Keeps Sentry running; dropping it ends the session and sends what is
/// pending, waiting at most the timeout given to [`init`].
#[must_use = "dropping the guard immediately ends the Sentry session"]
pub struct Guard {
    /// The longest the final send may take.
    shutdown_timeout: Duration
}

impl Drop for Guard {
    fn drop(&mut self) {
        close(self.shutdown_timeout);
    }
}

/// Starts Sentry for this process: begins the session and installs a panic
/// hook that reports the crash (chained to the hooks already installed).
///
/// Replaces a client that is already running, ending its session first.
pub fn init(
    dsn: Dsn,
    environment: String,
    release: String,
    shutdown_timeout: Duration
) -> Guard {
    close(shutdown_timeout);
    *client_slot() = Some(Client {
        dsn,
        environment,
        release,
        pending: Vec::new(),
        session_id: new_uuid(),
        started: jiff::Timestamp::now().to_string(),
        started_at: Instant::now(),
        errors: 0
    });
    install_panic_hook();
    Guard { shutdown_timeout }
}

/// Whether a client is running.
#[must_use]
pub fn is_initialized() -> bool {
    client_slot().is_some()
}

/// Queues an error event with `message` and `tags`, sent when the session
/// ends. Does nothing when Sentry is not initialized.
pub fn capture_message(message: &str, tags: &[(&str, &str)]) {
    let mut slot = client_slot();
    let Some(client) = slot.as_mut() else {
        return;
    };
    let tags: serde_json::Map<String, serde_json::Value> = tags
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).into()))
        .collect();
    let event = build_event(
        &client.environment,
        &client.release,
        "error",
        serde_json::json!({ "message": { "formatted": message }, "tags": tags })
    );
    client.pending.push(event);
    client.errors = client.errors.saturating_add(1);
}

/// Ends the session and sends it with the pending events, waiting at most
/// `timeout`. Returns `true` when everything was delivered in time, `false`
/// on failure, timeout, or when Sentry was not initialized.
pub fn close(timeout: Duration) -> bool {
    let Some(client) = client_slot().take() else {
        return false;
    };
    finish(&client, SessionStatus::Exited, timeout)
}

/// Sends the client's events and its ended session.
fn finish(client: &Client, status: SessionStatus, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let session = build_session(client, status);
    let mut events = client.pending.iter();
    // Sentry takes at most one event per envelope, so the session travels
    // with the first event and any further events go on their own.
    let mut envelopes =
        vec![build_envelope(&client.dsn, events.next(), Some(&session))];
    envelopes
        .extend(events.map(|e| build_envelope(&client.dsn, Some(e), None)));
    let mut delivered = true;
    for envelope in envelopes {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        delivered &= send_envelope(&client.dsn, envelope, remaining).is_ok();
    }
    delivered
}

/// Builds an event payload with the fields every event shares.
///
/// `extra` supplies the event's own fields (`message` or `exception`,
/// `tags`).
#[must_use]
pub fn build_event(
    environment: &str,
    release: &str,
    level: &str,
    extra: serde_json::Value
) -> serde_json::Value {
    let mut event = serde_json::json!({
        "event_id": new_uuid().replace('-', ""),
        "timestamp": jiff::Timestamp::now().to_string(),
        "platform": "native",
        "level": level,
        "environment": environment,
        "release": release,
        "sdk": { "name": SDK_NAME, "version": env!("CARGO_PKG_VERSION") },
        "contexts": contexts(release),
    });
    if let (Some(event), serde_json::Value::Object(extra)) =
        (event.as_object_mut(), extra)
    {
        event.extend(extra);
    }
    event
}

/// The `os`, `device` and `runtime` contexts of an event.
#[must_use]
pub fn contexts(release: &str) -> serde_json::Value {
    let shell = zoi_core::utils::get_current_shell().map(|s| s.to_string());
    serde_json::json!({
        "os": {
            "name": std::env::consts::OS,
            "distribution": zoi_core::utils::get_linux_distribution(),
        },
        "device": { "arch": std::env::consts::ARCH },
        "runtime": { "name": "zoi", "version": release },
        "zoi": {
            "shell": shell,
        },
    })
}

/// Builds the session update that ends the client's session.
fn build_session(client: &Client, status: SessionStatus) -> serde_json::Value {
    serde_json::json!({
        "sid": client.session_id,
        "init": true,
        "started": client.started,
        "timestamp": jiff::Timestamp::now().to_string(),
        "status": status.as_str(),
        "errors": client.errors,
        "duration": client.started_at.elapsed().as_secs_f64(),
        "attrs": {
            "release": client.release,
            "environment": client.environment,
        },
    })
}

/// Builds an envelope holding an optional event and an optional session.
#[must_use]
pub fn build_envelope(
    dsn: &Dsn,
    event: Option<&serde_json::Value>,
    session: Option<&serde_json::Value>
) -> Vec<u8> {
    let mut header = serde_json::json!({
        "dsn": dsn.raw,
        "sent_at": jiff::Timestamp::now().to_string(),
        "sdk": { "name": SDK_NAME, "version": env!("CARGO_PKG_VERSION") },
    });
    if let (Some(event_id), Some(header)) = (
        event.and_then(|e| e.get("event_id")).cloned(),
        header.as_object_mut()
    ) {
        header.insert("event_id".into(), event_id);
    }
    let mut out = format!("{header}\n");
    for (kind, item) in [("event", event), ("session", session)] {
        let Some(item) = item else { continue };
        let payload = item.to_string();
        let item_header =
            serde_json::json!({ "type": kind, "length": payload.len() });
        let _ = writeln!(out, "{item_header}\n{payload}");
    }
    out.into_bytes()
}

/// Installs, once per process, a panic hook that reports the panic as a
/// fatal event, ends the session as crashed, sends both, then runs the hooks
/// installed before it.
fn install_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            report_panic(info);
            previous(info);
        }));
    });
}

/// Reports a panic to Sentry, if it is initialized.
fn report_panic(info: &std::panic::PanicHookInfo<'_>) {
    // `try_lock`: a panic while the client is locked must not deadlock, and
    // a poisoned lock still holds a usable client.
    let mut slot = match CLIENT.try_lock() {
        Ok(slot) => slot,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return
    };
    let Some(mut client) = slot.take() else {
        return;
    };
    drop(slot);
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let event = build_event(
        &client.environment,
        &client.release,
        "fatal",
        serde_json::json!({
            "exception": { "values": [{
                "type": "panic",
                "value": crate::crash::panic_message(info),
                "stacktrace": {
                    "frames": crate::crash::stack_frames(&backtrace),
                },
            }]},
            "tags": { "command": std::env::var("ZOI_CRASH_COMMAND").ok() },
        })
    );
    client.pending.insert(0, event);
    client.errors = client.errors.saturating_add(1);
    finish(&client, SessionStatus::Crashed, PANIC_SEND_TIMEOUT);
}

/// A new time-ordered (v7) UUID.
fn new_uuid() -> String {
    let ts = uuid::Timestamp::from_unix(
        uuid::NoContext,
        jiff::Timestamp::now().as_millisecond().cast_unsigned(),
        0
    );
    uuid::Uuid::new_v7(ts).to_string()
}

#[cfg(test)]
mod tests {
    use super::{Dsn, build_envelope, build_event};

    #[test]
    fn dsn_parses_into_envelope_url_and_key() {
        let dsn = Dsn::parse("https://abc123@o999.ingest.sentry.io/42")
            .expect("valid DSN");
        assert_eq!(
            dsn.envelope_url(),
            "https://o999.ingest.sentry.io/api/42/envelope/"
        );
        assert!(dsn.auth_header().contains("sentry_key=abc123"));
    }

    #[test]
    fn dsn_keeps_http_for_local_servers() {
        let dsn = Dsn::parse("http://key@127.0.0.1:9000/7").expect("valid");
        assert_eq!(dsn.envelope_url(), "http://127.0.0.1:9000/api/7/envelope/");
    }

    #[test]
    fn dsn_rejects_garbage() {
        for bad in [
            "not-a-dsn",
            "ftp://key@host/1",
            "https://host/1",
            "https://key@host",
            "https://key@host/project"
        ] {
            assert!(Dsn::parse(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn envelope_has_a_header_line_and_one_line_pair_per_item() {
        let dsn = Dsn::parse("https://k@host/1").expect("valid");
        let event = build_event(
            "test",
            "zoi@1.0.0",
            "error",
            serde_json::json!({ "message": { "formatted": "boom" } })
        );
        let session = serde_json::json!({ "sid": "s", "status": "exited" });
        let envelope = String::from_utf8(build_envelope(
            &dsn,
            Some(&event),
            Some(&session)
        ))
        .expect("UTF-8");
        let lines: Vec<&str> = envelope.lines().collect();
        assert_eq!(lines.len(), 5, "{envelope}");
        let header: serde_json::Value =
            serde_json::from_str(lines.first().expect("header")).expect("json");
        assert_eq!(header.get("event_id"), event.get("event_id"));
        assert!(
            lines
                .get(1)
                .is_some_and(|l| l.contains(r#""type":"event""#))
        );
        assert!(
            lines
                .get(3)
                .is_some_and(|l| l.contains(r#""type":"session""#))
        );
        assert!(lines.get(2).is_some_and(|l| l.contains("boom")));
    }

    #[test]
    fn events_carry_no_user_or_host() {
        let event =
            build_event("test", "zoi@1.0.0", "error", serde_json::json!({}));
        assert!(event.get("user").is_none());
        assert!(event.get("server_name").is_none());
        assert_eq!(event.get("level"), Some(&"error".into()));
        assert_eq!(event.get("release"), Some(&"zoi@1.0.0".into()));
    }
}
