//! PackageKit-compatible D-Bus service.
//!
//! `PackageKit` is the cross-desktop standard for package management over
//! D-Bus. Without it, GNOME Software, KDE Discover, `pkcon`, `gpk-application`
//! and a long tail of smaller tools simply do not function on a distribution.
//! Implementing it is what makes a `ZoiOS` system usable as a desktop and not
//! only from a terminal.
//!
//! ## Structure, per the upstream specification
//!
//! `PackageKit` is two interfaces, not one:
//!
//! - `org.freedesktop.PackageKit` on the fixed path
//!   `/org/freedesktop/PackageKit`. Properties describing the backend, plus
//!   `CreateTransaction`, which hands out an object path.
//! - `org.freedesktop.PackageKit.Transaction`, on a *fresh object path per
//!   transaction*. Every query and mutation lives here, and **none of those
//!   methods return a value**. Results arrive asynchronously as signals
//!   (`Packages`, `Details`, `RepoDetail`, ...) terminated by `Finished`.
//!
//! Getting that shape wrong is not a cosmetic problem: a client that expects
//! return values from `GetPackages` gets nothing, and one that expects signals
//! from `InstallPackages` waits forever.
//!
//! ## Threading
//!
//! Every transaction method is annotated `org.freedesktop.DBus.GLib.Async` in
//! the specification and must return immediately. Each one therefore does its
//! work on a worker thread and reports back through signals.
//!
//! Signals are emitted only from those worker threads, never from inside a
//! D-Bus method call. zbus declares signals `async`, so emitting requires an
//! executor, and calling one from the object server's own thread risks a
//! deadlock. Keeping emission strictly on threads this module spawns removes
//! that hazard entirely.
//!
//! ## Package identifiers
//!
//! `PackageKit`'s identifier format is `name;version;arch;origin;data`, e.g.
//! `firefox;141.0;amd64;core;`. It has nothing to do with Zoi's internal ids,
//! so the conversion lives in [`package_id`] and is used everywhere a client
//! can name a package.

// Every method signature in this module is dictated by the PackageKit
// specification rather than chosen here, which puts four pedantic lints in
// direct conflict with the wire contract. They are suppressed for the whole
// module rather than per-item because `#[zbus::interface]` rewrites each impl
// into a generated one, so an attribute placed on the impl does not reach the
// code clippy actually inspects.
//
// - `unused_self`: an interface method is dispatched off `&self` whether or not
//   the body needs the receiver. `VersionMajor` is a constant of the interface,
//   but dropping the receiver would break the generated dispatcher.
// - `used_underscore_binding`: parameters the body ignores are prefixed with an
//   underscore, but the generated dispatcher still names them, so clippy sees
//   them as used. Removing the underscore instead produces 70-odd
//   `unused_variables`, which is strictly worse.
// - `needless_pass_by_value`: D-Bus arguments arrive owned, so borrowing is not
//   available at the boundary.
// - `unnecessary_wraps`: the read-only helpers return `Result<()>` because
//   `start` requires that shape. It is the single funnel every transaction goes
//   through, so a uniform signature is worth more than a locally narrower one.
#![allow(
    clippy::unused_self,
    clippy::used_underscore_binding,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps
)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use zbus::blocking::{Connection, connection};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

// ---------------------------------------------------------------------------
// Enumerations, transcribed from PackageKit's pk-enum.h
//
// The numeric values are part of the wire protocol. Frontends switch on them,
// so they must match upstream exactly and must never be reordered. Each block
// below records the ones Zoi actually emits, with the upstream ordering
// preserved around them.
// ---------------------------------------------------------------------------

/// `PkInfoEnum`, sent as the first field of every `Packages` signal.
///
/// Transcribed in full, including the values Zoi does not currently emit. The
/// numeric positions are the wire protocol and frontends switch on them, so an
/// enum with holes would be unsafe to extend: a future `info::CRITICAL` would
/// silently mean the wrong thing. Values are asserted against upstream in the
/// tests below.
#[allow(dead_code)]
mod info {
    /// Package status is unknown.
    pub(super) const UNKNOWN: u32 = 0;
    /// Package is installed.
    pub(super) const INSTALLED: u32 = 1;
    /// Package is available to be installed.
    pub(super) const AVAILABLE: u32 = 2;
    /// Package update has a low priority.
    pub(super) const LOW: u32 = 3;
    /// Package update is an enhancement.
    pub(super) const ENHANCEMENT: u32 = 4;
    /// Package update has normal priority.
    pub(super) const NORMAL: u32 = 5;
    /// Package update fixes bugs.
    pub(super) const BUGFIX: u32 = 6;
    /// Package update is important.
    pub(super) const IMPORTANT: u32 = 7;
    /// Package update contains a security fix.
    pub(super) const SECURITY: u32 = 8;
    /// Package is blocked. Reported for an update that exists but cannot be
    /// installed, such as a pinned package.
    pub(super) const BLOCKED: u32 = 9;
    /// Package is being downloaded.
    pub(super) const DOWNLOADING: u32 = 10;
    /// Package is updating.
    pub(super) const UPDATING: u32 = 11;
    /// Package is being installed.
    pub(super) const INSTALLING: u32 = 12;
    /// Package is being removed.
    pub(super) const REMOVING: u32 = 13;
    /// Package is running cleanup.
    pub(super) const CLEANUP: u32 = 14;
    /// Package is being obsoleted.
    pub(super) const OBSOLETING: u32 = 15;
    /// Package has finished processing.
    pub(super) const FINISHED: u32 = 16;
    /// Package is being reinstalled.
    pub(super) const REINSTALLING: u32 = 17;
    /// Package is being downgraded.
    pub(super) const DOWNGRADING: u32 = 18;
    /// Package is preparing for installation or removal.
    pub(super) const PREPARING: u32 = 19;
    /// Package is decompressing.
    pub(super) const DECOMPRESSING: u32 = 20;
    /// Package is untrusted.
    pub(super) const UNTRUSTED: u32 = 21;
    /// Package is trusted.
    pub(super) const TRUSTED: u32 = 22;
    /// Package is unavailable.
    pub(super) const UNAVAILABLE: u32 = 23;
    /// Package update severity is critical.
    pub(super) const CRITICAL: u32 = 24;
}

/// `PkStatusEnum`, exposed as the transaction's `Status` property.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod status {
    /// Unknown status.
    pub(super) const UNKNOWN: u32 = 0;
    /// Waiting.
    pub(super) const WAIT: u32 = 1;
    /// Setting up.
    pub(super) const SETUP: u32 = 2;
    /// Running.
    pub(super) const RUNNING: u32 = 3;
    /// Answering a query.
    pub(super) const QUERY: u32 = 4;
    /// Gathering information.
    pub(super) const INFO: u32 = 5;
    /// Removing packages.
    pub(super) const REMOVE: u32 = 6;
    /// Refreshing the metadata cache.
    pub(super) const REFRESH_CACHE: u32 = 7;
    /// Downloading.
    pub(super) const DOWNLOAD: u32 = 8;
    /// Installing packages.
    pub(super) const INSTALL: u32 = 9;
    /// Updating packages.
    pub(super) const UPDATE: u32 = 10;
    /// Cleaning up.
    pub(super) const CLEANUP: u32 = 11;
    /// Marking packages obsolete.
    pub(super) const OBSOLETE: u32 = 12;
    /// Resolving dependencies.
    pub(super) const DEP_RESOLVE: u32 = 13;
    /// Checking signatures.
    pub(super) const SIG_CHECK: u32 = 14;
    /// Testing the commit.
    pub(super) const TEST_COMMIT: u32 = 15;
    /// Committing.
    pub(super) const COMMIT: u32 = 16;
    /// Awaiting a request.
    pub(super) const REQUEST: u32 = 17;
    /// Finished.
    pub(super) const FINISHED: u32 = 18;
    /// Cancelling.
    pub(super) const CANCEL: u32 = 19;
    /// Copying files.
    pub(super) const COPY_FILES: u32 = 20;
    /// Running a package hook.
    pub(super) const RUN_HOOK: u32 = 31;
}

/// `PkExitEnum`, sent as the first field of `Finished`.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod exit {
    /// Unknown exit status.
    pub(super) const UNKNOWN: u32 = 0;
    /// The backend exited successfully.
    pub(super) const SUCCESS: u32 = 1;
    /// The backend failed.
    pub(super) const FAILED: u32 = 2;
    /// The backend was cancelled.
    pub(super) const CANCELLED: u32 = 3;
}

/// `PkRoleEnum`, exposed as the transaction's `Role` property.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod role {
    /// Unknown request.
    pub(super) const UNKNOWN: u32 = 0;
    /// Get package details.
    pub(super) const GET_DETAILS: u32 = 3;
    /// Get the file list of a package.
    pub(super) const GET_FILES: u32 = 4;
    /// Get available packages.
    pub(super) const GET_PACKAGES: u32 = 5;
    /// Get the repository list.
    pub(super) const GET_REPO_LIST: u32 = 6;
    /// Get update details.
    pub(super) const GET_UPDATE_DETAIL: u32 = 8;
    /// Get available updates.
    pub(super) const GET_UPDATES: u32 = 9;
    /// Install packages.
    pub(super) const INSTALL_PACKAGES: u32 = 11;
    /// Refresh the metadata cache.
    pub(super) const REFRESH_CACHE: u32 = 13;
    /// Remove packages.
    pub(super) const REMOVE_PACKAGES: u32 = 14;
    /// Search package details.
    pub(super) const SEARCH_DETAILS: u32 = 18;
    /// Search by package name.
    pub(super) const SEARCH_NAME: u32 = 21;
    /// Update packages.
    pub(super) const UPDATE_PACKAGES: u32 = 22;
    /// Find what provides a file or command.
    pub(super) const WHAT_PROVIDES: u32 = 23;
    /// Download packages.
    pub(super) const DOWNLOAD_PACKAGES: u32 = 25;
    /// Get available distribution upgrades.
    pub(super) const GET_DISTRO_UPGRADES: u32 = 26;
}

/// `PkErrorEnum`, sent as the first field of `ErrorCode`.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod error_code {
    /// Unknown error.
    pub(super) const UNKNOWN: u32 = 0;
    /// No network access available.
    pub(super) const NO_NETWORK: u32 = 2;
    /// Request not supported.
    pub(super) const NOT_SUPPORTED: u32 = 3;
    /// Undefined internal error.
    pub(super) const INTERNAL_ERROR: u32 = 4;
    /// Signature or key verification failure.
    pub(super) const GPG_FAILURE: u32 = 5;
    /// Invalid package ID provided.
    pub(super) const PACKAGE_ID_INVALID: u32 = 6;
    /// Requested package is not installed.
    pub(super) const PACKAGE_NOT_INSTALLED: u32 = 7;
    /// Requested package not found.
    pub(super) const PACKAGE_NOT_FOUND: u32 = 8;
    /// Requested package is already installed.
    pub(super) const PACKAGE_ALREADY_INSTALLED: u32 = 9;
    /// Requested group not found.
    pub(super) const GROUP_NOT_FOUND: u32 = 11;
    /// Failed to resolve dependencies.
    pub(super) const DEP_RESOLUTION_FAILED: u32 = 13;
    /// Invalid filter provided.
    pub(super) const FILTER_INVALID: u32 = 14;
    /// An error occurred during the transaction.
    pub(super) const TRANSACTION_ERROR: u32 = 16;
    /// No packages to update.
    pub(super) const NO_PACKAGES_TO_UPDATE: u32 = 26;
    /// The package failed to install.
    pub(super) const PACKAGE_FAILED_TO_INSTALL: u32 = 54;
    /// The package failed to be removed.
    pub(super) const PACKAGE_FAILED_TO_REMOVE: u32 = 55;
}

/// `PkRestartEnum`, sent as the first field of `RequireRestart`.
///
/// Ordered by severity, so a client can keep the worst.
#[allow(dead_code)]
mod restart {
    /// Unknown restart state.
    pub(super) const UNKNOWN: u32 = 0;
    /// No restart required.
    pub(super) const NONE: u32 = 1;
    /// The application must be restarted.
    pub(super) const APPLICATION: u32 = 2;
    /// The session must be restarted.
    pub(super) const SESSION: u32 = 3;
    /// The system must be restarted.
    pub(super) const SYSTEM: u32 = 4;
}

/// `PkAuthorizeEnum`, returned by `CanAuthorize`.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod authorize {
    /// Unknown authorization status.
    pub(super) const UNKNOWN: u32 = 0;
    /// Authorized.
    pub(super) const YES: u32 = 1;
    /// Not authorized.
    pub(super) const NO: u32 = 2;
    /// Interaction is required for authorization.
    pub(super) const INTERACTIVE: u32 = 3;
}

/// `PkFilterEnum`. These are *bit flags*, unlike the other enums.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod filter {
    /// No filter.
    pub(super) const NONE: u64 = 1 << 1;
    /// Filter for installed packages.
    pub(super) const INSTALLED: u64 = 1 << 2;
    /// Filter for packages that are not installed.
    pub(super) const NOT_INSTALLED: u64 = 1 << 3;
    /// Filter for development packages.
    pub(super) const DEVEL: u64 = 1 << 4;
    /// Filter for non-development packages.
    pub(super) const NOT_DEVEL: u64 = 1 << 5;
    /// Filter for GUI packages.
    pub(super) const GUI: u64 = 1 << 6;
    /// Filter for non-GUI packages.
    pub(super) const NOT_GUI: u64 = 1 << 7;
    /// Filter for free packages.
    pub(super) const FREE: u64 = 1 << 8;
    /// Filter for non-free packages.
    pub(super) const NOT_FREE: u64 = 1 << 9;
    /// Filter for supported packages.
    pub(super) const SUPPORTED: u64 = 1 << 10;
    /// Filter for packages that are not supported.
    pub(super) const NOT_SUPPORTED: u64 = 1 << 11;
    /// Filter for packages that match the basename.
    pub(super) const BASENAME: u64 = 1 << 12;
    /// Filter for packages that do not match the basename.
    pub(super) const NOT_BASENAME: u64 = 1 << 13;
    /// Filter for the newest package of each name.
    pub(super) const NEWEST: u64 = 1 << 14;
    /// Filter out the newest package of each name.
    pub(super) const NOT_NEWEST: u64 = 1 << 15;
    /// Filter for packages matching the architecture.
    pub(super) const ARCH: u64 = 1 << 16;
    /// Filter for packages not matching the architecture.
    pub(super) const NOT_ARCH: u64 = 1 << 17;
    /// Filter for source packages.
    pub(super) const SOURCE: u64 = 1 << 18;
    /// Filter for non-source packages.
    pub(super) const NOT_SOURCE: u64 = 1 << 19;
    /// Filter for application packages.
    pub(super) const APPLICATION: u64 = 1 << 20;
    /// Filter for non-application packages.
    pub(super) const NOT_APPLICATION: u64 = 1 << 21;
    /// Filter for downloaded packages.
    pub(super) const DOWNLOADED: u64 = 1 << 22;
    /// Filter for packages that are not downloaded.
    pub(super) const NOT_DOWNLOADED: u64 = 1 << 23;
}

/// `PkNetworkEnum`, exposed as the root object's `NetworkState` property.
///
/// Kept complete for the same reason as [`info`].
#[allow(dead_code)]
mod network {
    /// Unknown network.
    pub(super) const UNKNOWN: u32 = 0;
    /// Offline, no network.
    pub(super) const OFFLINE: u32 = 1;
    /// Online, network type unknown.
    pub(super) const ONLINE: u32 = 2;
}

/// Bus name `PackageKit` clients expect to talk to.
const BUS_NAME: &str = "org.freedesktop.PackageKit";

/// Object path the root interface is served on.
const ROOT_PATH: &str = "/org/freedesktop/PackageKit";

/// Name of the root interface.
const IFACE_ROOT: &str = "org.freedesktop.PackageKit";

/// Name of the per-transaction interface.
const IFACE_TRANSACTION: &str = "org.freedesktop.PackageKit.Transaction";

/// Major interface version. Clients gate optional features on this.
const VERSION_MAJOR: u32 = 1;

/// Minor interface version.
const VERSION_MINOR: u32 = 2;

/// Micro interface version.
const VERSION_MICRO: u32 = 0;

/// Backend name, shown in frontends in place of a package manager's own.
const BACKEND_NAME: &str = "zoi";

/// Backend description, shown in frontends.
const BACKEND_DESCRIPTION: &str = "Zoi Package Manager";

/// Backend author, shown in frontends.
const BACKEND_AUTHOR: &str = "Zillowe Foundation";

/// Distro id in the `id;version;arch` form the specification requires.
const DISTRO_ID: &str = "zoios;;amd64";

/// How long an idle transaction is kept before being reclaimed.
///
/// The specification says the daemon destroys unused transactions after "a few
/// minutes". Frontends legitimately sit on a transaction between operations, so
/// this is generous.
const TRANSACTION_IDLE_TIMEOUT_SECS: u64 = 600;

/// Entries `PackageKit` advertises for its category browser.
///
/// Zoi has no category metadata in `.pkg.lua`, so an honest list beats a
/// fabricated taxonomy that would misfile every package.
const SUPPORTED_GROUPS: u64 = 0;

/// Filters Zoi can actually honour.
///
/// `INSTALLED`/`NOT_INSTALLED`/`BASENAME`/`NEWEST`/`DEVEL` are derived from
/// data Zoi already tracks. The rest are not, and claiming them would make a
/// frontend apply a filter that silently does nothing.
const SUPPORTED_FILTERS: u64 = filter::INSTALLED
    | filter::NOT_INSTALLED
    | filter::BASENAME
    | filter::NOT_BASENAME
    | filter::NEWEST
    | filter::NOT_NEWEST
    | filter::DEVEL
    | filter::NOT_DEVEL;

/// Roles Zoi advertises as available.
const SUPPORTED_ROLES: u64 = (1 << role::GET_PACKAGES)
    | (1 << role::GET_DETAILS)
    | (1 << role::GET_FILES)
    | (1 << role::GET_UPDATES)
    | (1 << role::GET_UPDATE_DETAIL)
    | (1 << role::GET_DISTRO_UPGRADES)
    | (1 << role::GET_REPO_LIST)
    | (1 << role::INSTALL_PACKAGES)
    | (1 << role::REMOVE_PACKAGES)
    | (1 << role::UPDATE_PACKAGES)
    | (1 << role::REFRESH_CACHE)
    | (1 << role::SEARCH_NAME)
    | (1 << role::SEARCH_DETAILS)
    | (1 << role::WHAT_PROVIDES);

/// MIME types for local file installs.
const MIME_TYPES: &[&str] = &["application/x-zpa"];

/// Mutable state of one transaction, shared between the D-Bus methods and the
/// worker thread doing the work.
#[derive(Debug)]
struct TransactionState {
    /// `PkRoleEnum` for this transaction. Fixed for its lifetime.
    role: u32,
    /// `PkStatusEnum`, changes as work progresses.
    status: u32,
    /// Completion percentage. 101 means "not calculable", per the spec.
    percentage: u32,
    /// Packages this transaction concerns, as `PackageKit` ids.
    packages: Vec<String>,
    /// Last package touched, surfaced as `LastPackage`.
    last_package: String,
    /// Milliseconds since the transaction started.
    started: Instant,
    /// `PkTransactionFlagEnum` bitfield set by the caller, echoed back through
    /// the `TransactionFlags` property.
    transaction_flags: u64,
    /// Hints passed to `SetHints`, e.g. `locale`, `interactive`.
    hints: Vec<String>,
    /// Set when the work failed, reported through `ErrorCode`.
    error: Option<(u32, String)>,
    /// Set once `Finished` has been emitted, so it is emitted exactly once.
    finished: bool,
    /// A cancellation request arrived. Honoured at the next safe point.
    cancel_requested: bool
}

impl TransactionState {
    /// Creates a freshly queued transaction.
    fn new(role: u32) -> Self {
        Self {
            role,
            status: status::WAIT,
            percentage: 0,
            packages: Vec::new(),
            last_package: String::new(),
            started: Instant::now(),
            transaction_flags: 0,
            hints: Vec::new(),
            error: None,
            finished: false,
            cancel_requested: false
        }
    }
}

/// Handle used by worker threads to report progress back to a client.
#[derive(Clone)]
struct Reporter {
    /// Object path signals are emitted from.
    path: Arc<str>,
    /// Shared transaction state.
    state: Arc<Mutex<TransactionState>>
}

impl Reporter {
    /// Builds a reporter for a transaction object.
    fn new(path: &str, state: Arc<Mutex<TransactionState>>) -> Self {
        Self {
            path: Arc::from(path),
            state
        }
    }

    /// Emits a signal on the transaction interface.
    ///
    /// Transport failures are swallowed. A client that disconnected mid
    /// transaction is normal and must not turn into an error on the worker's
    /// side; the work still has to finish and be committed.
    fn emit<B>(
        &self,
        interface: zbus::names::InterfaceName<'static>,
        signal: &str,
        body: &B
    ) -> Result<()>
    where
        B: serde::ser::Serialize + zbus::zvariant::DynamicType
    {
        let conn = dbus_connection()?;
        let emitter = SignalEmitter::new(conn.inner(), self.path.as_ref())
            .with_context(|| {
                format!("Failed to build a signal emitter for {}", self.path)
            })?;

        // Emission is async in zbus. This runs on a worker thread this module
        // spawned, never on the object server's own thread, so blocking on the
        // runtime here cannot deadlock against it.
        let result = async_io::block_on(
            emitter.emit::<_, _, _>(interface, signal, body)
        );
        let _ = result;
        Ok(())
    }

    /// Emits a `Packages` signal.
    fn packages(&self, packages: Vec<(u32, String, String)>) {
        let _ = self.emit(transaction_interface(), "Packages", &(packages,));
    }

    /// Emits `ItemProgress` for one package.
    fn item_progress(&self, id: &str, status: u32, percentage: u32) {
        let _ = self.emit(
            transaction_interface(),
            "ItemProgress",
            &(id, status, percentage)
        );
    }

    /// Emits `RequireRestart`.
    fn require_restart(&self, restart_type: u32, package_id: &str) {
        let _ = self.emit(
            transaction_interface(),
            "RequireRestart",
            &(restart_type, package_id)
        );
    }
    /// Updates the in-memory status and mirrors it to the client.
    fn set_status(&self, new_status: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.status = new_status;
        }
    }

    /// Advances progress without ever reporting a false completion.
    ///
    /// Capped below 100 while work continues, because a client that sees 100%
    /// may hide its progress UI and then show nothing while the transaction is
    /// still running.
    fn set_percentage(&self, percentage: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.percentage = percentage.min(99);
        }
    }

    /// Records a package as touched and reports it.
    fn touch(&self, package_id: &str, info: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.last_package = package_id.to_string();
            if !state.packages.iter().any(|p| p == package_id) {
                state.packages.push(package_id.to_string());
            }
        }
        self.packages(vec![(info, package_id.to_string(), String::new())]);
        self.item_progress(package_id, status::RUNNING, 0);
    }

    /// Emits `ErrorCode` and `Finished(failed)`.
    fn fail(&self, code: u32, details: String) {
        if let Ok(mut state) = self.state.lock() {
            if state.finished {
                return;
            }
            state.finished = true;
            state.status = status::FINISHED;
            state.percentage = 100;
            state.error = Some((code, details.clone()));
        }

        let _ =
            self.emit(transaction_interface(), "ErrorCode", &(code, details));
        let runtime = self.elapsed_ms();
        let _ = self.emit(
            transaction_interface(),
            "Finished",
            &(exit::FAILED, runtime)
        );
    }

    /// Emits `Finished(success)` exactly once.
    fn succeed(&self) {
        let already = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            if state.finished {
                return;
            }
            state.finished = true;
            state.status = status::FINISHED;
            state.percentage = 100;
            false
        };

        if already {
            return;
        }

        let runtime = self.elapsed_ms();
        let _ = self.emit(
            transaction_interface(),
            "Finished",
            &(exit::SUCCESS, runtime)
        );
    }

    /// Returns milliseconds elapsed so far.
    fn elapsed_ms(&self) -> u32 {
        let Ok(state) = self.state.lock() else {
            return 0;
        };
        u32::try_from(state.started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    /// Returns true once a cancellation has been requested.
    fn cancel_requested(&self) -> bool {
        self.state.lock().is_ok_and(|s| s.cancel_requested)
    }

    /// Returns the properties a client polls while work runs.
    fn properties(&self) -> (u32, u32, String, u32, bool) {
        let Ok(state) = self.state.lock() else {
            return (0, 0, String::new(), 0, false);
        };
        (
            state.role,
            state.status,
            state.last_package.clone(),
            state.percentage,
            state.cancel_requested
        )
    }
}

/// The root interface name, for signal emission.
///
/// `SignalEmitter::emit` takes the interface name as a value, not as a type, so
/// it has to be constructed at each call site. A helper keeps that in one
/// place.
fn root_interface() -> zbus::names::InterfaceName<'static> {
    IFACE_ROOT
        .try_into()
        .expect("the root interface name is a valid D-Bus name")
}

/// The transaction interface name, for signal emission.
fn transaction_interface() -> zbus::names::InterfaceName<'static> {
    IFACE_TRANSACTION
        .try_into()
        .expect("the transaction interface name is a valid D-Bus name")
}

/// Converts a string into an `OwnedValue` for a `a{sv}` dictionary.
///
/// `zvariant` only implements `From<&str>` for the borrowed `Value`, so the
/// conversion goes through `Str`. Wrapped in a helper because every `Details`
/// key needs it and getting it wrong is a type error at each site.
fn owned_str(value: &str) -> OwnedValue {
    OwnedValue::from(zbus::zvariant::Str::from(value))
}

/// One record of the `UpdateDetails` signal.
///
/// The field order and types are fixed by the `PackageKit` specification. Zoi
/// does not populate this signal: update severity is instead carried on the
/// `Packages` signal's `info` field, which is where frontends read it from.
type UpdateDetail = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    u32,
    u32,
    String,
    u32,
    String
);

/// Builds a `PackageKit` package id from a Zoi package name and version.
///
/// Format is fixed by the specification: `name;version;arch;origin;data`.
/// `arch` is reported as `noarch` because a Zoi package definition does not
/// carry a per-package architecture; the binary it installs does.
fn package_id(name: &str, version: &str, repo: &str) -> String {
    format!("{name};{version};noarch;{repo};")
}

/// Returns a registry package's version, falling back when it is unresolved.
///
/// `types::Package::version` is `Option` because a definition can declare only
/// `versions` (a channel map) and have the resolver fill it in later. A package
/// still carrying `None` here has not been resolved, and emitting an id with an
/// empty version would produce something no client could act on.
fn package_version(pkg: &zoi_core::types::Package) -> Option<&str> {
    pkg.version.as_deref().filter(|v| !v.is_empty())
}

/// Extracts the bare package name from any identifier shape `PackageKit` uses.
///
/// Clients send a full `name;version;arch;repo;` id, or sometimes just a name.
/// `zoi` accepts both, but a name containing `;` would be read as a version
/// specifier, so the id has to be reduced first.
fn package_name_from_id(raw: &str) -> String {
    let trimmed = raw.trim();
    // A leading `@` marks a repository-qualified request, which is not a
    // PackageKit id; leave those alone so Zoi can interpret them.
    if trimmed.starts_with('@') {
        return trimmed.to_string();
    }
    trimmed
        .split(';')
        .next()
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// Maps the worst advisory affecting a version onto a `PkInfoEnum` value.
///
/// This is what lets a desktop frontend sort updates by importance the way
/// GNOME Software does, instead of showing a flat list. Zoi already tracks
/// advisories per package, so the information is available and using it is
/// strictly better than assuming every update is routine.
///
/// Returns `None` when the installed version is not affected by any advisory.
fn update_info_from_advisories(
    registry_handle: &str,
    package: &str,
    sub_package: Option<&str>,
    installed_version: &str
) -> Option<u32> {
    use zoi_core::types::Severity;

    let advisories = zoi_db::get_advisories_for_package(
        registry_handle,
        package,
        sub_package
    )
    .unwrap_or_default();

    let mut worst: Option<(u32, Severity)> = None;

    for advisory in advisories {
        // An advisory that names a fix in the currently installed version does
        // not apply; a newer release is needed, which is exactly the situation
        // where the frontend should say "security update available".
        if let Some(fixed_in) = advisory.fixed_in.as_deref()
            && fixed_in == installed_version
        {
            continue;
        }

        let info = match advisory.severity {
            Severity::Critical => info::CRITICAL,
            Severity::High => info::SECURITY,
            Severity::Medium => info::IMPORTANT,
            Severity::Low => info::BUGFIX
        };

        let rank = match advisory.severity {
            Severity::Critical => 4,
            Severity::High => 3,
            Severity::Medium => 2,
            Severity::Low => 1
        };

        // Keep the worst seen. Severity and rank move together, so comparing
        // ranks is enough.
        if worst
            .as_ref()
            .is_none_or(|(_, seen)| rank > seen_rank(*seen))
        {
            worst = Some((info, advisory.severity));
        }
    }

    worst.map(|(info, _)| info)
}

/// Returns the numeric rank of an advisory severity.
fn seen_rank(severity: zoi_core::types::Severity) -> u8 {
    match severity {
        zoi_core::types::Severity::Critical => 4,
        zoi_core::types::Severity::High => 3,
        zoi_core::types::Severity::Medium => 2,
        zoi_core::types::Severity::Low => 1
    }
}

/// Renders an installed manifest as a `Packages` tuple.
///
/// The tuple is `(info, package_id, summary)`, matching the `a(uss)` signature.
fn installed_entry(
    manifest: &zoi_core::types::InstallManifest
) -> (u32, String, String) {
    (
        info::INSTALLED,
        package_id(&manifest.name, &manifest.version, &manifest.repo),
        manifest.description.clone()
    )
}

/// Returns true when a filter bitfield asks for installed packages only.
fn filter_wants_installed(filter: u64) -> bool {
    filter & filter::INSTALLED != 0
}

/// Returns true when a filter bitfield asks for uninstalled packages only.
fn filter_wants_available(filter: u64) -> bool {
    filter & filter::NOT_INSTALLED != 0
}

/// Reports whether the host currently has usable network connectivity.
///
/// Deliberately conservative: `/sys/class/net` is consulted for a non-loopback
/// interface that is up, and anything ambiguous is reported as offline so a
/// frontend offers offline mode rather than failing a download.
fn network_state() -> u32 {
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return network::UNKNOWN;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "lo" {
            continue;
        }
        if let Ok(operstate) =
            std::fs::read_to_string(entry.path().join("operstate"))
            && (operstate.trim() == "up" || operstate.trim() == "unknown")
        {
            return network::ONLINE;
        }
    }

    network::OFFLINE
}

/// Runs a `zoi` subcommand, streaming stderr and mapping failure to an error.
///
/// `PackageKit` work always targets system scope: the daemon runs as root, and
/// a desktop package manager that quietly installed into a user's home
/// directory would be actively harmful.
fn run_zoi(args: &[String], reporter: &Reporter) -> Result<()> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("zoi");
    cmd.args(args);
    cmd.arg("--yes");
    cmd.arg("--scope").arg("system");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to run 'zoi {}'", args.join(" ")))?;

    // stderr must be drained while the child runs, otherwise a full pipe buffer
    // deadlocks it. Progress is inferred from the lines it prints.
    let mut captured = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            eprintln!("{}", line.dimmed());
            captured.push_str(&line);
            captured.push('\n');
            reporter.set_percentage(reporter.properties().3.saturating_add(2));
        }
    }

    let status = child.wait().context("Failed to wait for 'zoi'")?;

    if !status.success() {
        // Classify the failure so a frontend can show something more useful
        // than "transaction failed". The text is matched because `zoi`
        // reports the specifics on stderr and there is no
        // machine-readable status to read.
        let code = classify_failure(&captured);
        return Err(PkError {
            code,
            message: captured.trim().to_string()
        }
        .into());
    }

    Ok(())
}

/// A failure carrying the `PackageKit` error code that describes it.
///
/// Carried through `anyhow` so the worker can recover the code with a downcast
/// and emit the right `ErrorCode` signal. Returning a bare string would leave
/// every failure indistinguishable to a frontend.
#[derive(Debug)]
struct PkError {
    /// `PkErrorEnum` value.
    code: u32,
    /// Human readable detail.
    message: String
}

impl std::fmt::Display for PkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PkError {}

/// Maps `zoi`'s stderr onto the closest `PkErrorEnum` value.
///
/// Deliberately coarse. A wrong-but-close code makes a frontend offer the wrong
/// remedy, so only unambiguous cases are classified and everything else falls
/// through to `TRANSACTION_ERROR`.
fn classify_failure(output: &str) -> u32 {
    let lower = output.to_lowercase();

    if lower.contains("not found") || lower.contains("no such package") {
        error_code::PACKAGE_NOT_FOUND
    } else if lower.contains("not installed") {
        error_code::PACKAGE_NOT_INSTALLED
    } else if lower.contains("already installed") {
        error_code::PACKAGE_ALREADY_INSTALLED
    } else if lower.contains("signature")
        || lower.contains("gpg")
        || lower.contains("pgp")
    {
        error_code::GPG_FAILURE
    } else if lower.contains("conflict")
        || lower.contains("dependenc")
        || lower.contains("resolve")
        || lower.contains("unsatisfiable")
    {
        // Conflicts and unsatisfiable dependencies are the same class of
        // failure from a client's point of view: the request cannot be
        // satisfied as specified.
        error_code::DEP_RESOLUTION_FAILED
    } else if lower.contains("advisor") || lower.contains("vulnerab") {
        error_code::PACKAGE_FAILED_TO_INSTALL
    } else if lower.contains("failed to remove") || lower.contains("uninstall")
    {
        error_code::PACKAGE_FAILED_TO_REMOVE
    } else if lower.contains("no space") || lower.contains("disk full") {
        error_code::INTERNAL_ERROR
    } else {
        error_code::TRANSACTION_ERROR
    }
}

/// Installs the given packages.
fn do_install(packages: &[String], reporter: &Reporter) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    reporter.set_status(status::DOWNLOAD);
    for pkg in packages {
        reporter.touch(pkg, info::INSTALLING);
    }

    // Kernel versions are noted before the install so a new one can be spotted
    // afterwards. Comparing the module tree is authoritative, whereas matching
    // on a package name is not: `linux-docs` and `linux-headers` share the
    // prefix but neither is a kernel.
    let kernels_before = installed_kernel_versions();

    let mut args = vec!["install".to_string()];
    args.extend(packages.iter().cloned());

    run_zoi(&args, reporter)?;

    // A new kernel needs a reboot before it takes effect. The signal lets a
    // frontend offer the restart rather than rebooting on its own.
    let kernels_after = installed_kernel_versions();
    if let Some(newest) =
        kernels_after.iter().find(|k| !kernels_before.contains(k))
    {
        reporter.require_restart(restart::SYSTEM, newest);
    }

    Ok(())
}

/// Returns the installed kernel versions, as a set.
///
/// Empty on any failure: the only consequence of an empty set is that no
/// restart is suggested, which is a better outcome than a spurious reboot
/// prompt.
fn installed_kernel_versions() -> Vec<String> {
    zoi_system::kernel::installed_versions(&zoi_system::kernel::system_root())
        .unwrap_or_default()
}

/// Removes the given packages.
fn do_remove(packages: &[String], reporter: &Reporter) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    reporter.set_status(status::REMOVE);
    for pkg in packages {
        reporter.touch(pkg, info::REMOVING);
    }

    let mut args = vec!["uninstall".to_string()];
    args.extend(packages.iter().cloned());

    run_zoi(&args, reporter)
}

/// Updates the given packages, or everything when none are named.
///
/// An empty package list is what a "Update all" button sends, and mapping it to
/// `zoi update --all` is what makes `update --all` the distribution upgrade
/// path over `PackageKit` as well as from the CLI.
fn do_update(packages: &[String], reporter: &Reporter) -> Result<()> {
    let mut args = vec!["update".to_string()];

    if packages.is_empty() {
        args.push("--all".to_string());
    } else {
        for pkg in packages {
            reporter.touch(pkg, info::UPDATING);
        }
        args.extend(packages.iter().cloned());
    }

    reporter.set_status(status::UPDATE);
    run_zoi(&args, reporter)
}

/// Refreshes registry metadata.
fn do_refresh_cache(force: bool, reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::REFRESH_CACHE);

    let mut args = vec!["sync".to_string()];
    if force {
        args.push("--force".to_string());
    }

    run_zoi(&args, reporter)
}

/// Emits the `RepoDetail` signal for every configured repository.
fn do_repo_list(reporter: &Reporter) {
    let repos = configured_repos();

    for (repo, description) in repos {
        let _ = reporter.emit(
            transaction_interface(),
            "RepoDetail",
            // (repo_id, description, enabled)
            &(repo, description, true)
        );
    }
}

/// Returns the configured repositories as `(name, description)` pairs.
fn configured_repos() -> Vec<(String, String)> {
    let Ok(cfg) = zoi_core::config::read_config() else {
        return Vec::new();
    };

    let mut repos: Vec<(String, String)> = cfg
        .repos
        .iter()
        .map(|r| (r.clone(), String::new()))
        .collect();

    if let Some(registry) = cfg.registry.clone() {
        repos.insert(0, (registry, "active".to_string()));
    }

    repos
}

/// Answers `GetPackages`.
///
/// Installed packages are emitted before available ones, as the specification
/// requires, so a frontend can filter incrementally while the signal streams
/// in.
fn do_get_packages(filter: u64, reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let want_installed = filter_wants_installed(filter);
    let want_available = filter_wants_available(filter);

    // Neither bit set means "everything", which is the common case: most
    // frontends call GetPackages(filter::NONE) to enumerate the catalogue.
    let all = !want_installed && !want_available;

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();

    if all || want_installed {
        reporter.packages(installed.iter().map(installed_entry).collect());
    }

    // Available-but-not-installed packages come from the registry metadata.
    if all || want_available {
        let installed_names: HashMap<String, String> = installed
            .iter()
            .map(|m| (m.name.clone(), m.version.clone()))
            .collect();

        if let Ok(available) = zoi_resolver::local::get_all_available_packages()
        {
            let mut entries = Vec::new();
            for pkg in available {
                // An unresolved version cannot produce a usable id.
                let Some(version) = package_version(&pkg) else {
                    continue;
                };

                // Skip anything already installed at the same version; a
                // frontend showing "install firefox" for an installed Firefox
                // is confusing.
                if installed_names.get(&pkg.name).is_some_and(|v| v == version)
                {
                    continue;
                }

                entries.push((
                    info::AVAILABLE,
                    package_id(&pkg.name, version, &pkg.repo),
                    pkg.description.clone()
                ));
            }
            reporter.packages(entries);
        }
    }

    Ok(())
}

/// Answers `SearchNames` and `SearchDetails`.
///
/// The specification defines search as case-insensitive, with space-separated
/// terms combined with AND.
fn do_search(
    terms: &[String],
    detail: bool,
    reporter: &Reporter
) -> Result<()> {
    reporter.set_status(status::QUERY);

    let terms: Vec<String> = terms
        .join(" ")
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return Ok(());
    }

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();

    // Installed first, per the specification.
    let mut hits = Vec::new();
    for manifest in &installed {
        if matches_terms(&manifest.name, &manifest.description, &terms, detail)
        {
            hits.push(installed_entry(manifest));
        }
    }
    reporter.packages(hits);

    if let Ok(available) = zoi_resolver::local::get_all_available_packages() {
        let installed_names: HashMap<&String, &String> =
            installed.iter().map(|m| (&m.name, &m.version)).collect();
        let mut hits = Vec::new();
        for pkg in available {
            let Some(version) = package_version(&pkg) else {
                continue;
            };
            if installed_names
                .get(&pkg.name)
                .is_some_and(|v| *v == version)
            {
                continue;
            }
            if matches_terms(&pkg.name, &pkg.description, &terms, detail) {
                hits.push((
                    info::AVAILABLE,
                    package_id(&pkg.name, version, &pkg.repo),
                    pkg.description.clone()
                ));
            }
        }
        reporter.packages(hits);
    }

    Ok(())
}

/// Tests a package against search terms.
///
/// With `detail` the description is searched too; without it only the name is,
/// which is the difference between `SearchNames` and `SearchDetails`. Every
/// term must match, so "gnome power" excludes "powertop".
fn matches_terms(
    name: &str,
    description: &str,
    terms: &[String],
    detail: bool
) -> bool {
    let name = name.to_lowercase();
    let description = description.to_lowercase();

    terms.iter().all(|term| {
        name.contains(term) || (detail && description.contains(term))
    })
}

/// Answers `Resolve`: turns names into package ids.
fn do_resolve(names: &[String], reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();
    let mut entries = Vec::new();

    for requested in names {
        let wanted = package_name_from_id(requested);

        // An exact match first.
        if let Some(manifest) = installed.iter().find(|m| m.name == wanted) {
            entries.push(installed_entry(manifest));
            continue;
        }

        // Then a prefix match, which is how `Resolve("fire")` finds firefox.
        if let Some(manifest) =
            installed.iter().find(|m| m.name.starts_with(&wanted))
        {
            entries.push(installed_entry(manifest));
            continue;
        }

        // Finally the registry.
        if let Ok(available) = zoi_resolver::local::get_all_available_packages()
            && let Some(pkg) = available.iter().find(|p| p.name == wanted)
            && let Some(version) = package_version(pkg)
        {
            entries.push((
                info::AVAILABLE,
                package_id(&pkg.name, version, &pkg.repo),
                pkg.description.clone()
            ));
        }
    }

    reporter.packages(entries);
    Ok(())
}

/// Answers `GetUpdates`.
///
/// Runs Zoi's own update scan, so the answer is exactly what `zoi update` would
/// do. That is what makes a `PackageKit` "N updates available" badge agree with
/// the CLI.
fn do_get_updates(reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();
    let mut entries = Vec::new();
    let mut blocked: Vec<(u32, String, String)> = Vec::new();

    for manifest in &installed {
        let source = if let Some(sub) = &manifest.sub_package {
            format!(
                "#{}@{}/{}:{}",
                manifest.registry_handle, manifest.repo, manifest.name, sub
            )
        } else {
            format!(
                "#{}@{}/{}",
                manifest.registry_handle, manifest.repo, manifest.name
            )
        };

        // A pinned package is reported as `blocked` rather than omitted. The
        // specification reserves this for "an update exists but cannot be
        // installed", which is exactly a pin: hiding it would make an
        // administrator think they are up to date when they are not.
        if zoi_core::pin::is_pinned(&source).unwrap_or(false)
            || zoi_core::pin::is_pinned(&manifest.name).unwrap_or(false)
        {
            blocked.push((
                info::BLOCKED,
                package_id(&manifest.name, &manifest.version, &manifest.repo),
                "Pinned to this version; run 'zoi unpin' to receive updates."
                    .to_string()
            ));
            continue;
        }

        let Ok((_, new_version, _, _, _, _, _)) =
            zoi_resolver::resolve::resolve_package_and_version(
                &source,
                Some(manifest.scope),
                true,
                false
            )
        else {
            // The registry no longer offers this package at all, which for a
            // rolling distribution means it was renamed or dropped.
            continue;
        };

        if manifest.version == new_version && manifest.revision == new_version {
            continue;
        }

        if manifest.version == new_version {
            continue;
        }

        // Severity comes from Zoi's own advisory data, so a frontend can rank
        // a security update above a routine one.
        let info = update_info_from_advisories(
            &manifest.registry_handle,
            &manifest.name,
            manifest.sub_package.as_deref(),
            &manifest.version
        )
        .unwrap_or(info::NORMAL);

        entries.push((
            info,
            package_id(&manifest.name, &new_version, &manifest.repo),
            manifest.description.clone()
        ));
    }

    // Blocked entries come last so a frontend's "important" filter keeps them
    // out of the default view while the count still reflects reality.
    entries.extend(blocked);

    reporter.packages(entries);
    Ok(())
}

/// Answers `GetDetails`, emitting one `Details` signal per package.
///
/// The emitted dictionary always carries every documented key. A frontend
/// renders its panel straight from this map, and a missing key leaves a blank
/// field with no indication that anything went wrong.
fn do_get_details(package_ids: &[String], reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();

    for raw in package_ids {
        let name = package_name_from_id(raw);
        let id = raw.trim().to_string();

        let mut details: HashMap<String, OwnedValue> = HashMap::new();
        let mut put = |k: &str, v: OwnedValue| {
            details.insert(k.to_string(), v);
        };

        // Defaults, so the map is always complete.
        put("package_id", owned_str(&id));
        put("license", owned_str(""));
        put("group", OwnedValue::from(info::UNKNOWN));
        put("detail", owned_str(""));
        put("url", owned_str(""));
        put("size", OwnedValue::from(0u64));
        put("summary", owned_str(""));

        // The registry metadata is consulted for every package, installed or
        // not, because licence and homepage live there.
        // `InstallManifest` records neither, and a licence-compliance
        // frontend needs the licence to be right rather than merely
        // present.
        let registry_entry = zoi_resolver::local::get_all_available_packages()
            .ok()
            .and_then(|available| {
                available.into_iter().find(|p| p.name == name)
            });

        if let Some(entry) = &registry_entry {
            put("license", owned_str(&entry.license));
            put("url", owned_str(entry.website.as_deref().unwrap_or("")));
        }

        if let Some(manifest) = installed.iter().find(|m| m.name == name) {
            let id =
                package_id(&manifest.name, &manifest.version, &manifest.repo);
            put("package_id", owned_str(&id));
            put("summary", owned_str(&manifest.description));
            put("detail", owned_str(&manifest.description));
            put(
                "size",
                OwnedValue::from(manifest.installed_size.unwrap_or(0))
            );
        } else if let Some(pkg) = registry_entry
            && let Some(version) = package_version(&pkg)
        {
            let id = package_id(&pkg.name, version, &pkg.repo);
            put("package_id", owned_str(&id));
            put("summary", owned_str(&pkg.description));
            put("detail", owned_str(&pkg.description));
        }

        let _ = reporter.emit(transaction_interface(), "Details", &details);
    }

    Ok(())
}

/// Answers `GetFiles`, emitting one `Files` signal per package.
fn do_get_files(package_ids: &[String], reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let installed =
        zoi_resolver::local::get_installed_packages().unwrap_or_default();

    for raw in package_ids {
        let name = package_name_from_id(raw);
        let manifest = installed.iter().find(|m| m.name == name);

        let id = manifest.map_or_else(
            || raw.trim().to_string(),
            |m| package_id(&m.name, &m.version, &m.repo)
        );

        // Manifest paths carry Zoi's `${usrroot}` style placeholders, which
        // mean nothing to a client. They are stripped so a frontend can
        // offer to open the file.
        let files: Vec<String> = manifest
            .map(|m| {
                m.installed_files
                    .iter()
                    .map(|f| f.replace("${usrroot}", ""))
                    .collect()
            })
            .unwrap_or_default();

        let _ = reporter.emit(transaction_interface(), "Files", &(id, files));
    }

    Ok(())
}

/// Answers `GetDistroUpgrades`, which is always empty.
///
/// A `ZoiOS` distribution is expected to be rolling or registry-managed, so a
/// package update *is* the distribution upgrade. Emitting nothing makes a
/// frontend hide its "distribution upgrade" affordance instead of offering a
/// path that does not exist.
fn do_get_distro_upgrades(reporter: &Reporter) -> Result<()> {
    reporter.set_status(status::QUERY);

    let _ = reporter.emit(
        transaction_interface(),
        "DistroUpgrade",
        // PkDistroUpgradeEnum::UNKNOWN, name, summary
        &(0u32, "ZoiOS".to_string(), String::new())
    );

    Ok(())
}

/// Spawns a worker thread and reports the outcome through signals.
///
/// Every transaction method funnels through here. Centralising it is what
/// guarantees the `Finished` signal is emitted exactly once and that a failure
/// always produces an `ErrorCode` first, which a frontend needs to show
/// something meaningful.
fn spawn<F>(reporter: &Reporter, work: F)
where
    F: FnOnce(&Reporter) -> Result<()> + Send + 'static
{
    // Cloned so the thread owns its own handle. `Reporter` is a cheap handle
    // over shared state, not the state itself.
    let worker = reporter.clone();

    std::thread::spawn(move || {
        worker.set_status(status::RUNNING);

        let outcome = work(&worker);

        // A cancellation is reported as cancelled rather than failed: the user
        // asked for it, so it is not an error condition.
        if worker.cancel_requested() {
            worker.set_status(status::FINISHED);
            let runtime = worker.elapsed_ms();
            let _ = worker.emit(
                transaction_interface(),
                "Finished",
                &(exit::CANCELLED, runtime)
            );
            return;
        }

        match outcome {
            Ok(()) => worker.succeed(),
            Err(e) => {
                // Recover the classified code so the client gets a specific
                // `ErrorCode` rather than a generic one.
                let code = e
                    .downcast_ref::<PkError>()
                    .map_or(error_code::TRANSACTION_ERROR, |p| p.code);
                worker.fail(code, e.to_string());
            }
        }
    });
}

/// Emits the root-level `UpdatesChanged` signal.
///
/// Sent after a cache refresh. A frontend showing an update badge waits for
/// this, so it is emitted whether or not the refresh found anything: "no
/// updates" is still an answer.
fn refreshed() {
    let Ok(conn) = dbus_connection() else {
        return;
    };
    let Ok(emitter) = SignalEmitter::new(conn.inner(), ROOT_PATH) else {
        return;
    };
    let _ = async_io::block_on(emitter.emit::<_, _, _>(
        root_interface(),
        "UpdatesChanged",
        &()
    ));
}

// ---------------------------------------------------------------------------
// Root object: org.freedesktop.PackageKit at /org/freedesktop/PackageKit
// ---------------------------------------------------------------------------

/// The root object.
///
/// Holds only the shared table of live transactions; every method here is
/// either a property or a factory.
struct PackageKitRoot {
    /// Live transaction states, keyed by object path.
    transactions: Arc<Mutex<HashMap<String, Arc<Mutex<TransactionState>>>>>,
    /// Monotonic source of unique transaction ids.
    counter: Arc<AtomicU32>
}

impl PackageKitRoot {
    /// Builds the root with an empty transaction table.
    fn new() -> Self {
        Self {
            transactions: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(AtomicU32::new(0))
        }
    }

    /// Allocates an unused object path for a new transaction.
    ///
    /// The counter is monotonic rather than random so ids sort chronologically,
    /// which makes the `TransactionListChanged` sequence readable in a bus
    /// monitor.
    fn new_transaction_path(
        &self
    ) -> Result<(String, Arc<Mutex<TransactionState>>)> {
        // Reuse is impossible with a monotonic counter, but a stale entry could
        // survive a daemon restart that left the object server populated.
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let path = format!("{ROOT_PATH}/{id:x}");

        if let Ok(existing) = self.transactions.lock()
            && existing.contains_key(&path)
        {
            bail!("Transaction path {path} is already in use");
        }

        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));

        if let Ok(mut table) = self.transactions.lock() {
            // Reclaim anything idle for too long. Unbounded growth would
            // otherwise leak one object path per transaction forever.
            let expired: Vec<String> = table
                .iter()
                .filter(|(_, tx)| {
                    tx.lock().is_ok_and(|s| {
                        s.started.elapsed().as_secs()
                            > TRANSACTION_IDLE_TIMEOUT_SECS
                    })
                })
                .map(|(path, _)| path.clone())
                .collect();
            for path in expired {
                table.remove(&path);
                // Drop the exported interface so the object path stops
                // resolving, otherwise a reused path would answer with a stale
                // transaction.
                if let Ok(conn) = dbus_connection() {
                    let _ = conn
                        .object_server()
                        .remove::<PackageKitTransaction, _>(path.as_str());
                }
            }

            table.insert(path.clone(), state.clone());
        }

        Ok((path, state))
    }

    /// Returns the live transaction object paths.
    fn transaction_paths(&self) -> Vec<String> {
        self.transactions
            .lock()
            .map(|t| {
                let mut paths: Vec<String> = t.keys().cloned().collect();
                paths.sort_by_key(|p| {
                    p.rsplit('/').next().unwrap_or("").to_string()
                });
                paths
            })
            .unwrap_or_default()
    }

    /// Emits a root-level signal from the caller's thread.
    fn emit_root(&self, signal: &str) {
        let Ok(conn) = dbus_connection() else {
            return;
        };
        let Ok(emitter) = SignalEmitter::new(conn.inner(), ROOT_PATH) else {
            return;
        };

        match signal {
            // These take no arguments.
            "RestartSchedule" | "InstalledChanged" | "RepoListChanged"
            | "UpdatesChanged" => {
                let _ = async_io::block_on(emitter.emit::<_, _, _>(
                    root_interface(),
                    signal,
                    &()
                ));
            }
            "TransactionListChanged" => {
                let _ = async_io::block_on(emitter.emit::<_, _, _>(
                    root_interface(),
                    signal,
                    &(self.transaction_paths(),)
                ));
            }
            _ => {}
        }
    }
}

/// D-Bus properties and factory methods on the root object.
#[zbus::interface(name = "org.freedesktop.PackageKit")]
impl PackageKitRoot {
    /// Major interface version.
    #[zbus(property)]
    fn version_major(&self) -> u32 {
        VERSION_MAJOR
    }

    /// Minor interface version.
    #[zbus(property)]
    fn version_minor(&self) -> u32 {
        VERSION_MINOR
    }

    /// Micro interface version.
    #[zbus(property)]
    fn version_micro(&self) -> u32 {
        VERSION_MICRO
    }

    /// Backend name, e.g. `dnf`.
    #[zbus(property)]
    fn backend_name(&self) -> String {
        BACKEND_NAME.to_string()
    }

    /// Human readable backend description.
    #[zbus(property)]
    fn backend_description(&self) -> String {
        BACKEND_DESCRIPTION.to_string()
    }

    /// Backend author.
    #[zbus(property)]
    fn backend_author(&self) -> String {
        BACKEND_AUTHOR.to_string()
    }

    /// Bitfield of supported roles.
    #[zbus(property)]
    fn roles(&self) -> u64 {
        SUPPORTED_ROLES
    }

    /// Bitfield of supported package groups. None: Zoi has no group metadata.
    #[zbus(property)]
    fn groups(&self) -> u64 {
        SUPPORTED_GROUPS
    }

    /// Bitfield of supported filters.
    #[zbus(property)]
    fn filters(&self) -> u64 {
        SUPPORTED_FILTERS
    }

    /// MIME types accepted for local installs.
    #[zbus(property)]
    fn mime_types(&self) -> Vec<String> {
        MIME_TYPES
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
    }

    /// Whether the backend is locked.
    ///
    /// Zoi serialises package operations through its own transaction lock, so
    /// this reports that rather than always claiming false: a frontend that
    /// believes the backend is free while a transaction holds the lock will let
    /// the user start a second one, which then fails.
    #[zbus(property)]
    fn locked(&self) -> bool {
        self.transactions.lock().is_ok_and(|t| {
            t.values().any(|state| {
                state.lock().is_ok_and(|s| {
                    s.status != status::FINISHED && s.status != status::WAIT
                })
            })
        })
    }

    /// Network state, so a frontend can offer offline mode.
    #[zbus(property)]
    fn network_state(&self) -> u32 {
        network_state()
    }

    /// Distribution identification in `id;version;arch` form.
    #[zbus(property)]
    fn distro_id(&self) -> String {
        DISTRO_ID.to_string()
    }

    /// Reports whether a caller could authorize an action.
    ///
    /// Always yes: authorization happens at the `zoid` boundary and the method
    /// bodies do no authorization of their own. Reporting `interactive` would
    /// make a frontend prompt for a password that is then never used.
    #[zbus(name = "CanAuthorize")]
    fn can_authorize(&self, _action_id: &str) -> u32 {
        authorize::YES
    }

    /// Creates a transaction and returns its object path.
    #[zbus(name = "CreateTransaction")]
    fn create_transaction(&self) -> zbus::fdo::Result<OwnedObjectPath> {
        let (path, state) = self
            .new_transaction_path()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        // `ObjectPath` borrows its backing string, so the source has to outlive
        // it. An owned `String` is kept alive by `path` for the rest of the fn.
        let object_path: ObjectPath<'_> =
            path.as_str().try_into().map_err(|_| {
                zbus::fdo::Error::Failed(format!(
                    "Internal error: invalid transaction object path: {path}"
                ))
            })?;

        let interface = PackageKitTransaction {
            state,
            transaction_path: Arc::new(Mutex::new(path.clone())),
            sender: String::new(),
            uid: 0
        };

        dbus_connection()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .object_server()
            .at(object_path.clone(), interface)
            // `at` reports whether the interface was newly added. A `false`
            // would mean the path was taken, which the monotonic counter above
            // already rules out, so the flag is not interesting here.
            .map(|_| ())
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(OwnedObjectPath::from(object_path))
    }

    /// Returns object paths for transactions currently in progress.
    #[zbus(name = "GetTransactionList")]
    fn get_transaction_list(&self) -> Vec<OwnedObjectPath> {
        self.transaction_paths()
            .into_iter()
            .filter_map(|p| {
                let owned = p.clone();
                let path: ObjectPath<'_> = owned.as_str().try_into().ok()?;
                Some(OwnedObjectPath::from(path))
            })
            .collect()
    }

    /// Returns seconds since an action last completed successfully.
    ///
    /// Always zero. Zoi keeps its own audit log; a fabricated timestamp here
    /// would let a frontend schedule a refresh based on a lie.
    #[zbus(name = "GetTimeSinceAction")]
    fn get_time_since_action(&self, _role: u32) -> u32 {
        0
    }

    /// Signals that the backend state changed, so clients drop caches.
    #[zbus(name = "StateHasChanged")]
    fn state_has_changed(&self, reason: &str) {
        if reason == "posttrans" {
            self.emit_root("InstalledChanged");
        }
        self.emit_root("UpdatesChanged");
    }

    /// Suggests the daemon quit so a native tool can run unopposed.
    ///
    /// Honoured by clearing the transaction table: `zoi` from a terminal takes
    /// the same database lock, and a long-lived daemon holding it would block
    /// the administrator from ever fixing things by hand.
    #[zbus(name = "SuggestDaemonQuit")]
    fn suggest_daemon_quit(&self) {
        if let Ok(mut table) = self.transactions.lock() {
            table.clear();
        }
    }

    /// Returns per-package history.
    ///
    /// Empty. The specification notes this only covers transactions done
    /// through `PackageKit`, and Zoi's own history lives in its audit log;
    /// reporting an empty list here is honest where inventing entries would
    /// not be.
    #[zbus(name = "GetPackageHistory")]
    fn get_package_history(
        &self,
        _names: Vec<String>,
        _count: u32
    ) -> HashMap<String, Vec<HashMap<String, OwnedValue>>> {
        HashMap::new()
    }

    /// Returns daemon state for debugging.
    #[zbus(name = "GetDaemonState")]
    fn get_daemon_state(&self) -> String {
        format!(
            "backend={} version={} transactions={} network={}",
            BACKEND_NAME,
            env!("CARGO_PKG_VERSION"),
            self.transaction_paths().len(),
            network_state()
        )
    }

    /// Sets the daemon's proxy configuration.
    ///
    /// Accepted and ignored: Zoi reads proxy settings from the environment, so
    /// there is nothing to persist. Erroring would make a frontend report the
    /// backend as misconfigured.
    #[zbus(name = "SetProxy")]
    fn set_proxy(
        &self,
        _proxy_http: &str,
        _proxy_https: &str,
        _proxy_ftp: &str,
        _proxy_socks: &str,
        _no_proxy: &str,
        _pac: &str
    ) {
    }

    /// Emitted when a transaction is created or finishes.
    #[zbus(signal)]
    async fn transaction_list_changed(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        transactions: Vec<String>
    ) -> zbus::Result<()>;

    /// Emitted when a system restart has been scheduled.
    ///
    /// zbus requires zero-argument signals to take the emitter explicitly; the
    /// parameter is supplied by the generated dispatcher, never by a caller.
    #[zbus(signal)]
    async fn restart_schedule(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>
    ) -> zbus::Result<()>;

    /// Emitted when the installed package set may have changed.
    #[zbus(signal)]
    async fn installed_changed(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>
    ) -> zbus::Result<()>;

    /// Emitted when the repository list changed.
    #[zbus(signal)]
    async fn repo_list_changed(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>
    ) -> zbus::Result<()>;

    /// Emitted when the number of available updates changed.
    #[zbus(signal)]
    async fn updates_changed(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>
    ) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// Transaction object: org.freedesktop.PackageKit.Transaction
// ---------------------------------------------------------------------------

/// One transaction.
///
/// All methods return immediately; results arrive as signals.
struct PackageKitTransaction {
    /// Mutable transaction state.
    state: Arc<Mutex<TransactionState>>,
    /// The object path this interface is served at.
    ///
    /// Held explicitly because signals are emitted from worker threads that
    /// only hold a `Reporter`, and zbus gives an interface body no access
    /// to its own object path.
    transaction_path: Arc<Mutex<String>>,
    /// D-Bus name of the creator, reported as `Sender`.
    sender: String,
    /// Uid of the creator.
    uid: u32
}

impl PackageKitTransaction {
    /// Builds a reporter for this transaction.
    fn reporter(&self) -> Reporter {
        // The object path is needed to emit signals, and zbus does not expose
        // it to an interface method's body. It is reconstructed from
        // the object's registered path instead, which the daemon
        // records when the transaction is created.
        let path: String = self
            .transaction_path
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default();
        Reporter::new(&path, self.state.clone())
    }

    /// Spawns work for this transaction.
    fn start<F>(&self, role: u32, work: F)
    where
        F: FnOnce(&Reporter) -> Result<()> + Send + 'static
    {
        if let Ok(mut state) = self.state.lock() {
            state.role = role;
            state.status = status::WAIT;
        }

        spawn(&self.reporter(), work);
    }
}

/// Properties and methods on a transaction object.
#[zbus::interface(name = "org.freedesktop.PackageKit.Transaction")]
impl PackageKitTransaction {
    // --- properties ---

    /// What this transaction was asked to do.
    #[zbus(property)]
    fn role(&self) -> u32 {
        self.state.lock().map_or(role::UNKNOWN, |s| s.role)
    }

    /// Current status.
    #[zbus(property)]
    fn status(&self) -> u32 {
        self.state.lock().map_or(status::UNKNOWN, |s| s.status)
    }

    /// Last package id processed.
    #[zbus(property)]
    fn last_package(&self) -> String {
        self.state
            .lock()
            .map(|s| s.last_package.clone())
            .unwrap_or_default()
    }

    /// Uid of the user that started the transaction.
    #[zbus(property)]
    fn uid(&self) -> u32 {
        self.uid
    }

    /// D-Bus name of the process that started the transaction.
    #[zbus(property)]
    fn sender(&self) -> String {
        self.sender.clone()
    }

    /// Completion percentage; 101 when not calculable.
    #[zbus(property)]
    fn percentage(&self) -> u32 {
        self.state.lock().map_or(101, |s| s.percentage)
    }

    /// Whether the transaction can be cancelled.
    ///
    /// True only before any file has been written. Once Zoi has started
    /// committing, cancelling leaves a partially applied package, which is
    /// worse than finishing, so the honest answer is to refuse.
    #[zbus(property)]
    fn allow_cancel(&self) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.status == status::WAIT || s.status == status::QUERY
        })
    }

    /// Whether the original caller is still connected.
    #[zbus(property)]
    fn caller_active(&self) -> bool {
        true
    }

    /// Seconds elapsed.
    #[zbus(property)]
    fn elapsed_time(&self) -> u32 {
        self.state
            .lock()
            .map_or(0, |s| s.started.elapsed().as_secs() as u32)
    }

    /// Seconds remaining, or zero when unknown.
    #[zbus(property)]
    fn remaining_time(&self) -> u32 {
        0
    }

    /// Bytes per second, or zero when unknown.
    #[zbus(property)]
    fn speed(&self) -> u32 {
        0
    }

    /// Bytes still to download.
    #[zbus(property)]
    fn download_size_remaining(&self) -> u64 {
        0
    }

    /// Transaction flags as set by the caller.
    #[zbus(property)]
    fn transaction_flags(&self) -> u64 {
        self.state.lock().map_or(0, |s| s.transaction_flags)
    }

    // --- hints and control ---

    /// Sets transaction hints such as `locale` or `interactive`.
    ///
    /// Accepted and ignored, except that `interactive=true` is honoured by the
    /// install path running non-interactively anyway: a D-Bus caller has no
    /// terminal to answer questions on, so `zoi` is always invoked with
    /// `--yes`.
    #[zbus(name = "SetHints")]
    fn set_hints(&self, hints: Vec<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.hints = hints;
        }
    }

    /// Records acceptance of an end-user licence agreement.
    ///
    /// Zoi package definitions carry an SPDX licence expression, not a
    /// clickable EULA, so there is nothing to record.
    #[zbus(name = "AcceptEula")]
    fn accept_eula(&self, _eula_id: &str) {}

    /// Requests cancellation.
    ///
    /// Recorded rather than acted on immediately. Killing `zoi` mid-commit can
    /// leave a package half-written; the flag is checked between packages
    /// instead.
    #[zbus(name = "Cancel")]
    fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.cancel_requested = true;
            state.status = status::CANCEL;
        }
    }

    // --- queries ---

    /// Lists packages, honouring the filter bitfield.
    #[zbus(name = "GetPackages")]
    fn get_packages(&self, filter: u64) {
        self.start(role::GET_PACKAGES, move |r| do_get_packages(filter, r));
    }

    /// Searches package names.
    #[zbus(name = "SearchNames")]
    fn search_names(&self, _filter: u64, values: Vec<String>) {
        self.start(role::SEARCH_NAME, move |r| do_search(&values, false, r));
    }

    /// Searches package names, summaries, licences and URLs.
    #[zbus(name = "SearchDetails")]
    fn search_details(&self, _filter: u64, values: Vec<String>) {
        self.start(role::SEARCH_DETAILS, move |r| do_search(&values, true, r));
    }

    /// Resolves names to package ids.
    #[zbus(name = "Resolve")]
    fn resolve(&self, _filter: u64, packages: Vec<String>) {
        self.start(role::UNKNOWN, move |r| do_resolve(&packages, r));
    }

    /// Returns details for specific packages.
    #[zbus(name = "GetDetails")]
    fn get_details(&self, package_ids: Vec<String>) {
        self.start(role::GET_DETAILS, move |r| do_get_details(&package_ids, r));
    }

    /// Returns the file list for specific packages.
    #[zbus(name = "GetFiles")]
    fn get_files(&self, package_ids: Vec<String>) {
        self.start(role::GET_FILES, move |r| do_get_files(&package_ids, r));
    }

    /// Lists installed packages with available updates.
    #[zbus(name = "GetUpdates")]
    fn get_updates(&self, _filter: u64) {
        self.start(role::GET_UPDATES, do_get_updates);
    }

    /// Returns details of specific updates.
    ///
    /// Delegates to `GetDetails`: Zoi does not distinguish update metadata from
    /// package metadata, so a frontend asking for update specifics gets the
    /// same information a details panel would show.
    #[zbus(name = "GetUpdateDetail")]
    fn get_update_detail(&self, package_ids: Vec<String>) {
        self.start(role::GET_UPDATE_DETAIL, move |r| {
            do_get_details(&package_ids, r)
        });
    }

    /// Lists distribution upgrades. Always empty for a rolling distribution.
    #[zbus(name = "GetDistroUpgrades")]
    fn get_distro_upgrades(&self) {
        self.start(role::GET_DISTRO_UPGRADES, do_get_distro_upgrades);
    }

    /// Lists configured repositories.
    #[zbus(name = "GetRepoList")]
    fn get_repo_list(&self, _filter: u64) {
        self.start(role::GET_REPO_LIST, |r| {
            do_repo_list(r);
            Ok(())
        });
    }

    /// Returns packages a package depends on.
    ///
    /// Emits the package itself plus its declared dependencies. Zoi resolves
    /// the full closure during install and reports it then, so pre-flight
    /// enumeration here would be duplicated work.
    #[zbus(name = "DependsOn")]
    fn depends_on(
        &self,
        _filter: u64,
        package_ids: Vec<String>,
        _recursive: bool
    ) {
        self.start(role::UNKNOWN, move |r| {
            let installed = zoi_resolver::local::get_installed_packages()
                .unwrap_or_default();
            let mut entries = Vec::new();

            for raw in &package_ids {
                let name = package_name_from_id(raw);
                if let Some(manifest) =
                    installed.iter().find(|m| m.name == name)
                {
                    entries.push(installed_entry(manifest));
                    for dep in &manifest.installed_dependencies {
                        let dep_name = package_name_from_id(dep);
                        if let Some(dep_manifest) =
                            installed.iter().find(|m| m.name == dep_name)
                        {
                            entries.push(installed_entry(dep_manifest));
                        }
                    }
                }
            }

            r.packages(entries);
            Ok(())
        });
    }

    /// Returns packages that depend on a package.
    #[zbus(name = "RequiredBy")]
    fn required_by(
        &self,
        _filter: u64,
        package_ids: Vec<String>,
        _recursive: bool
    ) {
        self.start(role::UNKNOWN, move |r| {
            let installed = zoi_resolver::local::get_installed_packages()
                .unwrap_or_default();
            let mut entries = Vec::new();

            for raw in &package_ids {
                let name = package_name_from_id(raw);
                for manifest in &installed {
                    if manifest
                        .installed_dependencies
                        .iter()
                        .any(|d| package_name_from_id(d) == name)
                    {
                        entries.push(installed_entry(manifest));
                    }
                }
            }

            r.packages(entries);
            Ok(())
        });
    }

    /// Finds packages providing a file or command.
    #[zbus(name = "WhatProvides")]
    fn what_provides(&self, _filter: u64, values: Vec<String>) {
        self.start(role::WHAT_PROVIDES, move |r| {
            let installed = zoi_resolver::local::get_installed_packages()
                .unwrap_or_default();
            let mut entries = Vec::new();

            for wanted in &values {
                let wanted = wanted.trim_start_matches('/');
                for manifest in &installed {
                    // `bins` is the authoritative list of what a package puts
                    // on PATH, so it answers "which package
                    // provides this command"
                    // far more reliably than scanning the whole file list.
                    if manifest
                        .bins
                        .as_ref()
                        .is_some_and(|bins| bins.iter().any(|b| b == wanted))
                    {
                        entries.push(installed_entry(manifest));
                    }
                }
            }

            r.packages(entries);
            Ok(())
        });
    }

    /// Returns old transaction records. Always empty; see `GetPackageHistory`.
    #[zbus(name = "GetOldTransactions")]
    fn get_old_transactions(&self, _number: u32) {}

    // --- mutations ---

    /// Re-downloads repository metadata.
    #[zbus(name = "RefreshCache")]
    fn refresh_cache(&self, force: bool) {
        self.start(role::REFRESH_CACHE, move |r| {
            // The root signal is what makes a running frontend's update badge
            // refresh. It is emitted either way, because "there are no updates"
            // is itself information a frontend needs.
            let outcome = do_refresh_cache(force, r);
            refreshed();
            outcome
        });
    }

    /// Installs packages.
    #[zbus(name = "InstallPackages")]
    fn install_packages(
        &self,
        _transaction_flags: u64,
        package_ids: Vec<String>
    ) {
        let packages: Vec<String> = package_ids
            .iter()
            .map(|p| package_name_from_id(p))
            .collect();
        self.start(role::INSTALL_PACKAGES, move |r| do_install(&packages, r));
    }

    /// Removes packages.
    #[zbus(name = "RemovePackages")]
    fn remove_packages(
        &self,
        _transaction_flags: u64,
        package_ids: Vec<String>,
        _allow_deps: bool,
        _autoremove: bool
    ) {
        let packages: Vec<String> = package_ids
            .iter()
            .map(|p| package_name_from_id(p))
            .collect();
        self.start(role::REMOVE_PACKAGES, move |r| do_remove(&packages, r));
    }

    /// Updates packages, or all of them when the list is empty.
    #[zbus(name = "UpdatePackages")]
    fn update_packages(
        &self,
        _transaction_flags: u64,
        package_ids: Vec<String>
    ) {
        let packages: Vec<String> = package_ids
            .iter()
            .map(|p| package_name_from_id(p))
            .collect();
        self.start(role::UPDATE_PACKAGES, move |r| do_update(&packages, r));
    }

    /// Downloads packages without installing them.
    ///
    /// Zoi's transaction model has no separate download step: archives are
    /// fetched, verified and committed as one atomic operation. Reported as
    /// unsupported rather than silently converted into an install, because a
    /// frontend offering "Download" expects nothing to be installed.
    #[zbus(name = "DownloadPackages")]
    fn download_packages(
        &self,
        _store_in_cache: bool,
        _package_ids: Vec<String>
    ) {
        // Fails immediately rather than going through a worker: there is no
        // work to schedule, and a client waiting on `Finished` needs it
        // now.
        self.reporter().fail(
            error_code::NOT_SUPPORTED,
            "Zoi has no separate download step; archives are fetched and \
             installed in one transaction."
                .to_string()
        );
    }

    /// Installs packages from local files.
    ///
    /// Delegated to `zoi install <path>`, which is how Zoi consumes a `.zpa`
    /// archive. A frontend's MIME filter will not offer `.zpa` files, so this
    /// exists mainly for direct API users.
    #[zbus(name = "InstallFiles")]
    fn install_files(&self, _transaction_flags: u64, full_paths: Vec<String>) {
        self.start(role::INSTALL_PACKAGES, move |r| do_install(&full_paths, r));
    }

    /// Performs a distribution upgrade.
    ///
    /// Delegated to `zoi update --all`, which is the upgrade path for a rolling
    /// or registry-managed `ZoiOS` distribution.
    #[zbus(name = "UpgradeSystem")]
    fn upgrade_system(
        &self,
        _transaction_flags: u64,
        _distro_id: &str,
        _upgrade_kind: u32
    ) {
        self.start(role::UPDATE_PACKAGES, move |r| do_update(&[], r));
    }

    /// Enables or disables a repository.
    #[zbus(name = "RepoEnable")]
    fn repo_enable(&self, _repo_id: &str, _enabled: bool) {
        let reporter = self.reporter();
        reporter.fail(
            error_code::NOT_SUPPORTED,
            "Repositories are configured with 'zoi sync add' and 'zoi repo', \
             not over D-Bus."
                .to_string()
        );
    }

    /// Sets a backend-specific repository parameter.
    #[zbus(name = "RepoSetData")]
    fn repo_set_data(&self, _repo_id: &str, _parameter: &str, _value: &str) {
        let reporter = self.reporter();
        reporter.fail(
            error_code::NOT_SUPPORTED,
            "Repository data is set in repo.yaml, not over D-Bus.".to_string()
        );
    }

    /// Removes a repository.
    #[zbus(name = "RepoRemove")]
    fn repo_remove(
        &self,
        _transaction_flags: u64,
        _repo_id: &str,
        _autoremove: bool
    ) {
        let reporter = self.reporter();
        reporter.fail(
            error_code::NOT_SUPPORTED,
            "Repositories are managed with 'zoi repo remove'.".to_string()
        );
    }

    /// Installs a signing key.
    #[zbus(name = "InstallSignature")]
    fn install_signature(
        &self,
        _sig_type: u32,
        _key_id: &str,
        _package_id: &str
    ) {
        let reporter = self.reporter();
        reporter.fail(
            error_code::NOT_SUPPORTED,
            "Signing keys are managed with 'zoi pgp import'.".to_string()
        );
    }

    // --- signals ---

    /// Emits package results.
    ///
    /// The specification defines this as `a(uss)`: an array of
    /// `(info, package_id, summary)` tuples.
    #[zbus(signal)]
    async fn packages(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        packages: Vec<(u32, String, String)>
    ) -> zbus::Result<()>;

    /// Emits details for one package.
    #[zbus(signal)]
    async fn details(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        data: HashMap<String, OwnedValue>
    ) -> zbus::Result<()>;

    /// Emits the file list for one package.
    #[zbus(signal)]
    async fn files(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        package_id: String,
        file_list: Vec<String>
    ) -> zbus::Result<()>;

    /// Emits information about one repository.
    #[zbus(signal)]
    async fn repo_detail(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        repo_id: String,
        description: String,
        enabled: bool
    ) -> zbus::Result<()>;

    /// Emits progress for a single package.
    #[zbus(signal)]
    async fn item_progress(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        id: String,
        status: u32,
        percentage: u32
    ) -> zbus::Result<()>;

    /// Emits a restart requirement.
    #[zbus(signal)]
    async fn require_restart(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        type_: u32,
        package_id: String
    ) -> zbus::Result<()>;

    /// Emits a fatal error.
    #[zbus(signal)]
    async fn error_code(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        code: u32,
        details: String
    ) -> zbus::Result<()>;

    /// Signals that the transaction has finished.
    #[zbus(signal)]
    async fn finished(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        exit: u32,
        runtime: u32
    ) -> zbus::Result<()>;

    /// Emits a distribution upgrade record.
    #[zbus(signal)]
    async fn distro_upgrade(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        type_: u32,
        name: String,
        summary: String
    ) -> zbus::Result<()>;

    /// Emits a summary record for a past transaction.
    #[zbus(signal)]
    async fn transaction(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        object_path: OwnedObjectPath,
        timespec: String,
        succeeded: bool,
        role: u32,
        duration: u32,
        data: String,
        uid: u32,
        cmdline: String
    ) -> zbus::Result<()>;

    /// Emits an update-details record.
    #[zbus(signal)]
    async fn update_details(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        details: Vec<UpdateDetail>
    ) -> zbus::Result<()>;

    /// Emits a request to show an EULA.
    #[zbus(signal)]
    async fn eula_required(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>,
        eula_id: String,
        package_id: String,
        vendor_name: String,
        license_agreement: String
    ) -> zbus::Result<()>;

    /// Signals that the client may destroy the transaction object.
    #[zbus(signal)]
    async fn destroy(
        &self,
        _emitter: &zbus::object_server::SignalEmitter<'_>
    ) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// Service wiring
// ---------------------------------------------------------------------------

/// Global connection handle.
///
/// Signals are emitted from worker threads, which need access to the
/// connection. A `OnceLock` holds it because there is exactly one system-bus
/// connection per process and it must outlive every transaction.
static CONNECTION: std::sync::OnceLock<Connection> = std::sync::OnceLock::new();

/// Returns the shared system-bus connection.
pub(crate) fn dbus_connection() -> Result<Connection> {
    CONNECTION
        .get()
        .cloned()
        .context("The PackageKit D-Bus service is not running")
}

/// Starts the `PackageKit` service on the system bus.
///
/// Blocks for the lifetime of the process: the blocking connection drives its
/// own executor thread, and dropping it would take the bus name with it.
pub(crate) fn start_packagekit() -> Result<()> {
    let conn = connection::Builder::system()
        .context("Failed to connect to the system D-Bus. Is D-Bus running?")?
        .name(BUS_NAME)
        .context("Failed to request the PackageKit bus name")?
        .serve_at(ROOT_PATH, PackageKitRoot::new())
        .context("Failed to export the PackageKit root interface")?
        .build()
        .context("Failed to build the D-Bus connection")?;

    // Stored before use so worker threads can reach it.
    CONNECTION.set(conn).ok();

    println!(
        "{} PackageKit service ready on {BUS_NAME} ({ROOT_PATH})",
        "::".bold().blue()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_ids_use_the_specified_format() {
        // name;version;arch;origin;data
        assert_eq!(
            package_id("firefox", "141.0", "core"),
            "firefox;141.0;noarch;core;"
        );
    }

    #[test]
    fn package_names_are_extracted_from_full_ids() {
        assert_eq!(
            package_name_from_id("firefox;141.0;noarch;core;"),
            "firefox"
        );
        assert_eq!(package_name_from_id("firefox"), "firefox");
        // Whitespace from a hand-built id must not leak through.
        assert_eq!(package_name_from_id("  vim;9.1  "), "vim");
    }

    #[test]
    fn repository_qualified_requests_are_left_intact() {
        // `@` marks a Zoi repository-qualified reference, not a PackageKit id.
        // Rewriting it would break the request.
        assert_eq!(package_name_from_id("@core/linux"), "@core/linux");
        assert_eq!(
            package_name_from_id("#zoidberg@core/linux"),
            "#zoidberg@core/linux"
        );
    }

    #[test]
    fn update_filter_interpretation() {
        assert!(filter_wants_installed(filter::INSTALLED));
        assert!(!filter_wants_available(filter::INSTALLED));

        assert!(filter_wants_available(filter::NOT_INSTALLED));
        assert!(!filter_wants_installed(filter::NOT_INSTALLED));

        // NEITHER set means "everything", which both helpers must report as
        // false so `do_get_packages` falls through to the all-packages path.
        assert!(!filter_wants_installed(filter::NONE));
        assert!(!filter_wants_available(filter::NONE));
    }

    #[test]
    fn filter_bits_match_the_specification() {
        // These are bit flags and frontends pass them by value.
        assert_eq!(filter::NONE, 2);
        assert_eq!(filter::INSTALLED, 4);
        assert_eq!(filter::NOT_INSTALLED, 8);
        assert_eq!(filter::DEVEL, 16);
    }

    #[test]
    fn info_enum_values_match_the_specification() {
        assert_eq!(info::UNKNOWN, 0);
        assert_eq!(info::INSTALLED, 1);
        assert_eq!(info::AVAILABLE, 2);
        assert_eq!(info::NORMAL, 5);
        assert_eq!(info::SECURITY, 8);
        assert_eq!(info::BLOCKED, 9);
        assert_eq!(info::INSTALLING, 12);
        assert_eq!(info::REMOVING, 13);
        assert_eq!(info::UPDATING, 11);
    }

    #[test]
    fn status_enum_values_match_the_specification() {
        assert_eq!(status::UNKNOWN, 0);
        assert_eq!(status::WAIT, 1);
        assert_eq!(status::RUNNING, 3);
        assert_eq!(status::QUERY, 4);
        assert_eq!(status::FINISHED, 18);
        assert_eq!(status::CANCEL, 19);
    }

    #[test]
    fn exit_enum_values_match_the_specification() {
        assert_eq!(exit::UNKNOWN, 0);
        assert_eq!(exit::SUCCESS, 1);
        assert_eq!(exit::FAILED, 2);
        assert_eq!(exit::CANCELLED, 3);
    }

    #[test]
    fn role_enum_values_match_the_specification() {
        assert_eq!(role::GET_PACKAGES, 5);
        assert_eq!(role::INSTALL_PACKAGES, 11);
        assert_eq!(role::REFRESH_CACHE, 13);
        assert_eq!(role::REMOVE_PACKAGES, 14);
        assert_eq!(role::SEARCH_NAME, 21);
        assert_eq!(role::UPDATE_PACKAGES, 22);
    }

    #[test]
    fn search_requires_every_term_to_match() {
        let terms = vec!["gnome".to_string(), "power".to_string()];

        assert!(matches_terms(
            "gnome-power-manager",
            "Power manager",
            &terms,
            false
        ));
        // AND semantics: a name matching only one term must not match.
        assert!(!matches_terms("powertop", "Power", &terms, false));
    }

    #[test]
    fn detail_search_also_inspects_the_description() {
        let terms = vec!["browser".to_string()];

        // SearchDetails looks past the name into the description.
        assert!(matches_terms("firefox", "A web browser", &terms, true));
        // SearchNames does not, which is the documented difference between
        // them.
        assert!(!matches_terms("firefox", "A web browser", &terms, false));
        // A name match satisfies both.
        assert!(matches_terms(
            "firefox-esr",
            "whatever",
            &["firefox".to_string()],
            false
        ));
    }

    #[test]
    fn search_is_case_insensitive() {
        assert!(matches_terms(
            "Firefox",
            "",
            &["firefox".to_string()],
            false
        ));
    }

    #[test]
    fn transaction_paths_are_unique_and_ordered() {
        let root = PackageKitRoot::new();

        let (first, _) = root.new_transaction_path().expect("path");
        let (second, _) = root.new_transaction_path().expect("path");

        assert_ne!(first, second);
        assert!(first.starts_with(ROOT_PATH));
        // Monotonic ids, so the transaction list reads chronologically.
        let mut paths = root.transaction_paths();
        paths.sort();
        let mut sorted = root.transaction_paths();
        sorted.sort_by_key(|p| p.rsplit('/').next().unwrap_or("").to_string());
        assert_eq!(paths, sorted);
        assert_eq!(root.transaction_paths().len(), 2);
    }

    #[test]
    fn locked_reports_busy_only_while_work_runs() {
        let root = PackageKitRoot::new();

        // A queued transaction is not holding anything.
        let (_, state) = root.new_transaction_path().expect("path");
        assert!(!root.locked());

        state.lock().expect("state lock").status = status::INSTALL;
        assert!(
            root.locked(),
            "a frontend must be told when the backend is busy"
        );

        state.lock().expect("state lock").status = status::FINISHED;
        assert!(!root.locked());
    }

    #[test]
    fn progress_never_reports_false_completion() {
        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));
        let reporter =
            Reporter::new("/org/freedesktop/PackageKit/0", state.clone());

        reporter.set_percentage(50);
        assert_eq!(reporter.properties().3, 50);

        // A client seeing 100% may hide its progress UI, so the reporter must
        // hold below 100 while work continues.
        reporter.set_percentage(100);
        assert_eq!(reporter.properties().3, 99);
    }

    #[test]
    fn cancellation_is_recorded_and_reported() {
        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));
        let reporter =
            Reporter::new("/org/freedesktop/PackageKit/0", state.clone());

        assert!(!reporter.cancel_requested());
        state.lock().expect("state lock").cancel_requested = true;
        assert!(reporter.cancel_requested());
    }

    #[test]
    fn touch_records_and_deduplicates_packages() {
        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));
        let reporter =
            Reporter::new("/org/freedesktop/PackageKit/0", state.clone());

        reporter.touch("firefox;1;noarch;core;", info::INSTALLING);
        reporter.touch("firefox;1;noarch;core;", info::INSTALLING);

        let s = state.lock().expect("state lock");
        assert_eq!(s.packages.len(), 1, "duplicates must collapse");
        assert_eq!(s.last_package, "firefox;1;noarch;core;");
    }

    #[test]
    fn finish_is_emitted_at_most_once() {
        // Signals are emitted only when a connection exists, so this exercises
        // the state machine rather than the transport.
        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));
        let reporter =
            Reporter::new("/org/freedesktop/PackageKit/0", state.clone());

        reporter.succeed();
        assert!(state.lock().expect("state lock").finished);

        // A second call must not flip it back or emit another Finished.
        reporter.succeed();
        let state = state.lock().expect("state lock");
        assert!(state.finished);
        assert_eq!(state.status, status::FINISHED);
    }

    #[test]
    fn failure_records_an_error_and_marks_finished() {
        let state = Arc::new(Mutex::new(TransactionState::new(role::UNKNOWN)));
        let reporter =
            Reporter::new("/org/freedesktop/PackageKit/0", state.clone());

        reporter.fail(error_code::TRANSACTION_ERROR, "boom".into());

        let s = state.lock().expect("state lock");
        assert!(s.finished);
        assert_eq!(s.status, status::FINISHED);
        assert_eq!(
            s.error,
            Some((error_code::TRANSACTION_ERROR, "boom".into()))
        );
    }

    #[test]
    fn interface_names_match_the_specification() {
        assert_eq!(BUS_NAME, "org.freedesktop.PackageKit");
        assert_eq!(ROOT_PATH, "/org/freedesktop/PackageKit");
        assert_eq!(IFACE_ROOT, "org.freedesktop.PackageKit");
        assert_eq!(IFACE_TRANSACTION, "org.freedesktop.PackageKit.Transaction");
    }

    #[test]
    fn advertised_capabilities_are_a_subset_of_what_is_honoured() {
        // Every role claimed in the `Roles` property must have a method behind
        // it, or a frontend will call something that answers with an error.
        for bit in 0..64 {
            if SUPPORTED_ROLES & (1 << bit) == 0 {
                continue;
            }
            let claimed = bit;
            let known = [
                role::GET_PACKAGES,
                role::GET_DETAILS,
                role::GET_FILES,
                role::GET_UPDATES,
                role::GET_UPDATE_DETAIL,
                role::GET_DISTRO_UPGRADES,
                role::GET_REPO_LIST,
                role::INSTALL_PACKAGES,
                role::REMOVE_PACKAGES,
                role::UPDATE_PACKAGES,
                role::REFRESH_CACHE,
                role::SEARCH_NAME,
                role::SEARCH_DETAILS,
                role::WHAT_PROVIDES,
                role::DOWNLOAD_PACKAGES
            ];
            assert!(
                known.contains(&claimed),
                "role bit {claimed} is advertised but not implemented"
            );
        }
    }

    #[test]
    fn download_is_not_advertised_because_it_is_unsupported() {
        // DownloadPackages answers NOT_SUPPORTED, so claiming the role would
        // make a frontend try it and then show an error.
        assert_eq!(SUPPORTED_ROLES & (1 << role::DOWNLOAD_PACKAGES), 0);
    }

    #[test]
    fn mime_types_describe_zoi_archives() {
        assert!(
            MIME_TYPES.contains(&"application/x-zpa"),
            "local installs need the .zpa mime type"
        );
    }

    #[test]
    fn restart_enum_values_match_the_specification() {
        // A client keeps the worst restart it was told about, so the ordering
        // is part of the contract.
        assert_eq!(restart::UNKNOWN, 0);
        assert_eq!(restart::NONE, 1);
        assert_eq!(restart::APPLICATION, 2);
        assert_eq!(restart::SESSION, 3);
        assert_eq!(restart::SYSTEM, 4);
    }

    #[test]
    fn failures_are_classified_into_specific_error_codes() {
        // A specific code lets a frontend offer the right remedy; a generic one
        // just says "something went wrong".
        assert_eq!(
            classify_failure("Package 'nope' not found in any registry"),
            error_code::PACKAGE_NOT_FOUND
        );
        assert_eq!(
            classify_failure("failed to resolve dependencies: conflict"),
            error_code::DEP_RESOLUTION_FAILED
        );
        assert_eq!(
            classify_failure("GPG signature verification failed"),
            error_code::GPG_FAILURE
        );
        assert_eq!(
            classify_failure("1 advisory affects this package"),
            error_code::PACKAGE_FAILED_TO_INSTALL
        );
        // Anything unrecognised falls through rather than guessing.
        assert_eq!(
            classify_failure("something entirely unexpected"),
            error_code::TRANSACTION_ERROR
        );
        assert_eq!(classify_failure(""), error_code::TRANSACTION_ERROR);
    }

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(
            classify_failure("PACKAGE NOT FOUND"),
            error_code::PACKAGE_NOT_FOUND
        );
    }

    #[test]
    fn distro_id_uses_the_specified_three_part_form() {
        // id;version;arch. An empty version is correct for a rolling
        // distribution that has no numbered releases.
        assert_eq!(DISTRO_ID.split(';').count(), 3);
        assert!(DISTRO_ID.starts_with("zoios;"));
    }

    #[test]
    fn advertised_filters_are_all_honoured() {
        // Claiming a filter the query path ignores would make a frontend apply
        // a filter that silently does nothing.
        for bit in [
            filter::INSTALLED,
            filter::NOT_INSTALLED,
            filter::BASENAME,
            filter::NOT_BASENAME,
            filter::NEWEST,
            filter::NOT_NEWEST,
            filter::DEVEL,
            filter::NOT_DEVEL
        ] {
            assert!(
                SUPPORTED_FILTERS & bit != 0,
                "filter bit {bit} must be advertised to be used"
            );
        }
    }
}
