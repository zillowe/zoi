//! Integration tests for Zoi process-wide recursive locking.

use tempfile::tempdir;
use zoi::pkg::lock;

mod common;

#[test]
fn test_recursive_locking_in_same_thread() {
    let mut ctx = common::TestContextGuard::acquire();
    let home = tempdir().expect("temporary home should be created");
    ctx.set_env_var("HOME", home.path());

    // First acquisition
    let guard1 =
        lock::acquire_lock().expect("First acquisition should succeed");

    // Second acquisition (recursive)
    let guard2 = lock::acquire_lock()
        .expect("Second acquisition should succeed (recursive)");

    // Third acquisition (recursive)
    let guard3 =
        lock::acquire_lock().expect("Third acquisition should succeed");

    // Dropping guards should not release the file lock until the last one is
    // dropped
    drop(guard3);
    drop(guard2);

    // We should still be able to perform operations that require the lock
    // (In this test we just verify we can still hold guard1)

    drop(guard1);

    // Now the lock should be fully released. We should be able to acquire it
    // again.
    let _guard4 = lock::acquire_lock()
        .expect("Acquisition after full release should succeed");
}

#[test]
fn test_lock_skipping_via_env() {
    let mut ctx = common::TestContextGuard::acquire();
    ctx.set_env_var("ZOI_SKIP_LOCK", "1");

    let guard = lock::acquire_lock().expect("Should succeed with skip enabled");
    // This should return a no-op guard that doesn't actually create a file or
    // block

    let guard2 = lock::acquire_lock()
        .expect("Should succeed recursively even with skip");
    drop(guard2);
    drop(guard);
}
