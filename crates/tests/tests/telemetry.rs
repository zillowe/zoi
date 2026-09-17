//! Integration tests for Zoi telemetry and usage reporting.

use tempfile::tempdir;
use zoi::pkg::{config, telemetry, types};

mod common;

#[test]
fn test_telemetry_respects_opt_in() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    // Keep the test hermetic: ignore any real keys from the ambient env.
    ctx.set_env_var("POSTHOG_API_KEY", "");
    ctx.set_env_var("SENTRY_DSN", "");
    common::TestContextGuard::set_sysroot(root.clone());

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };

    let res =
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
            .expect("unwrap failed");
    assert!(!res, "Telemetry should return false when disabled");

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    // With no API key configured the SDK client cannot be built, which must
    // surface as an error (not a silent success) so misconfiguration is
    // visible to packagers.
    let res_enabled =
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None);
    assert!(
        res_enabled.is_err(),
        "Telemetry without POSTHOG_API_KEY should error, got {res_enabled:?}"
    );
}

#[test]
fn test_mini_mode_is_never_tracked() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "");
    ctx.set_env_var("SENTRY_DSN", "");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };

    // `zoi-mini` is never tracked, even when the user opted-in.
    ctx.set_env_var("ZOI_MINI_MODE", "1");
    let res =
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
            .expect("mini mode should not error");
    assert!(!res, "zoi-mini must never send telemetry");
    assert!(
        telemetry::init_sentry().is_none(),
        "zoi-mini must never initialize Sentry"
    );

    // Without the mini marker (main CLI and `zoid` alike) the normal opt-in
    // path applies: with no API key configured this errors instead of
    // silently succeeding.
    ctx.set_env_var("ZOI_MINI_MODE", "");
    let res =
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None);
    assert!(
        res.is_err(),
        "opted-in non-mini context without a key should error, got {res:?}"
    );
}

#[test]
fn test_dau_ping_respects_opt_in_and_mini_mode() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "");
    common::TestContextGuard::set_sysroot(root.clone());

    // Opted-out: silent false, no network.
    let res = telemetry::posthog_capture_dau_ping()
        .expect("opted-out DAU ping should not error");
    assert!(!res, "DAU ping must be silent when opted-out");

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    // Opted-in without a key: surfaces misconfiguration as an error.
    let res = telemetry::posthog_capture_dau_ping();
    assert!(
        res.is_err(),
        "DAU ping without POSTHOG_API_KEY should error, got {res:?}"
    );

    // Mini mode never tracks, even when opted-in.
    ctx.set_env_var("ZOI_MINI_MODE", "1");
    let res = telemetry::posthog_capture_dau_ping()
        .expect("mini mode should not error");
    assert!(!res, "zoi-mini must never send DAU pings");
}

/// Runs `f` with a client-less Sentry Hub bound to the current thread.
///
/// `sentry::init` permanently binds a client to the process-wide main Hub,
/// which thread-local Hubs inherit - so "without init" assertions must run
/// inside an explicitly empty Hub to be deterministic regardless of test
/// thread scheduling.
fn without_sentry_client<R>(f: impl FnOnce() -> R) -> R {
    let hub =
        std::sync::Arc::new(sentry::Hub::new(None, std::sync::Arc::default()));
    sentry::Hub::run(hub, f)
}

#[test]
fn test_shutdown_sentry_without_init_is_noop() {
    // Must return false (nothing to drain) without hanging or panicking.
    assert!(!without_sentry_client(telemetry::shutdown_sentry));
}

#[test]
fn test_shutdown_sentry_drains_initialized_client() {
    // With an initialized client and an empty queue, close must succeed
    // without touching the network.
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("SENTRY_DSN", "https://abc123@o999.sentry.io/42");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let _guard = telemetry::init_sentry();
    assert!(telemetry::shutdown_sentry());
}

#[test]
fn test_sentry_capture_error_is_safe_without_init() {
    // Must never panic, with or without a Sentry client bound. Scoped to an
    // empty Hub so no delivery is attempted against other tests' clients.
    let err = anyhow::anyhow!("boom");
    without_sentry_client(|| {
        telemetry::sentry_capture_error("update", err.as_ref());
    });
}

#[test]
fn test_init_sentry_with_dsn_does_not_panic() {
    // Regression test: `sentry::init` with a DSN panicked with
    // "sentry crate was compiled without transport", which broke every CLI
    // invocation (and suppressed the crash-upload prompt) for opted-in users.
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("SENTRY_DSN", "https://abc123@o999.sentry.io/42");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let _guard = telemetry::init_sentry();
}

#[test]
fn test_crash_report_saved_locally_without_opt_in() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    common::TestContextGuard::set_sysroot(root.clone());

    // Crash files are always allowed locally, even when opted-out.
    let path = telemetry::crash::write_crash_report("test panic")
        .expect("crash report should be written");
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some("zoicrash"));
    assert!(path.exists(), "crash file should exist on disk");

    let listed = telemetry::crash::list_crashes();
    assert!(
        listed.contains(&path),
        "written crash report should be listed"
    );

    // The envelope payload must contain the panic message.
    let contents =
        std::fs::read_to_string(&path).expect("crash file should be readable");
    assert!(
        contents.contains("test panic"),
        "envelope should embed message"
    );
    assert_eq!(contents.lines().count(), 3, "envelope should have 3 lines");
}

#[test]
#[cfg(unix)]
fn test_crash_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    common::TestContextGuard::set_sysroot(root.clone());

    let path = telemetry::crash::write_crash_report("permission check")
        .expect("crash report should be written");
    let dir = path.parent().expect("crash file should have a parent dir");
    assert_eq!(
        std::fs::metadata(dir)
            .expect("crash dir should exist")
            .permissions()
            .mode()
            & 0o777,
        0o700,
        "crash directory must be owner-only"
    );
    assert_eq!(
        std::fs::metadata(&path)
            .expect("crash file should exist")
            .permissions()
            .mode()
            & 0o777,
        0o600,
        "crash report must be owner-only"
    );
}

#[test]
fn test_crash_send_refuses_when_opted_out() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    ctx.set_env_var("SENTRY_DSN", "");
    common::TestContextGuard::set_sysroot(root.clone());

    let path = telemetry::crash::write_crash_report("test panic for send")
        .expect("crash report should be written");

    // Default config has telemetry disabled: send must return false without
    // touching the network.
    let sent = telemetry::crash::send_crash_report(&path)
        .expect("send should not error when opted-out");
    assert!(!sent, "Crash upload must refuse while opted-out");
    assert!(path.exists(), "refused upload must keep the local file");
}

#[test]
fn test_dau_ping_throttled_to_once_per_day() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    // Fake key: the throttled path must return before any delivery is
    // attempted, so no network is needed even with a key configured.
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-throttle-test");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    // Pretend a DAU ping was just sent: the next call must be a silent
    // skip, not a second upload.
    let ts_path = zoi_core::utils::get_user_state_dir()
        .expect("state dir should resolve")
        .join("telemetry")
        .join("last_dau_ts");
    std::fs::create_dir_all(ts_path.parent().expect("ts file has a parent"))
        .expect("telemetry state dir should be creatable");
    let now_ms =
        u128::from(chrono::Utc::now().timestamp_millis().unsigned_abs());
    std::fs::write(&ts_path, now_ms.to_string())
        .expect("ts should be writable");

    let res = telemetry::posthog_capture_dau_ping()
        .expect("throttled DAU ping should not error");
    assert!(!res, "DAU ping within 24h of the last one must be skipped");
    assert!(
        telemetry::queue::list_queued().is_empty(),
        "throttled ping must not enqueue anything"
    );
}

#[test]
fn test_posthog_events_are_queued_without_network() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-queue-test");
    // - Never spawn the real flusher: `current_exe` here is the test binary.
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };

    // - Hot path does local I/O only: accepted even though the key is fake and
    //   no delivery is attempted.
    let queued =
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
            .expect("queueing should not error");
    assert!(queued, "opted-in event should be accepted for delivery");

    let dau = telemetry::posthog_capture_dau_ping().expect("dau queueing");
    assert!(dau, "first DAU ping should be accepted for delivery");

    let files = telemetry::queue::list_queued();
    assert_eq!(
        files.len(),
        2,
        "both events should sit in the queue, got {files:?}"
    );

    // - Payloads survive the round trip with the fields the flusher needs.
    let mut names: Vec<String> = files
        .iter()
        .map(|p| {
            let body = std::fs::read_to_string(p).expect("queue file readable");
            let value: serde_json::Value =
                serde_json::from_str(&body).expect("valid json");
            assert!(
                value
                    .get("distinct_id")
                    .and_then(|d| d.as_str())
                    .is_some_and(|d| !d.is_empty()),
                "queued event must carry a distinct_id"
            );
            assert!(
                value.get("props").and_then(|p| p.as_object()).is_some(),
                "queued event must carry props"
            );
            value
                .get("event")
                .and_then(|e| e.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["dau", "test"]);
}

/// Minimal stub `PostHog` server: accepts `expected` connections, answers each
/// with `200 {}`, and counts them. Panics (failing the test on join) if the
/// requests never arrive.
fn start_stub_posthog(
    expected: usize
) -> (
    String,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
    std::thread::JoinHandle<()>
) {
    use std::io::{BufRead as _, Read as _, Write as _};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("stub bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let addr = listener.local_addr().expect("stub addr");
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_child = Arc::clone(&hits);
    let handle = std::thread::spawn(move || {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(30);
        while hits_child.load(Ordering::SeqCst) < expected {
            assert!(
                std::time::Instant::now() <= deadline,
                "stub PostHog timed out waiting for requests"
            );
            match listener.accept() {
                Ok((stream, _)) => {
                    let mut reader = std::io::BufReader::new(stream);
                    let mut content_length = 0usize;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            break;
                        }
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() {
                            break;
                        }
                        if let Some(value) = trimmed
                            .strip_prefix("Content-Length:")
                            .or_else(|| trimmed.strip_prefix("content-length:"))
                        {
                            content_length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    if content_length > 0 {
                        let mut body = vec![0u8; content_length];
                        reader.read_exact(&mut body).ok();
                    }
                    let mut stream = reader.into_inner();
                    let response = b"HTTP/1.1 200 OK\r\ncontent-type: \
                                     application/json\r\ncontent-length: \
                                     2\r\nconnection: close\r\n\r\n{}";
                    let ok = stream.write_all(response).is_ok()
                        && stream.flush().is_ok();
                    if ok {
                        hits_child.fetch_add(1, Ordering::SeqCst);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break
            }
        }
    });
    (format!("http://{addr}"), hits, handle)
}

#[test]
fn test_flush_delivers_queued_events() {
    use std::sync::atomic::Ordering;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let (host, hits, server) = start_stub_posthog(2);

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-flush-test");
    ctx.set_env_var("POSTHOG_API_HOST", host);
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };
    telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
        .expect("queueing should not error");
    telemetry::posthog_capture_dau_ping().expect("dau queueing");
    assert_eq!(telemetry::queue::list_queued().len(), 2);

    let summary = telemetry::queue::flush_queue();
    assert_eq!(summary.sent, 2, "both files should deliver");
    assert_eq!(summary.failed, 0);
    assert!(
        telemetry::queue::list_queued().is_empty(),
        "delivered files must be deleted"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    server.join().expect("stub server should finish");
}

#[test]
fn test_flush_failure_keeps_files_for_retry() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-flush-test");
    // - Discard port: connections refuse fast, delivery fails.
    ctx.set_env_var("POSTHOG_API_HOST", "http://127.0.0.1:9");
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };
    telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
        .expect("queueing should not error");

    let summary = telemetry::queue::flush_queue();
    assert_eq!(summary.sent, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(
        telemetry::queue::list_queued().len(),
        1,
        "failed file must stay queued for the next flush"
    );
}

#[test]
fn test_flush_skips_while_opted_out() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-flush-test");
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    telemetry::posthog_capture_dau_ping().expect("dau queueing");
    assert_eq!(telemetry::queue::list_queued().len(), 1);

    // - Opt out after queueing: flush must leave the file alone.
    let cfg = types::Config {
        telemetry_enabled: false,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let summary = telemetry::queue::flush_queue();
    assert_eq!(summary.sent, 0);
    assert_eq!(
        telemetry::queue::list_queued().len(),
        1,
        "opted-out flush must keep queued files"
    );
}

#[test]
fn test_corrupt_queue_file_is_dropped_on_flush() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let (host, _hits, server) = start_stub_posthog(1);

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-flush-test");
    ctx.set_env_var("POSTHOG_API_HOST", host);
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    telemetry::posthog_capture_dau_ping().expect("dau queueing");
    let dir = zoi_core::utils::get_user_state_dir()
        .expect("state dir should resolve")
        .join("telemetry")
        .join("queue");
    std::fs::write(dir.join("corrupt.json"), "definitely not json{{{")
        .expect("corrupt file should be writable");

    let summary = telemetry::queue::flush_queue();
    assert_eq!(summary.sent, 1, "good file should deliver");
    assert_eq!(summary.skipped, 1, "corrupt file should be dropped");
    assert!(
        telemetry::queue::list_queued().is_empty(),
        "nothing should remain queued"
    );
    server.join().expect("stub server should finish");
}

#[test]
fn test_queue_caps_oldest_files() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("POSTHOG_API_KEY", "fake-key-for-queue-test");
    ctx.set_env_var("ZOI_TELEMETRY_NO_SPAWN", "1");
    common::TestContextGuard::set_sysroot(root.clone());

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };
    for _ in 0..105 {
        telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None)
            .expect("queueing should not error");
    }
    assert_eq!(
        telemetry::queue::list_queued().len(),
        100,
        "queue must be capped at MAX_QUEUE_FILES"
    );
}
