//! Durable out-of-process `PostHog` delivery queue.
//!
//! Rationale: analytics must never stall the CLI. The hot path (every command,
//! every install/uninstall) only appends a small JSON file under
//! `$XDG_STATE_HOME/zoi/telemetry/queue/` - fast local I/O, no network - and
//! spawns a detached `zoi telemetry flush` child that delivers the files after
//! the parent has already exited. Files that fail to deliver stay queued and
//! are retried by the next flush, so a slow or absent network costs the user
//! nothing but background work.
//!
//! Environment overrides (all optional):
//! - `ZOI_TELEMETRY_NO_SPAWN`: when set, producers enqueue without spawning a
//!   flusher child. Used by tests (where `current_exe` is the test binary, not
//!   `zoi`) and as a manual escape hatch.
//! - `ZOI_TELEMETRY_FLUSHING`: set on the flusher child itself. Producers treat
//!   it as "already flushing" and skip enqueueing, so the flush command can
//!   never enqueue (and re-spawn) itself.

use std::error::Error;
use std::io::{ErrorKind, Write as _};
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs};

/// Version tag embedded in every queue file for forward compatibility.
pub const QUEUE_FORMAT_VERSION: u32 = 1;

/// File extension for queued analytics events.
pub const QUEUE_EXTENSION: &str = "json";

/// Upper bound on queued files. Beyond this the oldest files are dropped at
/// enqueue time so a broken backend can never fill the disk.
pub const MAX_QUEUE_FILES: usize = 100;

/// Maximum age of a queued file. Older files are pruned instead of delivered,
/// so stale analytics never skew current numbers.
pub const MAX_QUEUE_AGE: Duration = Duration::from_hours(30 * 24);

/// Name of the single-flight lock file inside the queue directory.
pub const FLUSH_LOCK_FILE: &str = "flush.lock";

/// A flush lock older than this is considered stale (crashed flusher) and may
/// be stolen by the next flusher.
pub const FLUSH_LOCK_STALE_AFTER: Duration = Duration::from_mins(10);

/// A single analytics event waiting for delivery.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct QueuedEvent {
    /// Schema version, always [`QUEUE_FORMAT_VERSION`].
    pub version: u32,
    /// `PostHog` event name (`"dau"`, `"install"`, `"uninstall"`, ...).
    pub event: String,
    /// Anonymous client ID (`distinct_id`).
    pub distinct_id: String,
    /// Event properties, exactly as the producer built them.
    pub props: serde_json::Map<String, serde_json::Value>
}

/// Outcome of a foreground [`flush_queue`] run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FlushSummary {
    /// Files successfully delivered and deleted.
    pub sent: usize,
    /// Files that failed delivery and stay queued for retry.
    pub failed: usize,
    /// Files skipped: unreadable, unparsable, pruned, or left alone because
    /// another flusher holds the lock / telemetry is currently disabled.
    pub skipped: usize
}

/// Returns `true` inside the detached flusher child.
///
/// Producers check this to avoid enqueueing (and re-spawning) from the flush
/// command itself.
pub(crate) fn is_flusher_child() -> bool {
    env::var_os("ZOI_TELEMETRY_FLUSHING").is_some()
}

/// Returns the queue directory, creating it with owner-only access on Unix.
fn queue_dir() -> Result<PathBuf, Box<dyn Error>> {
    let dir = zoi_core::utils::get_user_state_dir()?
        .join("telemetry")
        .join("queue");
    fs::create_dir_all(&dir)?;
    restrict_dir(&dir)?;
    Ok(dir)
}

/// Restricts a directory to owner-only access on Unix.
#[cfg(unix)]
fn restrict_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Non-Unix platforms manage access via ACLs; nothing to do.
#[cfg(not(unix))]
fn restrict_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Restricts a file to owner-only access on Unix.
#[cfg(unix)]
fn restrict_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// Non-Unix platforms manage access via ACLs; nothing to do.
#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Monotonic counter mixed into file names so two enqueues in the same
/// millisecond never collide.
static ENQUEUE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Builds a unique file stem from the current time, PID, and a counter.
fn unique_file_stem() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let n = ENQUEUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ms}-{}-{n}", std::process::id())
}

/// Returns the modification time of a file in milliseconds since the epoch, or
/// `None` when it cannot be determined. Unknown-mtime files sort as newest so
/// pruning never deletes a file it cannot age.
fn mtime_millis(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis())
}

/// Lists queued event files, oldest first. Returns an empty vec when the queue
/// directory does not exist yet.
fn queued_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str())
                        == Some(QUEUE_EXTENSION)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| mtime_millis(p).unwrap_or(u128::MAX));
    files
}

/// Drops files older than [`MAX_QUEUE_AGE`] and, when still over
/// [`MAX_QUEUE_FILES`], the oldest extras. Best-effort: failures are ignored
/// so pruning can never fail a command.
fn prune_queue(dir: &Path) {
    let files = queued_files(dir);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let max_age_ms = MAX_QUEUE_AGE.as_millis();
    let mut survivors: Vec<PathBuf> = Vec::with_capacity(files.len());
    for path in files {
        let too_old = mtime_millis(&path)
            .is_some_and(|t| now_ms.saturating_sub(t) > max_age_ms);
        if too_old {
            fs::remove_file(&path).ok();
        } else {
            survivors.push(path);
        }
    }
    if survivors.len() > MAX_QUEUE_FILES {
        for stale in survivors.iter().take(survivors.len() - MAX_QUEUE_FILES) {
            fs::remove_file(stale).ok();
        }
    }
}

/// Appends an event to the delivery queue. Pure local I/O: never touches the
/// network, so it cannot stall the CLI.
///
/// # Errors
///
/// Returns an error when the queue directory cannot be created or the event
/// cannot be written.
pub fn enqueue(
    event: &str,
    distinct_id: &str,
    props: &serde_json::Map<String, serde_json::Value>
) -> Result<(), Box<dyn Error>> {
    let dir = queue_dir()?;
    let body = serde_json::to_string(&QueuedEvent {
        version: QUEUE_FORMAT_VERSION,
        event: event.to_string(),
        distinct_id: distinct_id.to_string(),
        props: props.clone()
    })
    .map_err(|e| format!("Failed to serialize telemetry event: {e}"))?;
    // - Tmp-plus-rename keeps a killed writer from leaving a half-written queue
    //   file behind.
    // - `create_new` plus a few retries covers the (near-impossible) case of
    //   two writers picking the same stem.
    for _ in 0..10 {
        let stem = unique_file_stem();
        let tmp = dir.join(format!("{stem}.tmp"));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Box::new(e))
        };
        if let Err(e) = file.write_all(body.as_bytes()) {
            fs::remove_file(&tmp).ok();
            return Err(Box::new(e));
        }
        drop(file);
        restrict_file(&tmp).ok();
        fs::rename(&tmp, dir.join(format!("{stem}.{QUEUE_EXTENSION}")))?;
        // - Prune after the write so the cap counts the new file: the oldest
        //   files go, the event just queued always survives.
        prune_queue(&dir);
        return Ok(());
    }
    Err("Could not allocate a telemetry queue file name".into())
}

/// Lists queued event files, oldest first. Empty when telemetry never queued
/// anything (or the queue was fully delivered).
pub fn list_queued() -> Vec<PathBuf> {
    let dir = zoi_core::utils::get_user_state_dir()
        .map(|d| d.join("telemetry").join("queue"))
        .unwrap_or_default();
    queued_files(&dir)
}

/// Whether the single-flight flush lock file exists and looks fresh.
fn flush_lock_is_fresh(lock_path: &Path) -> bool {
    if !lock_path.exists() {
        return false;
    }
    !lock_is_stale(lock_path)
}

/// Whether a lock file is stale (older than [`FLUSH_LOCK_STALE_AFTER`]) and
/// may be stolen. Unreadable locks count as stale so a wedged lock can never
/// block delivery forever; locks from the future (clock skew) count as fresh.
fn lock_is_stale(lock_path: &Path) -> bool {
    match fs::metadata(lock_path)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed())
    {
        Ok(Ok(age)) => age > FLUSH_LOCK_STALE_AFTER,
        Ok(Err(_)) => false,
        Err(_) => true
    }
}

/// Outcome of trying to take the single-flight flush lock.
enum LockOutcome {
    /// Lock taken (or stolen from a stale holder); caller must flush.
    Acquired,
    /// Another flusher is working; caller must back off.
    Held
}

/// Takes the single-flight lock, stealing it when stale.
fn acquire_flush_lock(lock_path: &Path) -> LockOutcome {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{} {}", std::process::id(), now_millis());
            LockOutcome::Acquired
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            if lock_is_stale(lock_path) {
                fs::remove_file(lock_path).ok();
                return acquire_flush_lock(lock_path);
            }
            LockOutcome::Held
        }
        Err(_) => LockOutcome::Held
    }
}

/// Current wall-clock time in milliseconds since the epoch (0 on clock error).
fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Refreshes the lock's mtime so long flushes are never mistaken for stale.
fn touch_lock(lock_path: &Path) {
    fs::write(
        lock_path,
        format!("{} {}", std::process::id(), now_millis())
    )
    .ok();
}

/// Removes the flush lock when the flush run ends, for any reason.
struct FlushLockGuard {
    /// Path of the lock file to remove on drop.
    path: PathBuf
}

impl Drop for FlushLockGuard {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

/// Delivers every queued event to `PostHog`, oldest first.
///
/// Runs in the detached flusher child (or when invoked manually): generous
/// timeouts are fine here because no CLI invocation waits on this. Files that
/// fail stay queued for the next run; unparsable files are deleted. Delivery
/// stops at the first transport-level failure (the network is down; the rest
/// would fail identically), while per-event rejections only skip that file.
///
/// Never fails the caller: all outcomes are reported in the summary.
pub fn flush_queue() -> FlushSummary {
    let mut summary = FlushSummary::default();
    let Ok(dir) = queue_dir() else {
        return summary;
    };
    prune_queue(&dir);
    let lock_path = dir.join(FLUSH_LOCK_FILE);
    if matches!(acquire_flush_lock(&lock_path), LockOutcome::Held) {
        summary.skipped = queued_files(&dir).len();
        return summary;
    }
    let _lock = FlushLockGuard {
        path: lock_path.clone()
    };

    let Some(client) = super::posthog_client() else {
        // - Opted-out, offline, mini, or unconfigured: leave everything queued.
        return summary;
    };

    for path in queued_files(&dir) {
        touch_lock(&lock_path);
        let Ok(body) = fs::read_to_string(&path) else {
            summary.skipped += 1;
            continue;
        };
        let Ok(queued) = serde_json::from_str::<QueuedEvent>(&body) else {
            // - Corrupt file: undeliverable, drop it rather than retrying
            //   forever.
            fs::remove_file(&path).ok();
            summary.skipped += 1;
            continue;
        };
        let mut event = posthog_rs::Event::new(
            queued.event.as_str(),
            queued.distinct_id.as_str()
        );
        for (key, value) in queued.props {
            let _ = event.insert_prop(key, value);
        }
        match client.capture_immediate(event) {
            Ok(report) if report.all_persisted() => {
                fs::remove_file(&path).ok();
                summary.sent += 1;
            }
            Ok(_) => {
                // - Accepted by the transport but not persisted: keep for
                //   retry, move on to the next file.
                summary.failed += 1;
            }
            Err(_) => {
                // - Transport-level failure (network down, DNS, refused): the
                //   rest would fail identically, so stop here.
                summary.failed += 1;
                break;
            }
        }
    }
    summary
}

/// Spawns a detached `zoi telemetry flush` child to deliver the queue.
///
/// Fire-and-forget by design: stdio is nulled, the child leaves the process
/// group/session (Unix) or window station (Windows), and the parent never
/// waits - delivery happens after this process has exited. Returns `true` when
/// the child was launched. Never fails the caller: every failure mode returns
/// `false` (events stay queued for the next invocation).
///
/// Skipped (returns `false`) when `ZOI_TELEMETRY_NO_SPAWN` is set (tests),
/// inside the flusher child itself, or when a fresh flush lock shows a
/// flusher is already working.
pub fn spawn_flusher() -> bool {
    if env::var_os("ZOI_TELEMETRY_NO_SPAWN").is_some() || is_flusher_child() {
        return false;
    }
    if let Ok(dir) = zoi_core::utils::get_user_state_dir()
        && flush_lock_is_fresh(
            &dir.join("telemetry").join("queue").join(FLUSH_LOCK_FILE)
        )
    {
        return false;
    }
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    let mut cmd = Command::new(exe);
    cmd.args(["telemetry", "flush"]);
    cmd.env("ZOI_TELEMETRY_FLUSHING", "1");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        // SAFETY: Runs after `fork` in the child before `exec`. `setsid(2)`
        // is async-signal-safe and touches no shared state, so it is safe
        // here; the new session keeps terminal hangup from killing the
        // flusher after the parent exits.
        unsafe {
            cmd.pre_exec(|| {
                nix::unistd::setsid()
                    .map(|_| ())
                    .map_err(std::io::Error::other)
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }
    // - Deliberately never waited on: process exit reparents the child to init,
    //   which reaps it.
    cmd.spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::QueuedEvent;

    #[test]
    fn queued_event_roundtrips_through_json() {
        let mut props = serde_json::Map::new();
        props.insert("command".into(), "install".into());
        let event = QueuedEvent {
            version: super::QUEUE_FORMAT_VERSION,
            event: "dau".into(),
            distinct_id: "client-1".into(),
            props
        };
        let body = serde_json::to_string(&event).expect("serialize");
        let back: QueuedEvent =
            serde_json::from_str(&body).expect("deserialize");
        assert_eq!(back.event, "dau");
        assert_eq!(back.distinct_id, "client-1");
        assert_eq!(back.props.len(), 1);
    }

    #[test]
    fn file_stems_are_unique() {
        let a = super::unique_file_stem();
        let b = super::unique_file_stem();
        assert_ne!(a, b);
    }
}
