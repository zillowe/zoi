//! The alternatives mechanism: choosing between interchangeable
//! implementations.
//!
//! ## Why this exists
//!
//! Some commands have several interchangeable implementations and the system
//! still has to put *something* at the canonical path. `awk` may be `gawk`,
//! `mawk` or `busybox`. `vi` may be `vim`, `nvim` or `vi`. `editor` may be
//! anything.
//!
//! Without a mechanism, the package that happens to install last wins, or the
//! first one hardcodes a choice into `/usr/bin/awk` and every other
//! implementation fights it over the same file. Both are bad: changing the
//! decision means reinstalling packages, and file conflicts between them become
//! routine.
//!
//! ## The layout
//!
//! Mirrors Debian's `update-alternatives`, because that layout is understood
//! and because `/etc/alternatives` is a de-facto interface some software
//! probes:
//!
//! ```text
//! /usr/bin/awk -> /etc/alternatives/awk -> /usr/bin/gawk
//! ```
//!
//! Real files live in their own locations and are never moved. Only symlinks
//! are written, which is what makes switching instant and reversible, and what
//! lets several implementations coexist without touching each other.
//!
//! State lives under `/var/lib/zoi/alternatives/<name>/state.json`, one file
//! per group. A single index file would be simpler but would be rewritten in
//! full on every install and uninstall, so installing two packages would race.
//!
//! ## Choosing a winner
//!
//! Highest priority wins. Ties go to the most recently registered
//! implementation, because that is the one an administrator just installed and
//! therefore the one they most likely meant to make current.
//!
//! `link_set` records a manual override. It is deliberately sticky across
//! re-registration, so installing a higher-priority package does not silently
//! undo a decision the administrator made, and the choice is recoverable via
//! [`auto`](fn@auto).
//!
//! Everything is sysroot-aware, so the same code populates a target tree during
//! `zoi system distro build` and a live root during `zoi install`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::sysroot;
use crate::types::AlternativeEntry;

/// Directory, relative to the system root, holding alternative group state.
const STATE_DIR: &str = "var/lib/zoi/alternatives";

/// Directory, relative to the system root, holding the indirect symlinks.
///
/// `/etc/alternatives` rather than a Zoi-private path, because some software
/// already looks there.
const ETC_ALTERNATIVES_DIR: &str = "etc/alternatives";

/// File, inside a group directory, holding that group's state.
const STATE_FILE: &str = "state.json";

/// One registered implementation of an alternative.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alternative {
    /// Absolute path of the implementation.
    pub path: String,
    /// Priority; higher wins.
    pub priority: i32,
    /// Package that registered this implementation.
    pub package: String,
    /// Registration order within the group, used to break priority ties.
    ///
    /// An explicit counter rather than a timestamp because two registrations
    /// in the same transaction would otherwise collide.
    pub sequence: u64
}

/// State of one alternative group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlternativeGroup {
    /// Group name, e.g. `awk`.
    pub name: String,
    /// Absolute path of the canonical link, e.g. `/usr/bin/awk`.
    pub link: String,
    /// Every registered implementation, in registration order.
    pub alternatives: Vec<Alternative>,
    /// Manually selected path, overriding priority selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_set: Option<String>
}

impl AlternativeGroup {
    /// Returns the implementation currently selected.
    ///
    /// A manual override wins over priority, because that is the whole point of
    /// setting one. When the override names something that is no longer
    /// registered it is ignored rather than producing a dangling symlink.
    pub fn current(&self) -> Option<&Alternative> {
        if let Some(chosen) = &self.link_set
            && let Some(found) =
                self.alternatives.iter().find(|a| &a.path == chosen)
        {
            return Some(found);
        }

        self.best_by_priority()
    }

    /// Returns the highest-priority implementation, breaking ties by the most
    /// recent registration.
    fn best_by_priority(&self) -> Option<&Alternative> {
        self.alternatives
            .iter()
            // `max_by` keeps the *last* maximum, which is exactly the
            // most-recently-registered behaviour wanted on a tie.
            .max_by(|a, b| {
                (a.priority, a.sequence).cmp(&(b.priority, b.sequence))
            })
    }

    /// Returns the state of an alternative, for reporting.
    pub fn status(&self) -> AlternativeStatus {
        let current = self.current();
        let mut manual = false;

        if let Some(chosen) = &self.link_set
            && self.alternatives.iter().any(|a| &a.path == chosen)
        {
            manual = true;
        }

        AlternativeStatus {
            name: self.name.clone(),
            link: self.link.clone(),
            current: current.map(|a| a.path.clone()),
            manual,
            alternatives: self.alternatives.clone()
        }
    }
}

/// A flattened view of a group, for CLI output and for the library API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeStatus {
    /// Group name.
    pub name: String,
    /// Canonical link path.
    pub link: String,
    /// Path currently selected, if any.
    pub current: Option<String>,
    /// Whether the current choice was set manually.
    pub manual: bool,
    /// Every registered implementation.
    pub alternatives: Vec<Alternative>
}

/// Returns the system root the operations apply to.
fn root() -> PathBuf {
    sysroot::get_sysroot().unwrap_or_else(|| PathBuf::from("/"))
}

/// Resolves an absolute path inside the current sysroot.
fn in_root(root: &Path, absolute: &str) -> PathBuf {
    root.join(absolute.trim_start_matches('/'))
}

/// Rejects a path that is not absolute.
///
/// An alternatives entry with a relative path would resolve against whatever
/// directory the caller happened to be in, and the resulting symlink would
/// break the moment that changed. Failing loudly is the only safe response.
fn validate_absolute(field: &str, value: &str) -> Result<()> {
    if !value.starts_with('/') {
        bail!("{field} must be an absolute path, got '{value}'");
    }
    if value.contains("..") {
        bail!("{field} must not contain '..', got '{value}'");
    }
    Ok(())
}

/// Rejects a group name that is not safe to use as a filename.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("An alternative name must not be empty");
    }
    if name.contains('/') || name.contains('\0') {
        bail!("Invalid alternative name '{name}': must not contain '/'");
    }
    Ok(())
}

/// Reads every alternative group.
///
/// Groups are returned in name order so CLI output is stable, which also makes
/// this testable without comparing unordered collections.
///
/// # Errors
///
/// Returns an error only if a group directory exists but its state file cannot
/// be read. A missing state directory is reported as no groups, not an error.
pub fn list() -> Result<Vec<AlternativeGroup>> {
    list_in(&root())
}

/// The root-explicit form of [`list`].
///
/// # Errors
///
/// Propagates a failure to read a group state file. Individual unreadable
/// groups are skipped rather than failing the whole listing, so one bad
/// directory cannot hide every other group.
pub fn list_in(root: &Path) -> Result<Vec<AlternativeGroup>> {
    let dir = root.join(STATE_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        // No state directory simply means nothing has been registered.
        return Ok(Vec::new());
    };

    let mut groups = BTreeMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(group) = read_group(&path) else {
            // A corrupt or half-written group is skipped rather than failing
            // the whole listing; one bad directory must not hide
            // every other group.
            continue;
        };
        groups.insert(group.name.clone(), group);
    }

    Ok(groups.into_values().collect())
}

/// Reads one group by name.
///
/// # Errors
///
/// Returns an error if `name` is not usable as a filename, or if the group
/// exists but its state file cannot be read or parsed.
pub fn get(name: &str) -> Result<Option<AlternativeGroup>> {
    get_in(&root(), name)
}

/// The root-explicit form of [`get`].
///
/// # Errors
///
/// Returns an error if `name` is not usable as a filename, or if the group
/// exists but its state file cannot be read or parsed.
pub fn get_in(root: &Path, name: &str) -> Result<Option<AlternativeGroup>> {
    validate_name(name)?;
    let path = root.join(STATE_DIR).join(name);
    if !path.is_dir() {
        return Ok(None);
    }
    Ok(Some(read_group(&path)?))
}

/// Reads a group from its directory.
fn read_group(dir: &Path) -> Result<AlternativeGroup> {
    let path = dir.join(STATE_FILE);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

/// Writes a group's state.
///
/// Written to a temporary file and renamed, so a crash mid-write cannot leave a
/// truncated `state.json` that would lose every registration in the group.
fn write_group(root: &Path, group: &AlternativeGroup) -> Result<()> {
    let dir = root.join(STATE_DIR).join(&group.name);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create {}", dir.display()))?;

    let final_path = dir.join(STATE_FILE);
    let temp_path = dir.join(format!("{STATE_FILE}.tmp"));

    fs::write(&temp_path, serde_json::to_string_pretty(group)?)
        .with_context(|| format!("Failed to write {}", temp_path.display()))?;
    fs::rename(&temp_path, &final_path).with_context(|| {
        format!("Failed to replace {}", final_path.display())
    })?;

    Ok(())
}

/// Returns the next registration sequence number for a group.
///
/// Derived from the highest sequence already present, so it stays correct after
/// a deregistration removes the most recent entry.
fn next_sequence(group: &AlternativeGroup) -> u64 {
    group
        .alternatives
        .iter()
        .map(|a| a.sequence)
        .max()
        .unwrap_or(0)
        + 1
}

/// Registers the alternatives declared by a package.
///
/// Idempotent per `(name, path)`: re-registering an existing entry refreshes
/// its priority and moves it to the end of the tie-break order rather than
/// creating a duplicate. That matters because a kernel-style upgrade re-runs
/// the registration path.
///
/// # Errors
///
/// Returns an error if an entry has a non-absolute or traversing path, an
/// unusable group name, or if the state directory or a symlink cannot be
/// written.
pub fn register(
    entries: &[AlternativeEntry],
    package: &str
) -> Result<Vec<String>> {
    register_in(&root(), entries, package)
}

/// The root-explicit form of [`register`].
///
/// # Errors
///
/// As [`register`], with paths resolved under `root`.
pub fn register_in(
    root: &Path,
    entries: &[AlternativeEntry],
    package: &str
) -> Result<Vec<String>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut changed = Vec::new();

    // Grouped by name so each group's state is read and written once, rather
    // than once per entry.
    let mut grouped: BTreeMap<&str, Vec<&AlternativeEntry>> = BTreeMap::new();
    for entry in entries {
        validate_name(&entry.name)?;
        validate_absolute("alternatives.path", &entry.path)?;
        validate_absolute("alternatives.link", &entry.link)?;
        grouped.entry(&entry.name).or_default().push(entry);
    }

    for (name, group_entries) in grouped {
        // `grouped` is keyed by name, so this iteration always yields at least
        // one entry. `first()` rather than an index so that is a type-level
        // guarantee instead of a panic waiting for a future refactor.
        let Some(first_entry) = group_entries.first() else {
            continue;
        };

        let mut group = get_in(root, name)?.unwrap_or(AlternativeGroup {
            name: name.to_string(),
            link: first_entry.link.clone(),
            alternatives: Vec::new(),
            link_set: None
        });

        // A group's link is a property of the group, not of one registration.
        // If two packages disagree, the first registration wins and the
        // mismatch is reported rather than silently reassigning the
        // canonical path, which could point `/usr/bin/awk` somewhere a
        // package does not own.
        if group.link != first_entry.link {
            eprintln!(
                "Warning: alternative '{name}' is registered with link '{}' \
                 but '{}' expects '{}'. Keeping '{}'.",
                group.link, package, first_entry.link, group.link
            );
        }

        let before = group.current().map(|a| a.path.clone());

        for entry in group_entries {
            let sequence = next_sequence(&group);

            match group.alternatives.iter_mut().find(|a| a.path == entry.path) {
                Some(existing) => {
                    existing.priority = entry.priority;
                    existing.package = package.to_string();
                    // Re-registration counts as a fresh registration, so a
                    // reinstalled package takes the tie-break.
                    existing.sequence = sequence;
                }
                None => group.alternatives.push(Alternative {
                    path: entry.path.clone(),
                    priority: entry.priority,
                    package: package.to_string(),
                    sequence
                })
            }
        }

        let after = group.current().map(|a| a.path.clone());
        write_group(root, &group)?;
        apply_links(root, &group)?;

        if before != after {
            changed.push(name.to_string());
        }
    }

    Ok(changed)
}

/// Removes every alternative a package registered.
///
/// This is what lets two implementations of the same command coexist: removing
/// one leaves the other selected without any file conflict having occurred.
///
/// # Errors
///
/// Returns an error if a group state file cannot be read or written. A group
/// left with no implementations is removed entirely rather than left dangling.
pub fn deregister(package: &str) -> Result<Vec<String>> {
    deregister_in(&root(), package)
}

/// The root-explicit form of [`deregister`].
///
/// # Errors
///
/// As [`deregister`], with paths resolved under `root`.
pub fn deregister_in(root: &Path, package: &str) -> Result<Vec<String>> {
    let groups = list_in(root)?;
    let mut changed = Vec::new();

    for mut group in groups {
        if !group.alternatives.iter().any(|a| a.package == package) {
            continue;
        }

        let before = group.current().map(|a| a.path.clone());
        group.alternatives.retain(|a| a.package != package);

        // A manual override pointing at the removed package is dropped, or the
        // group would keep selecting something that no longer exists.
        if group.link_set.as_ref().is_some_and(|chosen| {
            group.alternatives.iter().all(|a| &a.path != chosen)
        }) {
            group.link_set = None;
        }

        let after = group.current().map(|a| a.path.clone());

        if group.alternatives.is_empty() {
            // The last implementation is gone, so the whole group goes with it.
            // Leaving behind an empty group would keep a dangling
            // `/etc/alternatives/<name>` that nothing resolves.
            remove_group(root, &group)?;
        } else {
            write_group(root, &group)?;
            apply_links(root, &group)?;
        }

        if before != after {
            changed.push(group.name);
        }
    }

    Ok(changed)
}

/// Deletes a group and the symlinks it owns.
fn remove_group(root: &Path, group: &AlternativeGroup) -> Result<()> {
    let _ = fs::remove_file(in_root(root, &group.link));
    let _ = fs::remove_file(in_root(root, &etc_path(&group.name)?));
    let _ = fs::remove_dir_all(root.join(STATE_DIR).join(&group.name));
    Ok(())
}

/// Manually selects an implementation.
///
/// # Errors
///
/// Returns an error if `name` is not usable as a filename, if no such group
/// exists, or if `path` is not one of the group's registered implementations.
/// Accepting an unknown path would produce a symlink to a file that does not
/// exist.
pub fn set(name: &str, path: &str) -> Result<()> {
    set_in(&root(), name, path)
}

/// The root-explicit form of [`set`].
///
/// # Errors
///
/// As [`set`], with paths resolved under `root`.
pub fn set_in(root: &Path, name: &str, path: &str) -> Result<()> {
    let Some(mut group) = get_in(root, name)? else {
        bail!("No such alternative: '{name}'. Run 'zoi alt list' to see them.");
    };

    if !group.alternatives.iter().any(|a| a.path == path) {
        bail!(
            "'{path}' is not a registered alternative for '{name}'. \
             Available: {}",
            group
                .alternatives
                .iter()
                .map(|a| a.path.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    group.link_set = Some(path.to_string());
    write_group(root, &group)?;
    apply_links(root, &group)?;

    Ok(())
}

/// Drops a manual override, returning to priority-based selection.
///
/// # Errors
///
/// Returns an error if `name` is not usable as a filename, if no such group
/// exists, or if the state cannot be written.
pub fn auto(name: &str) -> Result<Option<String>> {
    auto_in(&root(), name)
}

/// The root-explicit form of [`auto`](fn@auto).
///
/// # Errors
///
/// As [`auto`](fn@auto), with paths resolved under `root`.
pub fn auto_in(root: &Path, name: &str) -> Result<Option<String>> {
    let Some(mut group) = get_in(root, name)? else {
        bail!("No such alternative: '{name}'.");
    };

    group.link_set = None;
    let selected = group.current().map(|a| a.path.clone());

    write_group(root, &group)?;
    apply_links(root, &group)?;

    Ok(selected)
}

/// Removes one implementation from a group.
///
/// Used when a package's file is removed but the package itself stays, and by
/// `zoi alt remove` for correcting a registration by hand.
///
/// # Errors
///
/// Returns an error if `name` is not usable as a filename, if no such group
/// exists, or if `path` is not registered for it.
pub fn remove_implementation(name: &str, path: &str) -> Result<()> {
    remove_implementation_in(&root(), name, path)
}

/// The root-explicit form of [`remove_implementation`].
///
/// # Errors
///
/// As [`remove_implementation`], with paths resolved under `root`.
pub fn remove_implementation_in(
    root: &Path,
    name: &str,
    path: &str
) -> Result<()> {
    let Some(mut group) = get_in(root, name)? else {
        bail!("No such alternative: '{name}'.");
    };

    let before = group.alternatives.len();
    group.alternatives.retain(|a| a.path != path);

    if group.alternatives.len() == before {
        bail!("'{path}' is not registered as an alternative for '{name}'.");
    }

    if group.link_set.as_deref() == Some(path) {
        group.link_set = None;
    }

    if group.alternatives.is_empty() {
        remove_group(root, &group)?;
    } else {
        write_group(root, &group)?;
        apply_links(root, &group)?;
    }

    Ok(())
}

/// Returns the `/etc/alternatives` path for a group.
fn etc_path(name: &str) -> Result<String> {
    validate_name(name)?;
    Ok(format!("/{ETC_ALTERNATIVES_DIR}/{name}"))
}

/// Points a group's symlinks at its selected implementation.
///
/// Two links are maintained: `/etc/alternatives/<name>` at the implementation,
/// and the canonical `<link>` at `/etc/alternatives/<name>`. The indirection is
/// what makes switching a single `rename` rather than a rewrite of the
/// canonical path, and it keeps `/usr/bin/awk` pointing at something stable
/// even while a different implementation is current.
fn apply_links(root: &Path, group: &AlternativeGroup) -> Result<()> {
    let etc_dir = root.join(ETC_ALTERNATIVES_DIR);
    fs::create_dir_all(&etc_dir)
        .with_context(|| format!("Failed to create {}", etc_dir.display()))?;

    let etc_link = etc_dir.join(&group.name);

    if let Some(selected) = group.current() {
        symlink_to(root, &selected.path, &etc_link)?;

        // The canonical link points at the intermediate, never directly at
        // the implementation.
        let canonical = in_root(root, &group.link);
        symlink_to(root, &etc_path(&group.name)?, &canonical)?;
    } else {
        // Nothing registered. Leaving the links behind would produce a
        // dangling `/usr/bin/awk`, which breaks every caller in a way that is
        // much harder to diagnose than a missing command.
        let _ = fs::remove_file(&etc_link);
        let _ = fs::remove_file(in_root(root, &group.link));
    }

    Ok(())
}

/// Creates or replaces a symlink, doing nothing when it already resolves to the
/// target.
///
/// Replacing an identical symlink would change its mtime for no reason, and
/// some integrity tooling treats that as tampering.
fn symlink_to(root: &Path, target: &str, link: &Path) -> Result<()> {
    // `target` is absolute from the system's point of view. Inside a sysroot
    // the link has to be relative to the target root, otherwise the
    // produced symlink would point outside the image being built.
    let effective = if root == Path::new("/") {
        target.to_string()
    } else {
        in_root(root, target).to_string_lossy().to_string()
    };

    if let Ok(existing) = fs::read_link(link)
        && existing.to_string_lossy() == effective
    {
        return Ok(());
    }

    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create {}", parent.display())
        })?;
    }

    // `symlink` fails when the path already exists, so the old link is cleared
    // first. Removing by name rather than following it means a symlink to a
    // directory is replaced rather than having its target emptied.
    match fs::symlink_metadata(link) {
        Ok(meta) if meta.is_dir() => {
            fs::remove_dir_all(link).with_context(|| {
                format!("Failed to replace directory {}", link.display())
            })?;
        }
        Ok(_) => {
            fs::remove_file(link).with_context(|| {
                format!("Failed to remove {}", link.display())
            })?;
        }
        Err(_) => {}
    }

    std::os::unix::fs::symlink(&effective, link).with_context(|| {
        format!("Failed to create symlink {} -> {effective}", link.display())
    })?;

    Ok(())
}

/// Returns the status of every group, for CLI output.
///
/// # Errors
///
/// Propagates a failure to read the state directory listing.
pub fn status() -> Result<Vec<AlternativeStatus>> {
    Ok(list()?.iter().map(AlternativeGroup::status).collect())
}

/// Reconciles the alternatives state for the packages a transaction touched.
///
/// # Errors
///
/// Returns an error if a touched package's group state cannot be read or
/// written, or if a symlink cannot be created. A failure here leaves the
/// previous state intact rather than a partial one, because group state is
/// written to a temporary file and renamed.
pub fn reconcile(touched: &[String]) -> Result<Vec<String>> {
    reconcile_in(&root(), touched)
}

/// The root-explicit form of [`reconcile`].
///
/// # Errors
///
/// As [`reconcile`], with paths resolved under `root`.
pub fn reconcile_in(root: &Path, touched: &[String]) -> Result<Vec<String>> {
    if touched.is_empty() {
        return Ok(Vec::new());
    }

    // Deduplicated and sorted so a package listed twice, or listed in a
    // different order on a different run, produces identical state.
    let mut names: Vec<String> = touched.to_vec();
    names.sort();
    names.dedup();

    let mut changed = Vec::new();

    for name in &names {
        let manifest = find_installed(root, name);

        match manifest {
            Some(manifest) => {
                let entries = manifest.alternatives.unwrap_or_default();

                // Also drop this package's registrations for groups it no
                // longer declares. Without this, removing an
                // alternative from a `.pkg.lua` and
                // reinstalling would leave the old one active.
                let stale = stale_groups(root, name, &entries)?;
                for group in stale {
                    if deregister_group(root, &group, name)? {
                        changed.push(group);
                    }
                }

                if entries.is_empty() {
                    continue;
                }

                changed.extend(register_in(root, &entries, name)?);
            }
            None => {
                changed.extend(deregister_in(root, name)?);
            }
        }
    }

    changed.sort();
    changed.dedup();
    Ok(changed)
}

/// Returns the package store directories to search under `root`.
///
/// Both scopes are included because a user-scope package can legitimately
/// register an alternative: a canonical path inside the user's own `PATH` is
/// just as ambiguous as one under `/usr/bin`.
///
/// `utils::get_store_base_dir` already applies the global sysroot, so it cannot
/// be combined with an explicit `root` without applying the prefix twice. The
/// paths are therefore built directly from `root`, and the platform's system
/// data directory tail is reused so the layout stays in step with the rest of
/// Zoi.
fn store_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // System scope. The tail is taken from the platform-specific system data
    // directory so this does not drift from `utils::get_system_data_dir`.
    dirs.push(join_under(root, Path::new("/var/lib/zoi/pkgs/store")));

    if root == Path::new("/") {
        // On the live system the user store is wherever XDG points, which only
        // `utils` can resolve.
        if let Ok(user_store) =
            crate::utils::get_store_base_dir(crate::types::Scope::User)
        {
            dirs.push(user_store);
        }
    } else {
        // Inside a sysroot the user store is the XDG default under that root.
        // Honoured so a staged image is reconciled the same way the installed
        // system will be.
        dirs.push(root.join(".local/share/zoi/pkgs/store"));
    }

    dirs
}

/// Joins an absolute path under `root`, avoiding a duplicated prefix.
fn join_under(root: &Path, absolute: &Path) -> PathBuf {
    if root == Path::new("/") {
        return absolute.to_path_buf();
    }
    root.join(absolute.strip_prefix("/").unwrap_or(absolute))
}

/// Finds a package's install manifest.
///
/// Reads manifests straight out of the store rather than through the package
/// database, because this has to work inside a sysroot during a distro build
/// where no database is populated.
///
/// The store nests as `<store>/<hash>-<name>/<version>/manifest.json`, where
/// the directory name comes from `utils::get_package_dir_name`. Matching the
/// `-<name>` suffix is therefore what identifies a package.
fn find_installed(
    root: &Path,
    name: &str
) -> Option<crate::types::InstallManifest> {
    let suffix = format!("-{name}");

    for store in store_dirs(root) {
        let Ok(entries) = fs::read_dir(&store) else {
            continue;
        };

        for entry in entries.flatten() {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }

            let dir_name = pkg_dir.file_name().and_then(|n| n.to_str());
            let Some(dir_name) = dir_name else {
                continue;
            };
            if !dir_name.ends_with(&suffix) {
                continue;
            }

            let Ok(versions) = fs::read_dir(&pkg_dir) else {
                continue;
            };

            for version in versions.flatten() {
                let manifest_path = version.path().join("manifest.json");
                let Ok(content) = fs::read_to_string(&manifest_path) else {
                    continue;
                };
                if let Ok(manifest) = serde_json::from_str::<
                    crate::types::InstallManifest
                >(&content)
                {
                    return Some(manifest);
                }
            }
        }
    }

    None
}

/// Returns groups where `package` holds a registration that is no longer
/// declared.
///
/// Returns the group names, so the caller can deregister just those.
fn stale_groups(
    root: &Path,
    package: &str,
    declared: &[AlternativeEntry]
) -> Result<Vec<String>> {
    let groups = list_in(root)?;

    let mut stale = Vec::new();

    for group in groups {
        // Only groups this package still participates in.
        let owned_paths: Vec<&str> = group
            .alternatives
            .iter()
            .filter(|a| a.package == package)
            .map(|a| a.path.as_str())
            .collect();

        if owned_paths.is_empty() {
            continue;
        }

        // Of those, any the package no longer declares in its definition.
        let no_longer_declared = owned_paths.iter().any(|path| {
            !declared
                .iter()
                .any(|entry| entry.name == group.name && entry.path == **path)
        });

        if no_longer_declared {
            stale.push(group.name);
        }
    }

    Ok(stale)
}

/// Deregisters one package from one group.
///
/// Returns whether the selected implementation changed.
fn deregister_group(root: &Path, name: &str, package: &str) -> Result<bool> {
    let Some(mut group) = get_in(root, name)? else {
        return Ok(false);
    };

    let before = group.current().map(|a| a.path.clone());
    group.alternatives.retain(|a| a.package != package);

    if group.link_set.as_ref().is_some_and(|chosen| {
        group.alternatives.iter().all(|a| &a.path != chosen)
    }) {
        group.link_set = None;
    }

    let after = group.current().map(|a| a.path.clone());

    if group.alternatives.is_empty() {
        remove_group(root, &group)?;
    } else {
        write_group(root, &group)?;
        apply_links(root, &group)?;
    }

    Ok(before != after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    fn entry(
        name: &str,
        path: &str,
        link: &str,
        priority: i32
    ) -> AlternativeEntry {
        AlternativeEntry {
            name: name.to_string(),
            path: path.to_string(),
            link: link.to_string(),
            priority
        }
    }

    /// Creates a fake implementation file so symlinks resolve to something
    /// real.
    fn make_binary(root: &Path, path: &str) {
        let full = in_root(root, path);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(&full, "#!/bin/true\n").expect("write");
    }

    fn read_link(root: &Path, path: &str) -> String {
        fs::read_link(in_root(root, path))
            .expect("link")
            .to_string_lossy()
            .to_string()
    }

    /// Returns the currently selected path of a group, failing the test if the
    /// group or its selection is missing.
    ///
    /// Collapses what would otherwise be a chain of three `expect` calls at
    /// every assertion site, and names what was actually missing when it
    /// fails.
    fn current_path_in(root: &Path, name: &str) -> String {
        let group = get_in(root, name)
            .expect("group lookup succeeds")
            .unwrap_or_else(|| panic!("alternative '{name}' should exist"));

        group
            .current()
            .unwrap_or_else(|| {
                panic!("alternative '{name}' should have a selection")
            })
            .path
            .clone()
    }

    #[test]
    fn registration_links_the_canonical_path() {
        let root = temp_root();
        make_binary(root.path(), "/usr/bin/gawk");

        register_in(
            root.path(),
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("register");

        // The two-hop layout is what makes switching safe.
        assert_eq!(
            read_link(root.path(), "/usr/bin/awk"),
            format!("{}/etc/alternatives/awk", root.path().display())
        );
        assert!(
            read_link(root.path(), "/etc/alternatives/awk")
                .ends_with("/usr/bin/gawk")
        );
    }

    #[test]
    fn highest_priority_wins() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/mawk");
        make_binary(&r, "/usr/bin/gawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)],
            "mawk"
        )
        .expect("mawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");

        let group = get_in(&r, "awk").expect("get").expect("group");
        assert_eq!(
            group.current().map(|a| a.path.as_str()),
            Some("/usr/bin/gawk")
        );
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
        );
    }

    #[test]
    fn equal_priority_breaks_tie_on_most_recent() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/nawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/nawk", "/usr/bin/awk", 100)],
            "nawk"
        )
        .expect("nawk");

        // Same priority, so the later registration wins: it is the one the
        // administrator most likely just installed on purpose.
        let group = get_in(&r, "awk").expect("get").expect("group");
        assert_eq!(
            group.current().map(|a| a.path.as_str()),
            Some("/usr/bin/nawk")
        );
    }

    #[test]
    fn manual_selection_beats_priority() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)],
            "mawk"
        )
        .expect("mawk");

        set_in(&r, "awk", "/usr/bin/mawk").expect("set");
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/mawk")
        );

        let group = get_in(&r, "awk").expect("get").expect("group");
        assert!(group.status().manual, "status must report a manual choice");
    }

    #[test]
    fn manual_selection_survives_a_higher_priority_install() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/busybox");

        register_in(
            &r,
            &[entry("vi", "/usr/bin/gawk", "/usr/bin/vi", 100)],
            "gawk"
        )
        .expect("gawk");
        set_in(&r, "vi", "/usr/bin/gawk").expect("set");

        // A much higher priority arrives. Silently overriding the administrator
        // would be the wrong default, so the manual choice stands.
        register_in(
            &r,
            &[entry("vi", "/usr/bin/busybox", "/usr/bin/vi", 500)],
            "busybox"
        )
        .expect("busybox");

        let group = get_in(&r, "vi").expect("get").expect("group");
        assert_eq!(
            group.current().map(|a| a.path.as_str()),
            Some("/usr/bin/gawk"),
            "a manual choice must not be undone by an install"
        );
    }

    #[test]
    fn auto_drops_the_override_and_returns_to_priority() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)],
            "mawk"
        )
        .expect("mawk");
        set_in(&r, "awk", "/usr/bin/mawk").expect("set");

        let chosen = auto_in(&r, "awk").expect("auto");
        assert_eq!(chosen.as_deref(), Some("/usr/bin/gawk"));
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
        );

        let group = get_in(&r, "awk").expect("get").expect("group");
        assert!(!group.status().manual);
    }

    #[test]
    fn deregistering_the_active_implementation_falls_back() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)],
            "mawk"
        )
        .expect("mawk");

        deregister_in(&r, "gawk").expect("deregister");

        // This is the whole point: removing one implementation must not break
        // the command, it must fall back to the other.
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/mawk")
        );
        assert!(
            read_link(&r, "/usr/bin/awk").ends_with("/etc/alternatives/awk")
        );
    }

    #[test]
    fn deregistering_the_last_implementation_removes_the_links() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        deregister_in(&r, "gawk").expect("deregister");

        // A dangling /usr/bin/awk is worse than a missing one.
        assert!(in_root(&r, "/usr/bin/awk").symlink_metadata().is_err());
        assert!(
            in_root(&r, "/etc/alternatives/awk")
                .symlink_metadata()
                .is_err()
        );
        assert!(get_in(&r, "awk").expect("get").is_none());
    }

    #[test]
    fn deregistering_clears_a_manual_choice_of_the_removed_package() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)],
            "mawk"
        )
        .expect("mawk");
        set_in(&r, "awk", "/usr/bin/mawk").expect("set");

        deregister_in(&r, "mawk").expect("deregister");

        let group = get_in(&r, "awk").expect("get").expect("group");
        // The override named something that no longer exists, so it is dropped
        // rather than left to select a missing file.
        assert_eq!(
            group.current().map(|a| a.path.as_str()),
            Some("/usr/bin/gawk")
        );
        assert!(!group.status().manual);
    }

    #[test]
    fn deregistering_an_unrelated_package_changes_nothing() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("gawk");

        let changed = deregister_in(&r, "unrelated").expect("deregister");
        assert_eq!(changed.len(), 0, "changes: {changed:?}");
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
        );
    }

    #[test]
    fn registration_is_idempotent_per_path() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");

        let e = [entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)];
        register_in(&r, &e, "gawk").expect("first");
        register_in(&r, &e, "gawk").expect("second");

        let group = get_in(&r, "awk").expect("get").expect("group");
        // A duplicate would make `zoi alt list` confusing and could
        // double-count during a transition.
        assert_eq!(group.alternatives.len(), 1);
    }

    #[test]
    fn re_registration_refreshes_the_tie_break_position() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/nawk");

        let gawk = [entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)];
        register_in(&r, &gawk, "gawk").expect("gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/nawk", "/usr/bin/awk", 100)],
            "nawk"
        )
        .expect("nawk");
        let current = current_path_in(&r, "awk");
        assert_eq!(current, "/usr/bin/nawk");

        // Reinstalling gawk is a deliberate act, so it should take priority
        // back.
        register_in(&r, &gawk, "gawk").expect("reinstall");
        let current = current_path_in(&r, "awk");
        assert_eq!(current, "/usr/bin/gawk");
    }

    #[test]
    fn multiple_groups_are_tracked_separately() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/vim");
        make_binary(&r, "/usr/bin/nvim");

        register_in(
            &r,
            &[
                entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100),
                entry("editor", "/usr/bin/vim", "/usr/bin/editor", 100),
                entry("vi", "/usr/bin/vim", "/usr/bin/vi", 100)
            ],
            "gvim"
        )
        .expect("register");

        // `vi` and `editor` legitimately point at the same implementation; they
        // are separate groups because they are separate canonical links.
        register_in(
            &r,
            &[entry("editor", "/usr/bin/nvim", "/usr/bin/editor", 50)],
            "neovim"
        )
        .expect("neovim");

        let groups = list_in(&r).expect("list");
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["awk", "editor", "vi"]);
        let second = groups.get(1).expect("two groups");
        assert_eq!(second.alternatives.len(), 2);
    }

    #[test]
    fn relative_paths_are_rejected() {
        let root = temp_root();
        let r = root.path().to_path_buf();

        // A relative path would resolve against the caller's working directory,
        // and the symlink would break the moment that changed.
        let err = register_in(
            &r,
            &[entry("awk", "usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect_err("must reject a relative path");
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn traversal_in_paths_is_rejected() {
        let root = temp_root();
        let r = root.path().to_path_buf();

        assert!(
            register_in(
                &r,
                &[entry(
                    "awk",
                    "/usr/bin/../../etc/shadow",
                    "/usr/bin/awk",
                    100
                )],
                "evil",
            )
            .is_err()
        );
    }

    #[test]
    fn group_names_cannot_escape_the_state_directory() {
        let root = temp_root();
        let r = root.path().to_path_buf();

        // A name with a slash would place state outside `/var/lib/zoi`.
        assert!(
            register_in(
                &r,
                &[entry("../../escape", "/usr/bin/x", "/usr/bin/x", 1)],
                "evil",
            )
            .is_err()
        );
    }

    #[test]
    fn setting_an_unregistered_path_fails() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("register");

        // Accepting it would create a symlink to a file that does not exist.
        let err = set_in(&r, "awk", "/usr/bin/nope").expect_err("must reject");
        assert!(err.to_string().contains("not a registered alternative"));
    }

    #[test]
    fn removing_an_implementation_falls_back() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[
                entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100),
                entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)
            ],
            "combined"
        )
        .expect("register");

        remove_implementation_in(&r, "awk", "/usr/bin/gawk").expect("remove");
        assert!(
            read_link(&r, "/etc/alternatives/awk").ends_with("/usr/bin/mawk")
        );
    }

    #[test]
    fn listing_an_empty_root_is_not_an_error() {
        let root = temp_root();
        assert_eq!(list_in(root.path()).expect("list").len(), 0);
        assert!(get_in(root.path(), "nothing").expect("get").is_none());
    }

    #[test]
    fn symlinks_are_not_rewritten_when_unchanged() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        let e = [entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)];

        register_in(&r, &e, "gawk").expect("first");
        let before = fs::symlink_metadata(in_root(&r, "/usr/bin/awk"))
            .expect("stat")
            .modified()
            .expect("mtime");

        std::thread::sleep(std::time::Duration::from_millis(1100));
        register_in(&r, &e, "gawk").expect("second");
        let after = fs::symlink_metadata(in_root(&r, "/usr/bin/awk"))
            .expect("stat")
            .modified()
            .expect("mtime");

        // Rewriting an identical symlink would look like tampering to integrity
        // tooling for no benefit.
        assert_eq!(before, after);
    }

    #[test]
    fn registering_a_directory_in_the_way_is_handled() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");

        // Something else left a real directory where the canonical link goes.
        fs::create_dir_all(in_root(&r, "/usr/bin/awk")).expect("mkdir");

        register_in(
            &r,
            &[entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)],
            "gawk"
        )
        .expect("register");

        assert!(
            in_root(&r, "/usr/bin/awk")
                .symlink_metadata()
                .expect("stat fixture path")
                .is_symlink()
        );
    }

    #[test]
    fn status_reports_every_field_the_cli_needs() {
        let root = temp_root();
        let r = root.path().to_path_buf();
        make_binary(&r, "/usr/bin/gawk");
        make_binary(&r, "/usr/bin/mawk");

        register_in(
            &r,
            &[
                entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100),
                entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)
            ],
            "combined"
        )
        .expect("register");

        let group = get_in(&r, "awk")
            .expect("group exists")
            .expect("group is present");
        let s = group.status();
        assert_eq!(s.name, "awk");
        assert_eq!(s.link, "/usr/bin/awk");
        assert_eq!(s.current.as_deref(), Some("/usr/bin/gawk"));
        assert!(!s.manual);
        assert_eq!(s.alternatives.len(), 2);
    }
}
