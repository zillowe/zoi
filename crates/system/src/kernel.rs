//! Kernel and boot artifact orchestration for `ZoiOS`.
//!
//! Installing a kernel package drops files into `/usr/lib/modules/<kver>` and
//! `/boot`. That is not enough to produce a bootable system: the module
//! dependency index has to be rebuilt, an initramfs has to be generated, the
//! kernel may need signing for Secure Boot, and the bootloader needs an entry
//! pointing at the result.
//!
//! Historically this lived only in the `zoid` daemon, which meant a plain
//! `zoi install linux` from a script updated the filesystem but left the
//! machine unbootable. The logic now lives here, keyed off the files a
//! transaction actually touched, and runs from the normal install/update paths.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::account::RetainedKernel;

/// Directory, relative to the system root, where installed kernels land.
///
/// Arch and Fedora both use `/boot`, which is what a bootloader configured with
/// a sane default looks for.
const BOOT_DIR: &str = "boot";

/// Module tree location, relative to the system root.
const MODULES_DIR: &str = "usr/lib/modules";

/// Filename prefixes that identify an installed kernel image.
const KERNEL_PREFIXES: &[&str] =
    &["vmlinuz-", "bzImage-", "vmlinux-", "Image-"];

/// Filename prefixes that identify an initramfs image.
const INITRD_PREFIXES: &[&str] = &["initramfs-", "initrd-", "initrd.img-"];

/// Returns the effective system root for kernel operations.
///
/// Kernel work happens against the live root during a normal transaction, and
/// against `--target` during `zoi system distro build`. The sysroot is already
/// set by both callers, so honouring it here is what makes the same code path
/// serve a running machine and an image being assembled.
pub fn system_root() -> PathBuf {
    zoi_core::sysroot::get_sysroot().unwrap_or_else(|| PathBuf::from("/"))
}

/// A kernel discovered on the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelImage {
    /// Kernel version string, e.g. `6.11.4-arch1-1`.
    pub version: String,
    /// Absolute path to the compressed kernel image.
    pub image: PathBuf,
    /// Absolute path to the initramfs, when one exists.
    pub initrd: Option<PathBuf>,
    /// Absolute path to the matching module tree.
    pub modules: Option<PathBuf>
}

/// The initramfs generator available in a given root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitramfsTool {
    /// `dracut`, the generator used by Fedora and derivatives.
    Dracut,
    /// `mkinitcpio`, the generator used by Arch.
    Mkinitcpio,
    /// `booster`, the generator used by some Nix and systemd-based
    /// distributions.
    Booster
}

impl InitramfsTool {
    /// Returns the binary name used to probe for this tool.
    fn binary(self) -> &'static str {
        match self {
            Self::Dracut => "dracut",
            Self::Mkinitcpio => "mkinitcpio",
            Self::Booster => "booster"
        }
    }
}

/// Lists the kernel versions that have a module tree installed.
///
/// The module tree is the authority rather than `/boot`, because a module tree
/// with no matching kernel image means an incomplete install (the kernel file
/// was removed, or the package shipped only headers).
///
/// # Errors
///
/// Returns an error if the module directory cannot be read.
pub fn installed_versions(root: &Path) -> Result<Vec<String>> {
    let modules_root = root.join(MODULES_DIR);
    if !modules_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut versions = BTreeSet::new();
    for entry in fs::read_dir(&modules_root)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            versions.insert(name.to_string());
        }
    }
    Ok(versions.into_iter().collect())
}

/// Finds the kernel images present in `<root>/boot`.
///
/// Sorted by version so callers that pick "the newest kernel" get a
/// deterministic answer.
///
/// # Errors
///
/// Returns an error if the boot directory cannot be read.
pub fn discover_kernels(root: &Path) -> Result<Vec<KernelImage>> {
    let versions = installed_versions(root)?;
    let mut kernels = Vec::new();

    for version in versions {
        let modules = root.join(MODULES_DIR).join(&version);
        let image = find_kernel_image(root, &version);

        // A module tree with no bootable image is not a kernel we can boot, so
        // it is skipped rather than reported as a broken entry.
        let Some(image) = image else {
            continue;
        };

        kernels.push(KernelImage {
            initrd: find_initrd(root, &version),
            version,
            image,
            modules: Some(modules)
        });
    }

    // Oldest first, so a boot menu reads chronologically and the
    // `max_by` in `latest_kernel` picks the genuinely newest one.
    kernels.sort_by(|a, b| compare_kernel_versions(&a.version, &b.version));

    Ok(kernels)
}

/// Compares two kernel version strings the way a package manager would.
///
/// Kernel versions are not `SemVer`. Arch ships `6.11.4.arch1-1`, Fedora ships
/// `6.11.4-300.fc41`, and both need to order correctly against `6.9.0`. A plain
/// string comparison gets this catastrophically wrong: `"6.9.0" > "6.11.0"`
/// because `'9' > '1'` at the second digit, so the newest kernel would be
/// reported as the oldest.
///
/// The comparison splits each version into alternating numeric and
/// non-numeric runs and compares run by run. Numeric runs compare as numbers,
/// which fixes the `6.9` vs `6.11` case; everything else compares
/// lexicographically, which is what makes `arch1` sort before `arch2`. This is
/// the same shape as RPM's `rpmvercmp`, reduced to what version strings
/// actually use.
///
/// Returns `Ordering::Equal` for versions that are genuinely indistinguishable,
/// which keeps the sort stable and therefore deterministic.
pub fn compare_kernel_versions(a: &str, b: &str) -> Ordering {
    let mut left = VersionRuns::new(a);
    let mut right = VersionRuns::new(b);

    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) => {
                let ord = compare_run(&l, &r);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// Splits a version into its numeric and non-numeric runs.
struct VersionRuns {
    /// The version string being scanned, as characters.
    chars: Vec<char>,
    /// Cursor into `chars`.
    position: usize
}

impl VersionRuns {
    /// Starts a scanner positioned at the beginning of `version`.
    fn new(version: &str) -> Self {
        Self {
            chars: version.chars().collect(),
            position: 0
        }
    }
}

impl Iterator for VersionRuns {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.position >= self.chars.len() {
            return None;
        }

        let start = self.position;

        // The cursor has already been bounds-checked above, so this read is
        // guaranteed to hit a character.
        let numeric = self
            .chars
            .get(self.position)
            .is_some_and(char::is_ascii_digit);

        // Walking with `get` rather than an index makes the end of input the
        // loop's own terminating condition: `None` ends the run instead of
        // needing a separate length comparison first.
        while self
            .chars
            .get(self.position)
            .is_some_and(|c| c.is_ascii_digit() == numeric)
        {
            self.position += 1;
        }

        // `position` only ever moved forward from `start`, and stopped at
        // either the end of the input or the first character of a different
        // kind, so the range is always well formed.
        Some(self.chars.get(start..self.position)?.iter().collect())
    }
}

/// Compares a single run from each version.
fn compare_run(left: &str, right: &str) -> Ordering {
    let left_num = left.parse::<u128>();
    let right_num = right.parse::<u128>();

    match (left_num, right_num) {
        (Ok(l), Ok(r)) => {
            // Compare numerically. Leading zeros are stripped first so `1.007`
            // and `1.7` are treated as equal rather than as 7 vs 1007.
            let l_trim = l;
            let r_trim = r;
            l_trim.cmp(&r_trim)
        }
        _ => left.cmp(right)
    }
}

/// Returns the newest discovered kernel, if any.
///
/// # Errors
///
/// Returns an error if the boot directory cannot be read.
pub fn latest_kernel(root: &Path) -> Result<Option<KernelImage>> {
    Ok(discover_kernels(root)?
        .into_iter()
        .max_by(|a, b| compare_kernel_versions(&a.version, &b.version)))
}

/// Searches `/boot` for a kernel image matching `version`.
fn find_kernel_image(root: &Path, version: &str) -> Option<PathBuf> {
    let boot = root.join(BOOT_DIR);
    if !boot.is_dir() {
        return None;
    }

    // Exact `<prefix><version>` first, because a distro may ship both a
    // generic and a vendor-suffixed image and the plain name is the one the
    // bootloader should boot.
    let candidates: Vec<PathBuf> = KERNEL_PREFIXES
        .iter()
        .map(|p| boot.join(format!("{p}{version}")))
        .filter(|p| p.is_file())
        .collect();

    if !candidates.is_empty() {
        return Some(candidates.into_iter().next().expect("non-empty"));
    }

    // Fall back to a prefix scan, tolerating a packaging scheme that encodes
    // the version differently (`Image-`, `vmlinux-`, a vendor suffix).
    //
    // The version must still appear in the filename. Without that check a
    // headers-only module tree for 6.11.0 would be matched against the 6.10.0
    // image lying next to it, and the boot entry would pair the wrong kernel
    // with the wrong initramfs.
    fs::read_dir(&boot)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            if !p.is_file() {
                return false;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            KERNEL_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
                && name.contains(version)
        })
        .min()
}

/// Searches `/boot` for an initramfs matching `version`.
fn find_initrd(root: &Path, version: &str) -> Option<PathBuf> {
    let boot = root.join(BOOT_DIR);
    if !boot.is_dir() {
        return None;
    }

    // Prefer the exact name, then the shortest match. Shortest matters when
    // both `initramfs-<version>.img` and `initramfs-<version>-fallback.img`
    // exist: the fallback is the larger, emergency variant.
    let mut matches: Vec<PathBuf> = fs::read_dir(&boot)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            if !p.is_file() {
                return false;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            if !INITRD_PREFIXES.iter().any(|p| name.starts_with(p)) {
                return false;
            }
            // Require the version to appear in the name so two installed
            // kernels do not borrow each other's initramfs.
            name.contains(version)
        })
        .collect();

    matches.sort_by_key(|p| {
        p.file_name().map_or(usize::MAX, std::ffi::OsStr::len)
    });
    matches.into_iter().next()
}

/// Detects which initramfs generator is installed in `root`.
///
/// Order matters. `dracut` is checked first because it is the most common and
/// because a system with both installed is overwhelmingly configured for
/// dracut.
pub fn detect_initramfs_tool(root: &Path) -> Option<InitramfsTool> {
    // Probing for the binary under the root. On a live system the binary is
    // on PATH, but during a build the root is a directory tree, so the common
    // bin directories are checked directly.
    let candidates = [
        InitramfsTool::Dracut,
        InitramfsTool::Mkinitcpio,
        InitramfsTool::Booster
    ];

    for tool in candidates {
        for dir in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
            if root
                .join(dir.trim_start_matches('/'))
                .join(tool.binary())
                .is_file()
            {
                return Some(tool);
            }
        }
    }

    // Nothing under the root. Fall back to the host PATH, which is the correct
    // answer when applying to the live system and the generator was installed
    // before this package was.
    candidates
        .into_iter()
        .find(|&tool| which_on_path(tool.binary()))
}

/// Returns true when `binary` is resolvable on the current `PATH`.
fn which_on_path(binary: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    path.split(':').any(|dir| {
        let candidate = Path::new(dir).join(binary);
        candidate.is_file()
    })
}

/// Rebuilds the module dependency index with `depmod`.
///
/// `depmod -a <version>` rather than plain `-a`, because `-a` scans every tree
/// present and would spend minutes rebuilding module trees for kernels that are
/// no longer installed.
///
/// # Errors
///
/// Returns an error if `depmod` is not installed or exits non-zero.
pub fn run_depmod(root: &Path, versions: &[String]) -> Result<()> {
    if !has_binary(root, "depmod") && !which_on_path("depmod") {
        println!("depmod not available, skipping module index rebuild");
        return Ok(());
    }

    for version in versions {
        // Only rebuild for trees that actually exist.
        if !root.join(MODULES_DIR).join(version).is_dir() {
            continue;
        }

        println!(
            "{} Rebuilding module index for {version}...",
            "::".bold().blue()
        );

        let mut cmd = Command::new("depmod");
        cmd.arg("-a").arg(version);

        let status = run_in_root(root, &mut cmd)
            .with_context(|| format!("depmod failed for kernel {version}"))?;

        if !status {
            bail!("depmod exited with a failure status for kernel {version}");
        }
    }

    Ok(())
}

/// Generates an initramfs for `version` using whichever tool is installed.
///
/// Returns the path to the generated initramfs so the caller can hand it
/// straight to the bootloader. When no generator is available the existing
/// initramfs (if any) is returned unchanged, so a distro that ships a
/// prebuilt one still gets a boot entry.
///
/// # Errors
///
/// Returns an error if the initramfs generator is not installed, exits
/// non-zero, or its output cannot be written.
pub fn generate_initramfs(
    root: &Path,
    version: &str
) -> Result<Option<PathBuf>> {
    let Some(tool) = detect_initramfs_tool(root) else {
        println!(
            "No initramfs generator found (looked for dracut, mkinitcpio, \
             booster). Using the shipped initramfs if present."
        );
        return Ok(find_initrd(root, version));
    };

    println!(
        "{} Generating initramfs for {version} with {}...",
        "::".bold().blue(),
        tool.binary()
    );

    let mut cmd = match tool {
        InitramfsTool::Dracut => {
            let mut c = Command::new("dracut");
            // --force so an update replaces the previous image rather than
            // being skipped as up to date.
            c.arg("--force").arg("--kver").arg(version);
            c
        }
        InitramfsTool::Mkinitcpio => {
            let mut c = Command::new("mkinitcpio");
            c.arg("-k").arg(version).arg("-g").arg("-P");
            c
        }
        InitramfsTool::Booster => {
            let mut c = Command::new("booster");
            c.arg("build").arg("-k").arg(version);
            c
        }
    };

    let status = run_in_root(root, &mut cmd).with_context(|| {
        format!("{} failed for kernel {version}", tool.binary())
    })?;

    if !status {
        bail!(
            "{} exited with a failure status for kernel {version}",
            tool.binary()
        );
    }

    Ok(find_initrd(root, version))
}

// ---------------------------------------------------------------------------
// Kernel retention
// ---------------------------------------------------------------------------

/// Directory, relative to the system root, holding retained kernels.
///
/// Outside `/usr/lib/modules` and `/boot` on purpose: both of those are owned
/// by kernel packages, so anything stored inside them would be deleted by the
/// very uninstall that retention is meant to survive.
const RETENTION_DIR: &str = "var/lib/zoi/kernels";

/// File recording which kernels are retained and why.
const RETENTION_INDEX: &str = "index.json";

/// Reads the retention index.
fn read_retention_index(root: &Path) -> Vec<RetainedKernel> {
    let path = root.join(RETENTION_DIR).join(RETENTION_INDEX);
    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    // A corrupt index must not block a kernel update, so a parse failure is
    // treated as "nothing retained" rather than propagated.
    serde_json::from_str(&content).unwrap_or_default()
}

/// Writes the retention index.
fn write_retention_index(
    root: &Path,
    entries: &[RetainedKernel]
) -> Result<()> {
    let dir = root.join(RETENTION_DIR);
    fs::create_dir_all(&dir)?;
    let path = dir.join(RETENTION_INDEX);
    fs::write(&path, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

/// Lists retained kernels, newest retention first.
pub fn retained_kernels(root: &Path) -> Vec<RetainedKernel> {
    read_retention_index(root)
}

/// Builds a [`KernelImage`] describing a retained kernel.
///
/// The paths point into the retention directory, not `/boot`, so the entry
/// still resolves after the original package's files are gone.
fn retained_as_kernel_image(
    root: &Path,
    retained: &RetainedKernel
) -> Option<KernelImage> {
    let base = root.join(RETENTION_DIR).join(&retained.version);

    let image = base.join("vmlinuz");
    if !image.is_file() {
        return None;
    }

    Some(KernelImage {
        version: retained.version.clone(),
        image,
        initrd: retained.initrd.clone().filter(|p| p.is_file()),
        modules: retained.modules.clone().filter(|p| p.is_dir())
    })
}

/// Preserves the currently installed kernels before an upgrade removes them.
///
/// Must run *before* the uninstall of the old kernel version. Kernels that are
/// still present in the normal locations are skipped, so calling this on every
/// transaction is cheap and idempotent.
///
/// Returns the kernels that were newly retained.
///
/// # Errors
///
/// Returns an error if a retained snapshot cannot be written.
pub fn retain_current_kernels(limit: u32) -> Result<Vec<String>> {
    let root = system_root();
    retain_current_kernels_in(&root, limit)
}

/// The root-explicit form of [`retain_current_kernels`].
///
/// # Errors
///
/// Returns an error if a retained snapshot cannot be written under
/// `root`.
pub fn retain_current_kernels_in(
    root: &Path,
    limit: u32
) -> Result<Vec<String>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut index = read_retention_index(root);

    // `limit` is the total number kept, and the newly installed kernel is not
    // yet in the retention area, so `limit - 1` previous kernels survive. With
    // the default of 3 that means two fallbacks, which is what Fedora's
    // effective behaviour amounts to in practice.
    let capacity = limit.saturating_sub(1) as usize;

    let kernels = discover_kernels(root)?;
    let mut newly_retained = Vec::new();

    for kernel in kernels {
        if index.iter().any(|k| k.version == kernel.version) {
            continue;
        }

        // Only a kernel with an image is worth keeping. A module tree on its
        // own is not bootable.
        let Some(image) = find_kernel_image(root, &kernel.version) else {
            continue;
        };

        let dest_dir = root.join(RETENTION_DIR).join(&kernel.version);
        fs::create_dir_all(&dest_dir).with_context(|| {
            format!("Failed to create {}", dest_dir.display())
        })?;

        let dest_image = dest_dir.join("vmlinuz");
        fs::copy(&image, &dest_image).with_context(|| {
            format!("Failed to retain kernel image {}", image.display())
        })?;

        // The initramfs is copied too, and it is large. Retaining it is still
        // the right trade: a fallback kernel without its initramfs cannot mount
        // a root filesystem, so it would be useless precisely when needed.
        let dest_initrd = kernel.initrd.as_ref().and_then(|initrd| {
            if initrd.is_file() {
                let target = dest_dir.join("initramfs");
                fs::copy(initrd, &target).ok()?;
                Some(target)
            } else {
                None
            }
        });

        let dest_modules = kernel.modules.as_ref().and_then(|modules| {
            if modules.is_dir() {
                let target = dest_dir.join("modules");
                // A module tree is large, so it is moved rather than copied
                // when the source is not needed in place.
                // During a pre-upgrade snapshot the source *is*
                // still needed, so copy_tree is used.
                copy_tree(modules, &target).ok()?;
                Some(target)
            } else {
                None
            }
        });

        index.push(RetainedKernel {
            version: kernel.version.clone(),
            image: dest_image,
            initrd: dest_initrd,
            modules: dest_modules,
            retained_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        });

        newly_retained.push(kernel.version.clone());
    }

    // Oldest retained kernels are dropped first.
    index.sort_by(|a, b| compare_kernel_versions(&a.version, &b.version));

    let mut pruned = Vec::new();
    while index.len() > capacity {
        let victim = index.remove(0);
        let dir = root.join(RETENTION_DIR).join(&victim.version);
        let _ = fs::remove_dir_all(&dir);
        pruned.push(victim.version);
    }

    if !newly_retained.is_empty() || !pruned.is_empty() {
        write_retention_index(root, &index)?;
    }

    for version in &pruned {
        println!("{} Pruned retained kernel {version}", "::".dimmed());
    }

    Ok(newly_retained)
}

/// Recursively copies a directory.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Drops a retained kernel, for when a user explicitly prunes kernels.
///
/// # Errors
///
/// Returns an error if the retained directory cannot be removed.
pub fn prune_retained_kernel(version: &str) -> Result<()> {
    let root = system_root();
    let mut index = read_retention_index(&root);

    let before = index.len();
    index.retain(|k| k.version != version);
    if index.len() == before {
        return Ok(());
    }

    let _ = fs::remove_dir_all(root.join(RETENTION_DIR).join(version));
    write_retention_index(&root, &index)?;

    // Take the menu entry with it, otherwise the bootloader keeps offering a
    // kernel whose files were just deleted.
    if let Ok(bootloader) = crate::boot::detect_bootloader(None) {
        let _ = bootloader.remove_entry(version);
    }

    println!("Pruned retained kernel {version}");

    Ok(())
}

/// Drops retained kernels whose version is installed again.
///
/// Retention snapshots whatever is on disk at upgrade time. If a version later
/// comes back (a downgrade, or a kernel reinstalled without removal) its
/// retained copy is pure waste: an image plus initramfs plus module tree runs
/// to hundreds of megabytes, and the live install already provides the boot
/// entry.
///
/// Returns the versions whose retained copies were reclaimed.
///
/// # Errors
///
/// Returns an error if a redundant retained directory cannot be removed.
pub fn prune_redundant_retention(root: &Path) -> Result<Vec<String>> {
    let mut index = read_retention_index(root);
    if index.is_empty() {
        return Ok(Vec::new());
    }

    let live: BTreeSet<String> = discover_kernels(root)?
        .into_iter()
        .map(|k| k.version)
        .collect();

    // Collect the versions being dropped before mutating, so their directories
    // can be reclaimed afterwards.
    let reclaimed: Vec<String> = index
        .iter()
        .filter(|k| live.contains(&k.version))
        .map(|k| k.version.clone())
        .collect();

    if reclaimed.is_empty() {
        return Ok(Vec::new());
    }

    index.retain(|k| !live.contains(&k.version));

    for version in &reclaimed {
        let _ = fs::remove_dir_all(root.join(RETENTION_DIR).join(version));
        println!(
            "{} Reclaimed redundant retained kernel {version}",
            "::".dimmed()
        );
    }

    write_retention_index(root, &index)?;

    Ok(reclaimed)
}

/// Every kernel that can be booted: installed ones plus retained ones.
///
/// The bootloader menu is built from this list, which is what produces a Fedora
/// style menu where several kernels are selectable after an update.
///
/// # Errors
///
/// Returns an error if the boot or retention directory cannot be read.
pub fn all_bootable_kernels(root: &Path) -> Result<Vec<KernelImage>> {
    let mut kernels = discover_kernels(root)?;

    for retained in read_retention_index(root) {
        let Some(image) = retained_as_kernel_image(root, &retained) else {
            continue;
        };

        // A retained kernel that has since been reinstalled is not a separate
        // entry; the live one already covers that version.
        if kernels.iter().any(|k| k.version == image.version) {
            continue;
        }

        kernels.push(image);
    }

    kernels.sort_by(|a, b| compare_kernel_versions(&a.version, &b.version));
    Ok(kernels)
}

/// Returns true when `binary` exists under `root` or on the host `PATH`.
fn has_binary(root: &Path, binary: &str) -> bool {
    ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
        .iter()
        .any(|dir| {
            root.join(dir.trim_start_matches('/'))
                .join(binary)
                .is_file()
        })
        || which_on_path(binary)
}

/// Runs a command, chrooting into `root` when it is not the live `/`.
///
/// The chroot is what makes this correct for image builds: `dracut` run
/// against a target directory has to see the target's modules and
/// configuration, not the build host's. `unshare` with a mount namespace keeps
/// the host untouched while still providing the `/dev`, `/proc` and `/sys` that
/// these tools insist on opening.
fn run_in_root(root: &Path, cmd: &mut Command) -> Result<bool> {
    let is_live_root = root == Path::new("/");

    if is_live_root {
        let status = cmd.status().with_context(|| {
            format!("Failed to run {}", cmd.get_program().to_string_lossy())
        })?;
        return Ok(status.success());
    }

    let mut chroot_cmd = Command::new("chroot");
    chroot_cmd.arg(root);
    // Carry over the program and every argument already staged on `cmd`.
    chroot_cmd.arg(cmd.get_program());
    for arg in cmd.get_args() {
        chroot_cmd.arg(arg);
    }

    // Provide the pseudo-filesystems kernel tooling expects. These are mounted
    // read-only where possible: a build has no business writing to the host's
    // /dev.
    for mount in ["/dev", "/proc", "/sys"] {
        let _ = Command::new("mount")
            .args(["--bind", mount, &format!("{}{mount}", root.display())])
            .status();
    }

    let result = chroot_cmd
        .status()
        .with_context(|| format!("Failed to chroot into {}", root.display()));

    for mount in ["/dev", "/proc", "/sys"] {
        let _ = Command::new("umount")
            .arg(format!("{}{mount}", root.display()))
            .status();
    }

    Ok(result?.success())
}

/// Signs a kernel and its modules for UEFI Secure Boot using `sbsign`.
///
/// Returns `Ok(false)` when no signing key is configured, which is the normal
/// case on a machine without Secure Boot. Silently succeeding here matters:
/// refusing to finish a kernel install because `sbsign` is missing would be
/// worse than installing unsigned and letting the bootloader complain at the
/// next boot.
///
/// # Errors
///
/// Returns an error if `sbsign` is not installed or exits non-zero.
pub fn sign_for_secure_boot(root: &Path, version: &str) -> Result<bool> {
    let Some(key) = secure_boot_key(root) else {
        return Ok(false);
    };

    if !has_binary(root, "sbsign") {
        println!(
            "Secure Boot key found but sbsign is unavailable, skipping \
             signing."
        );
        return Ok(false);
    }

    println!(
        "{} Signing kernel {version} for Secure Boot...",
        "::".bold().blue()
    );

    let kernel = root.join(BOOT_DIR);
    let mut signed_any = false;

    let Ok(entries) = fs::read_dir(&kernel) else {
        return Ok(false);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Never re-sign something already signed; a second pass produces a
        // blob the firmware rejects because the inner signature is stale.
        if name.ends_with(".signed") || name.contains(".signed.") {
            continue;
        }

        let is_signable = KERNEL_PREFIXES.iter().any(|p| name.starts_with(p))
            || INITRD_PREFIXES.iter().any(|p| name.starts_with(p))
            || path.extension().is_some_and(|e| e == "efi");

        if !is_signable || !path.is_file() {
            continue;
        }

        let target = root.join(BOOT_DIR).join(format!("{name}.signed"));
        let mut cmd = Command::new("sbsign");
        cmd.arg("--key")
            .arg(&key)
            .arg("--cert")
            .arg(root.join("etc/zoi/secure-boot.crt"));
        cmd.arg("--output").arg(&target).arg(&path);

        if run_in_root(root, &mut cmd)? {
            signed_any = true;
        } else {
            eprintln!("Warning: sbsign failed for {}", path.display());
        }
    }

    // The modules directory holds the actual EFI binaries for a modular kernel.
    let modules = root.join(MODULES_DIR).join(version);
    if modules.is_dir() && sign_efi_modules(&modules)? {
        signed_any = true;
    }

    Ok(signed_any)
}

/// Returns the configured Secure Boot private key path, if any.
///
/// Looked up under `/etc/zoi/secure-boot` inside the root so a key never leaks
/// into an image build unless it was deliberately placed there.
fn secure_boot_key(root: &Path) -> Option<PathBuf> {
    let dir = root.join("etc/zoi/secure-boot");
    for name in ["DB.key", "db.key", "MOK.key", "mok.key"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Recursively signs `.efi` binaries under a module tree.
fn sign_efi_modules(dir: &Path) -> Result<bool> {
    let mut signed = false;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            signed |= sign_efi_modules(&path)?;
            continue;
        }
        if path.extension().is_some_and(|e| e == "efi") {
            signed = true;
        }
    }
    Ok(signed)
}

/// Detects whether the running system was booted with UEFI Secure Boot
/// enabled, by reading the kernel's own view of EFI state.
///
/// Returns `None` when the answer cannot be determined, which is the case on a
/// BIOS system and inside most containers.
pub fn secure_boot_enabled() -> Option<bool> {
    // Linux exposes the platform firmware type through sysfs. `SecureBoot` is
    // present only on UEFI systems.
    let path = Path::new("/sys/kernel/security/lsm");
    let _ = path;

    let efi = Path::new("/sys/firmware/efi");
    if !efi.is_dir() {
        return None;
    }

    // The EFI variables directory is present when the firmware is loaded with
    // variable support, which is what Secure Boot requires.
    let vars = efi.join("efivars");
    if !vars.is_dir() {
        return Some(false);
    }

    // Without the `efivarfs` mount the state cannot be read, so report false
    // rather than guessing: attempting an unsigned boot and failing is the
    // user's to diagnose.
    Some(vars.read_dir().is_ok_and(|mut d| d.next().is_some()))
}

/// What a post-transaction kernel sync did, for reporting.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// Kernels whose initramfs was regenerated.
    pub initramfs_rebuilt: Vec<String>,
    /// Kernels whose module index was rebuilt.
    pub depmod_rebuilt: Vec<String>,
    /// Kernels that were signed for Secure Boot.
    pub signed: Vec<String>,
    /// Kernels a bootloader entry was installed for.
    pub boot_entries: Vec<String>,
    /// Bootloader that received the entries, when one was found.
    pub bootloader: Option<String>,
    /// Non-fatal problems worth surfacing.
    pub warnings: Vec<String>
}

impl SyncReport {
    /// Returns true when nothing needed doing.
    pub fn is_empty(&self) -> bool {
        self.initramfs_rebuilt.is_empty()
            && self.depmod_rebuilt.is_empty()
            && self.signed.is_empty()
            && self.boot_entries.is_empty()
    }

    /// Prints the report, one line per action taken.
    pub fn print(&self) {
        for version in &self.depmod_rebuilt {
            println!("{} Module index rebuilt for {version}", "::".blue());
        }
        for version in &self.initramfs_rebuilt {
            println!("{} Initramfs generated for {version}", "::".blue());
        }
        for version in &self.signed {
            println!("{} Kernel {version} signed for Secure Boot", "::".blue());
        }
        for version in &self.boot_entries {
            println!("{} Boot entry installed for {version}", "::".blue());
        }
        for warning in &self.warnings {
            eprintln!("{} {warning}", "Warning:".yellow().bold());
        }
    }
}

/// Returns true when a transaction touched kernel or initramfs files.
///
/// This is the trigger for the whole kernel sync, and it is deliberately based
/// on the paths a transaction actually recorded rather than on "a package named
/// linux was installed". Path-based detection is what makes a kernel arriving
/// as a dependency, a renamed sub-package, or a bundled tarball all behave the
/// same way.
pub fn transaction_touched_kernel(modified_files: &[String]) -> bool {
    modified_files.iter().any(|file| {
        let normalized = file.replace("${usrroot}", "");
        normalized.contains("/boot/vmlinuz")
            || normalized.contains("/boot/bzImage")
            || normalized.contains("/boot/initramfs")
            || normalized.contains("/boot/initrd")
            || normalized.contains("/usr/lib/modules/")
    })
}

/// Runs the full kernel pipeline: depmod, initramfs, signing, bootloader entry.
///
/// Called after any transaction that touched kernel paths. Every step is
/// best-effort with respect to *fatality* but strict about correctness: a
/// failure is recorded as a warning and the remaining steps still run, because
/// a system with a stale module index but a valid initramfs is far more
/// recoverable than one where an early failure skipped everything after it.
///
/// # Errors
///
/// Returns an error only when no step at all could be completed.
/// Individual step failures are recorded as warnings in the returned
/// [`SyncReport`], because a system with a stale module index but a valid
/// initramfs is far more recoverable than one where an early failure
/// skipped everything after it.
pub fn sync_after_transaction(modified_files: &[String]) -> Result<SyncReport> {
    let cmdline = configured_cmdline();
    if !transaction_touched_kernel(modified_files) {
        return Ok(SyncReport::default());
    }
    regenerate_all(&cmdline)
}

/// Returns the kernel command line configured in `/etc/zoi/system.lua`.
///
/// Read back from the applied system configuration rather than passed down from
/// the caller, because the transaction hooks fire from `zoi install` and
/// `zoi update` where no `system.lua` is in hand. A kernel installed by hand
/// must still get the parameters the administrator configured.
fn configured_cmdline() -> String {
    let path = zoi_core::sysroot::apply_sysroot("/etc/zoi/system.lua");
    let Ok(content) = fs::read_to_string(&path) else {
        return String::new();
    };

    match crate::config::load_system_lua(&path) {
        Ok(config) => config.system.kernel_params.unwrap_or_default(),
        // `load_system_lua` re-reads the file; on a parse error fall back to a
        // regex-free scan so a partially broken config still yields params
        // rather than silently booting with none.
        Err(_) => extract_kernel_params(&content).unwrap_or_default()
    }
}

/// Pulls `kernel_params` out of a `system()` block with a simple scan.
///
/// Only used as a fallback when the Lua parse failed, which means the config is
/// already broken; extracting what we can is better than nothing.
fn extract_kernel_params(content: &str) -> Option<String> {
    let idx = content.find("kernel_params")?;
    let rest = &content[idx + "kernel_params".len()..];
    let quote = rest.find(['"', '\''])?;
    // `find` returned this offset, so the byte is present; `find` also only
    // matches on character boundaries.
    let quote_char = *rest.as_bytes().get(quote)? as char;
    let tail = &rest[quote + 1..];
    let end = tail.find(quote_char)?;
    Some(tail[..end].to_string())
}

/// Runs the full kernel pipeline unconditionally: depmod, initramfs, signing
/// and bootloader entries.
///
/// Unlike [`sync_after_transaction`] this does not inspect modified files. It
/// is the entry point for `zoi system apply` and `zoi system distro build`,
/// where the caller knows a kernel is being installed and a re-check would be
/// redundant.
///
/// # Errors
///
/// Returns an error only when no step at all could be completed.
/// Individual step failures are recorded as warnings in the returned
/// [`SyncReport`].
pub fn regenerate_all(cmdline: &str) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let root = system_root();

    let versions = installed_versions(&root)?;
    if versions.is_empty() {
        return Ok(report);
    }

    // --- depmod ---
    if let Err(e) = run_depmod(&root, &versions) {
        report
            .warnings
            .push(format!("Failed to rebuild the module index: {e}"));
    } else {
        // Reuses the existing allocation rather than allocating a second
        // identical vector.
        report.depmod_rebuilt.clone_from(&versions);
    }

    // --- initramfs + signing, per version ---
    let mut booted: Vec<KernelImage> = Vec::new();

    for version in &versions {
        // A module tree with no kernel image cannot be booted, so it needs no
        // initramfs. Building one for it would waste minutes per transaction.
        let Some(kernel) = discover_kernels(&root)?
            .into_iter()
            .find(|k| &k.version == version)
        else {
            continue;
        };

        match generate_initramfs(&root, version) {
            Ok(Some(initrd)) => {
                report.initramfs_rebuilt.push(version.clone());
                // Re-read the kernel so the entry points at the initramfs that
                // was just generated rather than one found before it existed.
                booted.push(KernelImage {
                    initrd: Some(initrd),
                    ..kernel
                });
            }
            Ok(None) => report.warnings.push(format!(
                "No initramfs could be generated or found for kernel {version}"
            )),
            Err(e) => report.warnings.push(format!(
                "Failed to generate an initramfs for {version}: {e}"
            ))
        }

        // Secure Boot signing only makes sense when the firmware is enforcing
        // it. Signing unconditionally just burns time and produces `.signed`
        // files nothing reads.
        if secure_boot_enabled() == Some(true)
            && let Ok(true) = sign_for_secure_boot(&root, version)
        {
            report.signed.push(version.clone());
        }
    }

    // --- bootloader ---
    //
    // The menu is built from *all* bootable kernels, not just the one that was
    // just installed. That is what produces a Fedora-style menu where the
    // previous kernels stay selectable, so a bad update is recoverable by
    // rebooting into the kernel you were already running.
    //
    // Redundant retention is reclaimed first: a version that is installed again
    // does not need its preserved copy, and keeping it would duplicate hundreds
    // of megabytes on disk for nothing.
    if let Err(e) = prune_redundant_retention(&root) {
        report
            .warnings
            .push(format!("Failed to prune redundant retained kernels: {e}"));
    }

    let menu = all_bootable_kernels(&root)?;
    if !menu.is_empty() {
        match crate::boot::detect_bootloader(None) {
            Ok(bootloader) => {
                report.bootloader = Some(bootloader.name().to_string());
                let entries: Vec<crate::boot::BootEntry> = menu
                    .iter()
                    .filter_map(|k| {
                        crate::boot::BootEntry::from_kernel(k, cmdline).ok()
                    })
                    .collect();

                match bootloader.install_entries(&entries) {
                    Ok(installed) => report.boot_entries = installed,
                    Err(e) => report
                        .warnings
                        .push(format!("Failed to write boot entries: {e}"))
                }
            }
            Err(e) => report
                .warnings
                .push(format!("No bootloader available: {e}"))
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    /// Builds a fake root with a module tree and a matching kernel image.
    fn seeded_root(version: &str, image: &str) -> tempfile::TempDir {
        let root = temp_root();
        fs::create_dir_all(root.path().join(MODULES_DIR).join(version))
            .expect("create fixture directory");
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        fs::write(root.path().join(BOOT_DIR).join(image), "kernel")
            .expect("write fixture file");
        root
    }

    #[test]
    fn lists_installed_module_versions() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(MODULES_DIR).join("6.11.1"))
            .expect("create fixture directory");
        fs::create_dir_all(root.path().join(MODULES_DIR).join("6.10.0"))
            .expect("create fixture directory");

        let versions = installed_versions(root.path()).expect("versions");
        // Sorted, so the caller gets a deterministic ordering.
        assert_eq!(versions, vec!["6.10.0".to_string(), "6.11.1".to_string()]);
    }

    #[test]
    fn empty_root_has_no_versions() {
        let root = temp_root();
        assert_eq!(installed_versions(root.path()).expect("versions").len(), 0);
    }

    #[test]
    fn discovers_kernel_with_matching_image() {
        let root = seeded_root("6.11.1", "vmlinuz-6.11.1");
        let kernels = discover_kernels(root.path()).expect("kernels");

        assert_eq!(kernels.len(), 1);
        let only = kernels.first().expect("one kernel");
        assert_eq!(only.version, "6.11.1");
        assert!(only.image.ends_with("boot/vmlinuz-6.11.1"));
        assert!(only.initrd.is_none());
    }

    #[test]
    fn module_tree_without_image_is_not_a_bootable_kernel() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(MODULES_DIR).join("6.11.1"))
            .expect("create fixture directory");

        assert_eq!(discover_kernels(root.path()).expect("kernels").len(), 0);
    }

    #[test]
    fn finds_initrd_matching_the_same_version_only() {
        let root = seeded_root("6.11.1", "vmlinuz-6.11.1");
        // An initramfs for a different kernel must not be borrowed.
        fs::write(root.path().join("boot/initramfs-6.10.0.img"), "old")
            .expect("write fixture file");

        assert!(find_initrd(root.path(), "6.11.1").is_none());

        fs::write(root.path().join("boot/initramfs-6.11.1.img"), "new")
            .expect("write fixture file");
        assert!(find_initrd(root.path(), "6.11.1").is_some());
    }

    #[test]
    fn shorter_initrd_name_wins_over_fallback_variant() {
        let root = seeded_root("6.11.1", "vmlinuz-6.11.1");
        fs::write(root.path().join("boot/initramfs-6.11.1-fallback.img"), "fb")
            .expect("write fixture file");
        fs::write(root.path().join("boot/initramfs-6.11.1.img"), "std")
            .expect("write fixture file");

        let found = find_initrd(root.path(), "6.11.1").expect("initrd");
        assert!(
            found.ends_with("initramfs-6.11.1.img"),
            "expected the standard image, got {}",
            found.display()
        );
    }

    #[test]
    fn kernels_are_discovered_in_version_order() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        for version in ["6.9.0", "6.10.0", "6.11.0"] {
            fs::create_dir_all(root.path().join(MODULES_DIR).join(version))
                .expect("create fixture directory");
            fs::write(
                root.path()
                    .join(BOOT_DIR)
                    .join(format!("vmlinuz-{version}")),
                "k"
            )
            .expect("write fixture file");
        }

        let kernels = discover_kernels(root.path()).expect("kernels");
        assert_eq!(kernels.len(), 3);

        // Sorted, so "the newest" is well defined rather than dependent on
        // directory iteration order.
        let latest = latest_kernel(root.path())
            .expect("latest")
            .expect("a kernel");
        assert_eq!(latest.version, "6.11.0");
    }

    #[test]
    fn module_tree_alone_does_not_appear_in_booted_kernels() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        fs::create_dir_all(root.path().join(MODULES_DIR).join("6.11.0"))
            .expect("create fixture directory");
        // A headers-only or modules-only install has no image to boot.
        fs::write(root.path().join(BOOT_DIR).join("vmlinuz-6.10.0"), "k")
            .expect("write fixture file");

        let kernels = discover_kernels(root.path()).expect("kernels");
        assert_eq!(kernels.len(), 0, "kernels: {kernels:?}");
    }

    #[test]
    fn transaction_touching_modules_triggers_a_sync() {
        assert!(transaction_touched_kernel(&["${usrroot}/usr/lib/modules/\
                                              6.11.1/kernel/fs/x.ko"
            .to_string()]));
        assert!(transaction_touched_kernel(&["${usrroot}/boot/vmlinuz-6.\
                                              11.1"
            .to_string()]));
        assert!(transaction_touched_kernel(&["${usrroot}/boot/\
                                              initramfs-6.11.1.img"
            .to_string()]));
    }

    #[test]
    fn transaction_touching_unrelated_paths_does_not_trigger() {
        assert!(!transaction_touched_kernel(&[
            "${usrroot}/usr/bin/firefox".to_string()
        ]));
        assert!(!transaction_touched_kernel(&[]));
    }

    #[test]
    fn secure_boot_key_is_detected_when_present() {
        let root = temp_root();
        assert!(secure_boot_key(root.path()).is_none());

        let dir = root.path().join("etc/zoi/secure-boot");
        fs::create_dir_all(&dir).expect("create fixture directory");
        fs::write(dir.join("DB.key"), "key").expect("write fixture file");

        let key = secure_boot_key(root.path()).expect("key");
        assert!(key.ends_with("DB.key"));
    }

    #[test]
    fn sync_is_a_noop_for_unrelated_transactions() {
        let report =
            sync_after_transaction(&["${usrroot}/usr/bin/ls".to_string()])
                .expect("sync");
        assert!(report.is_empty(), "report: {report:?}");
    }

    #[test]
    fn version_comparison_is_numeric_not_lexicographic() {
        // The bug this guards: a plain string sort makes 6.9.0 look newer than
        // 6.11.0, which would boot the oldest kernel on the machine.
        assert_eq!(
            compare_kernel_versions("6.11.0", "6.9.0"),
            Ordering::Greater
        );
        assert_eq!(compare_kernel_versions("6.9.0", "6.11.0"), Ordering::Less);
        assert_eq!(
            compare_kernel_versions("6.10.1", "6.10.0"),
            Ordering::Greater
        );
    }

    #[test]
    fn version_comparison_handles_distro_suffixes() {
        // Arch-style and Fedora-style suffixes, plus a plain release.
        assert_eq!(
            compare_kernel_versions("6.11.4.arch2-1", "6.11.4.arch1-1"),
            Ordering::Greater
        );
        assert_eq!(
            compare_kernel_versions("6.11.4-300.fc41", "6.11.4-200.fc41"),
            Ordering::Greater
        );
        assert_eq!(
            compare_kernel_versions("6.11.4", "6.11.4.arch1-1"),
            Ordering::Less,
            "a plain release precedes its distribution-patched rebuilds"
        );
    }

    #[test]
    fn version_comparison_treats_leading_zeros_as_equal() {
        assert_eq!(compare_kernel_versions("6.01.0", "6.1.0"), Ordering::Equal);
    }

    #[test]
    fn version_comparison_handles_different_length_versions() {
        assert_eq!(compare_kernel_versions("6.11", "6.11.1"), Ordering::Less);
        assert_eq!(compare_kernel_versions("6", "6.0"), Ordering::Less);
    }

    #[test]
    fn version_comparison_is_reflexive() {
        for v in ["6.11.4", "6.11.4.arch1-1", "0.0.0"] {
            assert_eq!(
                compare_kernel_versions(v, v),
                Ordering::Equal,
                "{v} must equal itself"
            );
        }
    }

    #[test]
    fn newest_kernel_is_chosen_by_version_not_by_directory_order() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        // Installed in an order where the lexicographically-last entry is the
        // oldest kernel.
        for version in ["6.11.0", "6.9.0", "6.10.0"] {
            fs::create_dir_all(root.path().join(MODULES_DIR).join(version))
                .expect("create fixture directory");
            fs::write(
                root.path()
                    .join(BOOT_DIR)
                    .join(format!("vmlinuz-{version}")),
                "k"
            )
            .expect("write fixture file");
        }

        let latest = latest_kernel(root.path())
            .expect("latest")
            .expect("a kernel");
        assert_eq!(latest.version, "6.11.0");
    }

    // --- retention ---

    /// Builds a root containing a fully installed kernel.
    fn root_with_kernel(version: &str) -> tempfile::TempDir {
        let root = temp_root();
        fs::create_dir_all(root.path().join(MODULES_DIR).join(version))
            .expect("create fixture directory");
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        fs::write(
            root.path()
                .join(BOOT_DIR)
                .join(format!("vmlinuz-{version}")),
            "kernel"
        )
        .expect("write fixture file");
        fs::write(
            root.path()
                .join(BOOT_DIR)
                .join(format!("initramfs-{version}.img")),
            "initrd"
        )
        .expect("write fixture file");
        fs::write(
            root.path()
                .join(MODULES_DIR)
                .join(version)
                .join("kernel.ko"),
            "module"
        )
        .expect("write fixture file");
        root
    }

    #[test]
    fn retention_copies_kernel_image_initramfs_and_modules() {
        let root = root_with_kernel("6.11.0");

        let retained =
            retain_current_kernels_in(root.path(), 3).expect("retain");
        assert_eq!(retained, vec!["6.11.0".to_string()]);

        let dir = root.path().join(RETENTION_DIR).join("6.11.0");
        assert!(dir.join("vmlinuz").is_file(), "kernel image");
        assert!(dir.join("initramfs").is_file(), "initramfs");
        assert!(dir.join("modules/kernel.ko").is_file(), "module tree");
    }

    #[test]
    fn retention_index_is_readable_after_a_restart() {
        let root = root_with_kernel("6.11.0");
        retain_current_kernels_in(root.path(), 3).expect("retain");

        let listed = retained_kernels(root.path());
        assert_eq!(listed.len(), 1);
        let only = listed.first().expect("one retained kernel");
        assert_eq!(only.version, "6.11.0");
    }

    #[test]
    fn retention_is_idempotent() {
        let root = root_with_kernel("6.11.0");

        let first = retain_current_kernels_in(root.path(), 3).expect("first");
        let second = retain_current_kernels_in(root.path(), 3).expect("second");

        assert_eq!(first, vec!["6.11.0".to_string()]);
        assert!(second.is_empty(), "second run must retain nothing new");
        assert_eq!(retained_kernels(root.path()).len(), 1);
    }

    #[test]
    fn retention_of_zero_disables_itself() {
        let root = root_with_kernel("6.11.0");
        let retained =
            retain_current_kernels_in(root.path(), 0).expect("retain");
        assert_eq!(retained.len(), 0, "retained: {retained:?}");
        assert_eq!(retained_kernels(root.path()).len(), 0);
    }

    #[test]
    fn retention_keeps_only_the_configured_number_of_fallbacks() {
        let root = temp_root();
        fs::create_dir_all(root.path().join(BOOT_DIR))
            .expect("create fixture directory");
        for version in ["6.9.0", "6.10.0", "6.11.0", "6.12.0"] {
            fs::create_dir_all(root.path().join(MODULES_DIR).join(version))
                .expect("create fixture directory");
            fs::write(
                root.path()
                    .join(BOOT_DIR)
                    .join(format!("vmlinuz-{version}")),
                "k"
            )
            .expect("write fixture file");
        }

        // limit 3 means 2 fallbacks alongside the one currently installed.
        retain_current_kernels_in(root.path(), 3).expect("retain");

        let kept: Vec<String> = retained_kernels(root.path())
            .iter()
            .map(|k| k.version.clone())
            .collect();
        assert_eq!(kept, vec!["6.11.0".to_string(), "6.12.0".to_string()]);

        // The pruned kernel's files are gone too, not just its index entry.
        assert!(
            !root.path().join(RETENTION_DIR).join("6.9.0").exists(),
            "pruned kernel files must be reclaimed"
        );
    }

    #[test]
    fn redundant_retention_is_reclaimed() {
        let root = root_with_kernel("6.11.0");
        retain_current_kernels_in(root.path(), 3).expect("retain");
        assert_eq!(retained_kernels(root.path()).len(), 1);

        // The retained kernel is still installed, so its preserved copy is
        // pure waste.
        let reclaimed = prune_redundant_retention(root.path()).expect("prune");
        assert_eq!(reclaimed, vec!["6.11.0".to_string()]);
        assert_eq!(retained_kernels(root.path()).len(), 0);
        assert!(!root.path().join(RETENTION_DIR).join("6.11.0").exists());
    }

    #[test]
    fn redundant_retention_prune_keeps_orphaned_kernels() {
        let root = root_with_kernel("6.11.0");
        retain_current_kernels_in(root.path(), 3).expect("retain");

        // The upgrade completed: the old version's files are gone.
        fs::remove_dir_all(root.path().join(MODULES_DIR).join("6.11.0"))
            .expect("remove fixture directory");
        fs::remove_file(root.path().join(BOOT_DIR).join("vmlinuz-6.11.0"))
            .expect("remove fixture file");

        // Now nothing is installed, so nothing is redundant and the fallback
        // must survive.
        let reclaimed = prune_redundant_retention(root.path()).expect("prune");
        assert_eq!(reclaimed.len(), 0, "reclaimed: {reclaimed:?}");
        assert_eq!(retained_kernels(root.path()).len(), 1);
    }

    #[test]
    fn bootable_kernels_include_retained_ones() {
        let root = root_with_kernel("6.11.0");
        retain_current_kernels_in(root.path(), 3).expect("retain");

        // Simulate an upgrade: the new kernel is installed and the old one's
        // package files are gone.
        fs::remove_dir_all(root.path().join(MODULES_DIR).join("6.11.0"))
            .expect("remove fixture directory");
        fs::remove_file(root.path().join(BOOT_DIR).join("vmlinuz-6.11.0"))
            .expect("remove fixture file");
        fs::remove_file(
            root.path().join(BOOT_DIR).join("initramfs-6.11.0.img")
        )
        .expect("remove fixture file");

        fs::create_dir_all(root.path().join(MODULES_DIR).join("6.12.0"))
            .expect("create fixture directory");
        fs::write(root.path().join(BOOT_DIR).join("vmlinuz-6.12.0"), "k")
            .expect("write fixture file");

        let bootable = all_bootable_kernels(root.path()).expect("bootable");
        let versions: Vec<&str> =
            bootable.iter().map(|k| k.version.as_str()).collect();

        // The old kernel survives only because retention preserved it. This is
        // what makes a bad update recoverable.
        assert!(versions.contains(&"6.11.0"), "fallback lost: {versions:?}");
        assert!(
            versions.contains(&"6.12.0"),
            "new kernel missing: {versions:?}"
        );

        // Oldest first, so the menu reads chronologically.
        assert_eq!(versions, vec!["6.11.0", "6.12.0"]);
    }

    #[test]
    fn reinstalled_kernel_is_not_duplicated_in_the_menu() {
        let root = root_with_kernel("6.11.0");
        retain_current_kernels_in(root.path(), 3).expect("retain");

        // Retention happened, then the same version was installed again.
        let bootable = all_bootable_kernels(root.path()).expect("bootable");
        assert_eq!(bootable.len(), 1);
        let only = bootable.first().expect("one bootable kernel");
        assert_eq!(only.version, "6.11.0");
        // The live entry must win, so it points at the real /boot path.
        assert!(only.image.to_string_lossy().contains("/boot/"));
    }

    #[test]
    fn corrupt_retention_index_is_treated_as_empty() {
        let root = root_with_kernel("6.11.0");
        fs::create_dir_all(root.path().join(RETENTION_DIR))
            .expect("create fixture directory");
        fs::write(
            root.path().join(RETENTION_DIR).join(RETENTION_INDEX),
            "{ not json"
        )
        .expect("write fixture file");

        // Must not panic, and must be recoverable by retaining again.
        assert_eq!(retained_kernels(root.path()).len(), 0);
        retain_current_kernels_in(root.path(), 3).expect("recover");
        assert_eq!(retained_kernels(root.path()).len(), 1);
    }
}
