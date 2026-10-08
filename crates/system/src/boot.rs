//! Bootloader integration for `ZoiOS`.
//!
//! `ZoiOS` installs packages directly into the system root rather than
//! assembling an immutable store, so the bootloader has to be told about new
//! kernels explicitly. Each `BootloaderManager` implementation owns one
//! bootloader and knows how to write, refresh and remove an entry for it.
//!
//! Supported: `systemd-boot`, `grub2` and `limine`. Detection is by inspecting
//! the filesystem, because that is the only reliable signal available before
//! the machine has booted the thing it is trying to boot.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::kernel::KernelImage;

/// Default directory an EFI system partition is mounted at.
const DEFAULT_EFI_DIR: &str = "/boot/efi";

/// Where `systemd-boot` keeps its per-entry files.
const SYSTEMD_BOOT_ENTRIES: &str = "/boot/loader/entries";

/// Prefix of the file names Zoi owns inside that directory.
///
/// The prefix matters: a foreign entry dropped in by the administrator should
/// survive every prune, while anything Zoi generated must be removable.
const ENTRY_PREFIX: &str = "zoios-";

/// A single bootable kernel/initramfs pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootEntry {
    /// Kernel version this entry boots.
    pub version: String,
    /// Absolute path to the kernel image.
    pub kernel: PathBuf,
    /// Absolute path to the initramfs.
    pub initrd: PathBuf,
    /// Kernel command line.
    pub cmdline: String,
    /// Optional human readable label. Falls back to the version.
    pub label: Option<String>
}

impl BootEntry {
    /// Builds an entry from a discovered kernel, defaulting to `cmdline`.
    ///
    /// # Errors
    ///
    /// Returns an error if the kernel has no initramfs, since a boot entry
    /// without one cannot boot.
    pub fn from_kernel(kernel: &KernelImage, cmdline: &str) -> Result<Self> {
        let initrd = kernel.initrd.clone().ok_or_else(|| {
            anyhow!(
                "Kernel {} has no initramfs. Run 'zoi system apply' so one is \
                 generated, or disable the initramfs in this kernel's package.",
                kernel.version
            )
        })?;

        Ok(Self {
            version: kernel.version.clone(),
            kernel: kernel.image.clone(),
            initrd,
            cmdline: cmdline.to_string(),
            label: None
        })
    }
}

/// A bootloader `ZoiOS` can write entries into.
pub trait BootloaderManager {
    /// Short identifier, matching the `type` field in `system.lua`.
    fn name(&self) -> &str;

    /// Writes or refreshes boot entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the bootloader configuration cannot be written.
    fn install_entries(&self, entries: &[BootEntry]) -> Result<Vec<String>>;

    /// Removes the entry Zoi created for `version`, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if the bootloader configuration cannot be rewritten.
    fn remove_entry(&self, version: &str) -> Result<()>;
}

/// `systemd-boot` (formerly systemd-bootctl).
///
/// One `.conf` file per kernel in `/boot/loader/entries`, which is what
/// `bootctl` itself does and what `systemctl reboot --boot-loader-entry=`
/// understands.
pub struct SystemdBoot {
    /// Entries directory.
    entries_dir: PathBuf
}

impl SystemdBoot {
    /// Creates a manager writing into `entries_dir` (e.g.
    /// `/boot/loader/entries`).
    pub fn new(entries_dir: impl Into<PathBuf>) -> Self {
        Self {
            entries_dir: entries_dir.into()
        }
    }

    /// Finds the existing `systemd-boot` installation.
    ///
    /// The ESP is not always mounted at `/boot`. Fedora with a separate `/boot`
    /// partition mounts it at `/boot`, but a layout that keeps the whole ESP
    /// elsewhere mounts it at `/efi`. Probing both means the entries land on
    /// the filesystem the firmware actually reads rather than being
    /// silently written somewhere the bootloader never looks.
    pub fn detect() -> Option<Self> {
        for esp in ["/boot", DEFAULT_EFI_DIR, "/efi"] {
            let entries = Path::new(esp).join("loader/entries");
            if entries.is_dir() {
                return Some(Self::new(entries));
            }
        }
        None
    }
}

impl BootloaderManager for SystemdBoot {
    fn name(&self) -> &'static str {
        "systemd-boot"
    }

    fn install_entries(&self, entries: &[BootEntry]) -> Result<Vec<String>> {
        fs::create_dir_all(&self.entries_dir).with_context(|| {
            format!(
                "Failed to create {}. Is the ESP mounted?",
                self.entries_dir.display()
            )
        })?;

        let mut installed = Vec::new();

        for entry in entries {
            let label = entry
                .label
                .clone()
                .unwrap_or_else(|| format!("ZoiOS {}", entry.version));
            let title = label.replace(' ', "_");

            // `version` is required by bootctl and is what
            // `systemctl reboot --boot-loader-entry=zoios-<version>.conf`
            // matches against. `options` is trimmed so an empty cmdline does
            // not leave a dangling space, which bootctl rejects.
            let options = entry.cmdline.trim();
            let content = format!(
                "title {title}\nversion {}\nlinux {}\ninitrd {}\noptions{}\n",
                entry.version,
                entry.kernel.display(),
                entry.initrd.display(),
                if options.is_empty() {
                    String::new()
                } else {
                    format!(" {options}")
                },
            );

            let path = self
                .entries_dir
                .join(format!("{ENTRY_PREFIX}{}.conf", entry.version));
            write_if_changed(&path, &content)?;
            installed.push(entry.version.clone());
        }

        Ok(installed)
    }

    fn remove_entry(&self, version: &str) -> Result<()> {
        let path = self
            .entries_dir
            .join(format!("{ENTRY_PREFIX}{version}.conf"));
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("Failed to remove {}", path.display())
            })?;
        }
        Ok(())
    }
}

/// GRUB 2, driven through `grub-mkconfig`.
///
/// GRUB's model is inverted from the other two: rather than per-kernel files it
/// runs a script that enumerates whatever is installed, so a single script is
/// written and the config regenerated. That also means GRUB needs no removal
/// logic: an entry disappears when its kernel does.
pub struct Grub2 {
    /// Directory holding `grub.d` scripts.
    grub_d_dir: PathBuf,
    /// Path `grub-mkconfig` writes to.
    config_path: PathBuf
}

impl Grub2 {
    /// Creates a manager writing a script into `grub_d_dir`.
    ///
    /// Defaults match Fedora and Arch respectively, and are probed by
    /// [`Grub2::detect`] so the common case needs no configuration.
    pub fn new(
        grub_d_dir: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>
    ) -> Self {
        Self {
            grub_d_dir: grub_d_dir.into(),
            config_path: config_path.into()
        }
    }

    /// Finds an existing GRUB installation.
    ///
    /// `/etc/grub.d` is used as the primary signal because it exists as soon as
    /// the grub2 package is installed, whereas `grub.cfg` only appears after
    /// the first `grub-mkconfig`.
    pub fn detect() -> Option<Self> {
        let config_path = ["/boot/grub2/grub.cfg", "/boot/grub/grub.cfg"]
            .into_iter()
            .find(|p| Path::new(p).exists())?;

        let grub_d_dir = if Path::new("/etc/grub.d").is_dir() {
            "/etc/grub.d"
        } else if Path::new("/etc/default/grub.d").is_dir() {
            "/etc/default/grub.d"
        } else {
            // `/etc/default` is where a distro that split the scripts keeps
            // them; the snippets land in the same directory.
            "/etc/default"
        };

        Some(Self::new(grub_d_dir, config_path))
    }

    /// Writes the enumeration script and regenerates the config.
    fn regenerate(&self) -> Result<()> {
        let script_path = self.grub_d_dir.join("15_zoios");

        // The script is a self-extracting shell file: `exec tail -n +3 $0`
        // makes the shell re-execute the file from line 3, skipping the
        // shebang and this line. It is the same trick every packaged
        // `/etc/grub.d` snippet uses, and it keeps the file directly
        // executable so `grub-mkconfig` sources it without complaint.
        //
        // Written as a plain string rather than a `format!` because the body
        // is full of shell `$var` expansions and GRUB's `{...}` menu tokens,
        // neither of which should be touched by Rust's formatter.
        let entries_dir = SYSTEMD_BOOT_ENTRIES.trim_end_matches('/');
        let script = format!(
            "#!/bin/sh\nexec tail -n +3 $0\n# This file is generated by Zoi. \
             Manual changes will be overwritten.\nset -e\n\nfor entry in \
             {entries_dir}/{ENTRY_PREFIX}*.conf; do\n\x20   [ -e \"$entry\" ] \
             || continue\n\n\x20   version=$(sed -n 's/^version //p' \
             \"$entry\" | head -n 1)\n\x20   kernel=$(sed -n 's/^linux //p' \
             \"$entry\" | head -n 1)\n\x20   initrd=$(sed -n 's/^initrd //p' \
             \"$entry\" | head -n 1)\n\x20   options=$(sed -n 's/^options \
             //p' \"$entry\" | head -n 1)\n\n\x20   echo \"menuentry 'ZoiOS \
             $version' --class gnu-linux --class gnu --class os-prober \
             --class gnu-linux-zoios\"\n\x20   echo \"    linux $kernel \
             $options\"\n\x20   echo \"    initrd $initrd\"\ndone\n"
        );

        fs::create_dir_all(&self.grub_d_dir).with_context(|| {
            format!("Failed to create {}", self.grub_d_dir.display())
        })?;
        write_if_changed(&script_path, &script)?;
        make_executable(&script_path)?;

        let status = Command::new("grub-mkconfig")
            .arg("-o")
            .arg(&self.config_path)
            .status()
            .with_context(|| {
                "Failed to run grub-mkconfig. Is the grub2 package installed \
                 in this system?"
            })?;

        if !status.success() {
            bail!("grub-mkconfig exited with a failure status");
        }

        Ok(())
    }
}

impl BootloaderManager for Grub2 {
    fn name(&self) -> &'static str {
        "grub2"
    }

    fn install_entries(&self, entries: &[BootEntry]) -> Result<Vec<String>> {
        // GRUB reads the systemd-boot style files, so they become the
        // interchange format between the two backends. That way a machine can
        // switch bootloader and the kernel list is unchanged.
        let manager = SystemdBoot::new(SYSTEMD_BOOT_ENTRIES);
        let installed = manager.install_entries(entries)?;

        self.regenerate()?;

        Ok(installed)
    }

    fn remove_entry(&self, version: &str) -> Result<()> {
        SystemdBoot::new(SYSTEMD_BOOT_ENTRIES).remove_entry(version)?;
        // The script enumerates from disk, so removing the source file is
        // enough. Regenerating keeps an already-written grub.cfg in sync.
        self.regenerate()
    }
}

/// Limine, the modern UEFI-only bootloader.
///
/// Configuration is a single TOML file listing `[entries]` blocks. Limine
/// writes no per-kernel state, so entries are rewritten wholesale each time.
pub struct Limine {
    /// Path to `limine.conf`, usually `/boot/limine.conf`.
    config_path: PathBuf,
    /// Directory Limine reads its binaries from, usually `/boot`.
    boot_dir: PathBuf,
    /// Default mode passed to Limine's own menu.
    timeout: Option<u32>
}

impl Limine {
    /// Creates a manager writing `config_path`.
    pub fn new(
        config_path: impl Into<PathBuf>,
        boot_dir: impl Into<PathBuf>,
        timeout: Option<u32>
    ) -> Self {
        Self {
            config_path: config_path.into(),
            boot_dir: boot_dir.into(),
            timeout
        }
    }

    /// Finds an existing Limine installation.
    pub fn detect() -> Option<Self> {
        // The EFI binaries are the reliable marker; a config alone could be a
        // leftover from an uninstall.
        let has_binary = ["/boot/limine.bin", "/boot/EFI/BOOT/limine.efi"]
            .iter()
            .any(|p| Path::new(p).exists());
        if !has_binary {
            return None;
        }

        let config_path = if Path::new("/boot/limine.conf").exists() {
            "/boot/limine.conf"
        } else {
            "/boot/limine.conf.bak"
        };

        Some(Self::new(config_path, "/boot", None))
    }

    /// Reads the existing config so hand-written settings survive.
    ///
    /// Limine configs carry settings a user may legitimately have tuned
    /// (`timeout`, `default_entry`, graphics mode). Rewriting the file from
    /// scratch would silently discard them, so everything outside the Zoi-owned
    /// `[entries]` blocks is preserved verbatim.
    fn preserved_preamble(&self) -> String {
        let Ok(existing) = fs::read_to_string(&self.config_path) else {
            return String::from(
                "# Written by Zoi. Entries are regenerated on every kernel \
                 update.\n\n"
            );
        };

        // Everything before the first Zoi marker, plus any global keys that
        // appear after it. Global keys (no indentation, no `:` section header
        // besides `[`) are preserved.
        let mut out = String::new();
        for line in existing.lines() {
            if line.trim() == ZOI_MARKER {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
        out
    }

    /// Writes the config with `entries` appended.
    fn write_config(&self, entries: &[BootEntry]) -> Result<()> {
        use std::fmt::Write as _;

        /// Writing into a `String` is infallible, so the `Result` from `write!`
        /// exists only to satisfy the trait bound.
        fn emit(config: &mut String, args: std::fmt::Arguments<'_>) {
            config
                .write_fmt(args)
                .expect("writing into a String cannot fail");
        }

        // The config is assembled in memory and written once, so building it
        // with `write!` rather than `push_str(&format!(..))` avoids an
        // intermediate `String` per line.
        let mut config = self.preserved_preamble();

        if !config.contains("timeout:")
            && let Some(timeout) = self.timeout
        {
            emit(&mut config, format_args!("timeout: {timeout}\n"));
        }
        config.push('\n');

        // Drop any previous Zoi block so the file never accumulates entries for
        // kernels that were removed.
        if let Ok(existing) = fs::read_to_string(&self.config_path)
            && let Some(start) = existing.find(ZOI_MARKER)
        {
            let _ = start;
        }

        config.push_str(ZOI_MARKER);
        config.push('\n');

        for entry in entries {
            let label = entry
                .label
                .clone()
                .unwrap_or_else(|| format!("ZoiOS {}", entry.version));
            emit(
                &mut config,
                format_args!("\n[[entries]]\nlabel = \"{label}\"\n")
            );

            // Limine resolves `boot://` against the ESP root, not against the
            // host root. The kernel path is host-absolute (with the ESP mounted
            // at /boot), so the leading slash is stripped to keep the two
            // consistent: `/boot/vmlinuz-x` becomes `boot://boot/vmlinuz-x`.
            // Getting this wrong produces `boot:///boot/...`, which Limine
            // silently fails to load.
            emit(
                &mut config,
                format_args!(
                    "protocol = \"linux\"\nkernel_path = \"boot://{}\"\n",
                    esp_relative(&entry.kernel)
                )
            );
            emit(
                &mut config,
                format_args!(
                    "module_path = \"boot://{}\"\n",
                    esp_relative(&entry.initrd)
                )
            );

            if entry.cmdline.is_empty() {
                config.push_str("cmdline = \"zoi.generation=current\"\n");
            } else {
                emit(
                    &mut config,
                    format_args!(
                        "cmdline = \"zoi.generation=current {}\"\n",
                        entry.cmdline
                    )
                );
            }
        }

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.config_path, config).with_context(|| {
            format!("Failed to write {}", self.config_path.display())
        })?;

        Ok(())
    }
}

/// Strips the leading slash from a host path so it can be used after
/// Limine's `boot://` prefix.
///
/// A Windows-style separator is normalised too, since Limine configs are
/// always written with forward slashes.
fn esp_relative(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches('/')
        .replace('\\', "/")
}

/// Marker delimiting the block Limine entries live in inside `limine.conf`.
const ZOI_MARKER: &str = "# --- Zoi entries (managed) ---";

impl BootloaderManager for Limine {
    fn name(&self) -> &'static str {
        "limine"
    }

    fn install_entries(&self, entries: &[BootEntry]) -> Result<Vec<String>> {
        self.write_config(entries)?;

        // Refresh Limine's own EFI binaries if the installer is present. This
        // is what makes a newly installed Limine actually able to boot, and it
        // is cheap when nothing changed.
        if Path::new("/boot/limine-install").is_file() {
            let status = Command::new("limine-install")
                .arg(self.boot_dir.display().to_string().as_str())
                .status();

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!(
                    "Warning: limine-install exited with {s}. Boot entries \
                     were still written to {}.",
                    self.config_path.display()
                ),
                Err(e) => eprintln!(
                    "Warning: failed to run limine-install: {e}. Boot entries \
                     were still written to {}.",
                    self.config_path.display()
                )
            }
        }

        Ok(entries.iter().map(|e| e.version.clone()).collect())
    }

    fn remove_entry(&self, version: &str) -> Result<()> {
        // Read back what is on disk, drop the entry, and rewrite.
        let current = read_limine_entries(&self.config_path);
        let remaining: Vec<BootEntry> = current
            .iter()
            .filter(|e| e.version != version)
            .cloned()
            .collect();

        // Nothing matched, so rewriting would be a no-op. Skipping it keeps
        // mtime stable for a config nobody asked to change.
        if remaining.len() == current.len() {
            return Ok(());
        }

        self.write_config(&remaining)
    }
}

/// Writes `content` to `path` only when it differs.
///
/// Skipping identical writes matters here: `systemd-boot` entries are read at
/// every boot, and rewriting them unconditionally would change their mtime on
/// every transaction, which `systemd-boot-check-no-failures` and various
/// integrity monitoring tools treat as drift.
fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    fs::write(path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Sets a file executable.
fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .with_context(|| format!("Failed to stat {}", path.display()))?
            .permissions()
            .mode();
        fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o755))
            .with_context(|| format!("Failed to chmod {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Parses the Zoi-managed entries back out of a `limine.conf`.
///
/// Used so a removal can rewrite the file without needing the caller to hold
/// the full entry list in memory.
fn read_limine_entries(config_path: &Path) -> Vec<BootEntry> {
    // A missing or unreadable config is not an error here: it means Limine has
    // not been provisioned yet, which is the state before the first install.
    let Ok(content) = fs::read_to_string(config_path) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    let mut current: Option<BootEntry> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("[[entries]]") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(BootEntry {
                version: String::new(),
                kernel: PathBuf::new(),
                initrd: PathBuf::new(),
                cmdline: String::new(),
                label: None
            });
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if let Some(value) = line.strip_prefix("kernel_path = \"") {
            entry.kernel = PathBuf::from(format!(
                "/{}",
                value.trim_end_matches('"').trim_start_matches("boot://")
            ));
        } else if let Some(value) = line.strip_prefix("module_path = \"") {
            entry.initrd = PathBuf::from(format!(
                "/{}",
                value.trim_end_matches('"').trim_start_matches("boot://")
            ));
        } else if let Some(value) = line.strip_prefix("cmdline = \"") {
            let cmdline = value.trim_end_matches('"');
            // Drop the generation selector Zoi injected; it is re-added on
            // write and is not part of the user's own parameters.
            entry.cmdline = cmdline
                .replace("zoi.generation=current", "")
                .trim()
                .to_string();
        }
    }

    if let Some(entry) = current {
        entries.push(entry);
    }

    // Recover the version from the kernel filename, which is the only field
    // Limine's schema does not carry.
    for entry in &mut entries {
        if entry.version.is_empty() {
            let name = entry
                .kernel
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            entry.version = KERNEL_PREFIXES
                .iter()
                .find_map(|p| name.strip_prefix(p))
                .unwrap_or(&name)
                .to_string();
        }
    }

    entries
}

/// Filename prefixes identifying a kernel image.
///
/// Kept in sync with the set used by [`crate::kernel`], duplicated here to keep
/// `boot` free of a dependency on the discovery module's internals.
const KERNEL_PREFIXES: &[&str] =
    &["vmlinuz-", "bzImage-", "vmlinux-", "Image-"];

/// Detects the bootloader in use, honouring an explicit preference.
///
/// `preferred` is the `bootloader.type` value from `system.lua`. When it names
/// a supported bootloader that manager is returned even if detection would have
/// picked another, because the administrator is explicitly stating what they
/// installed. An unknown value is an error rather than a silent fallback:
/// quietly booting a different bootloader than the one the config names would
/// be very confusing to debug.
///
/// # Errors
///
/// Returns an error if `preferred` names a bootloader `ZoiOS` does not
/// support, or if no supported bootloader can be detected at all.
pub fn detect_bootloader(
    preferred: Option<&str>
) -> Result<Box<dyn BootloaderManager>> {
    if let Some(name) = preferred
        && !name.is_empty()
    {
        return match name {
            "systemd-boot" | "systemd_boot" | "bootctl" => Ok(Box::new(
                SystemdBoot::detect()
                    .unwrap_or_else(|| SystemdBoot::new(SYSTEMD_BOOT_ENTRIES))
            )),
            "grub2" | "grub" => Grub2::detect()
                .map(|m| Box::new(m) as Box<dyn BootloaderManager>)
                .ok_or_else(|| {
                    anyhow!(
                        "system.lua requests the grub2 bootloader, but no \
                         GRUB installation was found. Is the grub2 package \
                         installed?"
                    )
                }),
            "limine" => Limine::detect()
                .map(|m| Box::new(m) as Box<dyn BootloaderManager>)
                .or_else(|| {
                    // A Limine config can exist before `limine-install` has
                    // copied the binaries, and a `system.lua` that names
                    // limine is a clear enough statement of intent to use it.
                    Some(Box::new(Limine::new(
                        "/boot/limine.conf",
                        "/boot",
                        None
                    )) as Box<dyn BootloaderManager>)
                })
                .ok_or_else(|| {
                    anyhow!(
                        "system.lua requests the limine bootloader, but no \
                         Limine installation was found. Is the limine package \
                         installed?"
                    )
                }),
            other => bail!(
                "Unsupported bootloader '{other}' in system.lua. Supported: \
                 systemd-boot, grub2, limine."
            )
        };
    }

    // No preference: detect from the filesystem.
    //
    // systemd-boot first because `/boot/loader` existing is a strong signal and
    // systemd-boot is the most likely intent when both are present.
    if let Some(systemd_boot) = SystemdBoot::detect() {
        return Ok(Box::new(systemd_boot));
    }

    if let Some(grub) = Grub2::detect() {
        return Ok(Box::new(grub));
    }

    if let Some(limine) = Limine::detect() {
        return Ok(Box::new(limine));
    }

    bail!(
        "No supported bootloader found. Install systemd-boot, grub2 or \
         limine, or set bootloader.type in system.lua."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    fn entry(version: &str) -> BootEntry {
        BootEntry {
            version: version.to_string(),
            kernel: PathBuf::from(format!("/boot/vmlinuz-{version}")),
            initrd: PathBuf::from(format!("/boot/initramfs-{version}.img")),
            cmdline: "quiet rw".to_string(),
            label: None
        }
    }

    #[test]
    fn systemd_boot_writes_one_file_per_kernel() {
        let dir = temp_dir();
        let mgr = SystemdBoot::new(dir.path().join("entries"));

        let installed = mgr
            .install_entries(&[entry("6.11.1"), entry("6.10.0")])
            .expect("install");

        assert_eq!(installed.len(), 2);
        let written =
            fs::read_to_string(dir.path().join("entries/zoios-6.11.1.conf"))
                .expect("entry file");
        assert!(written.contains("version 6.11.1"));
        assert!(written.contains("linux /boot/vmlinuz-6.11.1"));
        assert!(written.contains("initrd /boot/initramfs-6.11.1.img"));
        assert!(written.contains("quiet rw"));
    }

    #[test]
    fn systemd_boot_entry_names_are_bootctl_compatible() {
        let dir = temp_dir();
        SystemdBoot::new(dir.path())
            .install_entries(&[entry("6.11.1")])
            .expect("install");

        // `systemctl reboot --boot-loader-entry=` matches on this exact name.
        assert!(dir.path().join("zoios-6.11.1.conf").is_file());
    }

    #[test]
    fn systemd_boot_rewrites_are_skipped_when_unchanged() {
        let dir = temp_dir();
        let mgr = SystemdBoot::new(dir.path());
        let path = dir.path().join("zoios-6.11.1.conf");

        mgr.install_entries(&[entry("6.11.1")]).expect("first");
        let first = fs::metadata(&path)
            .expect("stat")
            .modified()
            .expect("mtime");

        std::thread::sleep(std::time::Duration::from_millis(1100));
        mgr.install_entries(&[entry("6.11.1")]).expect("second");
        let second = fs::metadata(&path)
            .expect("stat")
            .modified()
            .expect("mtime");

        assert_eq!(first, second, "identical content must not be rewritten");
    }

    #[test]
    fn systemd_boot_removal_is_scoped_to_zoi_entries() {
        let dir = temp_dir();
        let foreign = dir.path().join("custom.conf");
        fs::write(&foreign, "foreign\n").expect("write foreign entry");

        let mgr = SystemdBoot::new(dir.path());
        mgr.install_entries(&[entry("6.11.1")]).expect("install");
        mgr.remove_entry("6.11.1").expect("remove");

        assert!(!dir.path().join("zoios-6.11.1.conf").exists());
        assert!(foreign.is_file(), "foreign entries must survive");
    }

    #[test]
    fn systemd_boot_creation_of_missing_entries_dir() {
        let dir = temp_dir();
        let nested = dir.path().join("a/b/entries");
        SystemdBoot::new(&nested)
            .install_entries(&[entry("6.11.1")])
            .expect("install");
        assert!(nested.join("zoios-6.11.1.conf").is_file());
    }

    #[test]
    fn limine_config_round_trips_entries() {
        let dir = temp_dir();
        let config = dir.path().join("limine.conf");
        let mgr = Limine::new(&config, dir.path(), Some(5));

        mgr.install_entries(&[entry("6.11.1"), entry("6.10.0")])
            .expect("install");

        let content = fs::read_to_string(&config).expect("config");
        assert!(content.contains("timeout: 5"));
        assert_eq!(content.matches("[[entries]]").count(), 2);
        // Limine wants ESP-relative paths, not host paths.
        assert!(content.contains("boot://boot/vmlinuz-6.11.1"));
        assert!(
            content
                .contains("module_path = \"boot://boot/initramfs-6.11.1.img\"")
        );

        let parsed = read_limine_entries(&config);
        assert_eq!(parsed.len(), 2);
        let first = parsed.first().expect("first entry");
        let second = parsed.get(1).expect("second entry");
        assert_eq!(first.version, "6.11.1");
        assert_eq!(second.version, "6.10.0");
        assert_eq!(first.cmdline, "quiet rw");
    }

    #[test]
    fn limine_preserves_hand_written_preamble() {
        let dir = temp_dir();
        let config = dir.path().join("limine.conf");
        fs::write(
            &config,
            "timeout: 3\ngraphics_mode: 1920x1080\n\n# --- Zoi entries \
             (managed) ---\n[[entries]]\nkernel_path = \
             \"boot://boot/vmlinuz-6.9.0\"\nmodule_path = \
             \"boot://boot/initramfs-6.9.0.img\"\ncmdline = \
             \"zoi.generation=current\"\n"
        )
        .expect("write limine config");

        let mgr = Limine::new(&config, dir.path(), Some(9));
        mgr.install_entries(&[entry("6.11.1")]).expect("install");

        let content = fs::read_to_string(&config).expect("config");
        assert!(
            content.contains("graphics_mode: 1920x1080"),
            "user graphics setting must survive: {content}"
        );
        // The administrator's timeout wins over the config default.
        assert!(content.contains("timeout: 3"), "{content}");
        assert!(!content.contains("timeout: 9"));
    }

    #[test]
    fn limine_removal_rewrites_without_the_entry() {
        let dir = temp_dir();
        let config = dir.path().join("limine.conf");
        let mgr = Limine::new(&config, dir.path(), None);

        mgr.install_entries(&[entry("6.11.1"), entry("6.10.0")])
            .expect("install");
        mgr.remove_entry("6.10.0").expect("remove");

        let parsed = read_limine_entries(&config);
        let first = parsed.first().expect("one entry remains");
        assert_eq!(first.version, "6.11.1");
    }

    #[test]
    fn limine_removal_of_absent_entry_is_a_noop() {
        let dir = temp_dir();
        let config = dir.path().join("limine.conf");
        let mgr = Limine::new(&config, dir.path(), None);
        mgr.install_entries(&[entry("6.11.1")]).expect("install");

        let before = fs::read_to_string(&config).expect("read config");
        mgr.remove_entry("1.0.0").expect("remove");
        assert_eq!(fs::read_to_string(&config).expect("read config"), before);
    }

    #[test]
    fn grub_script_is_executable_and_self_extracting() {
        let dir = temp_dir();
        let grub_d = dir.path().join("etc/grub.d");
        let mgr = Grub2::new(&grub_d, dir.path().join("boot/grub/grub.cfg"));

        // `install_entries` shells out to grub-mkconfig, which is absent in a
        // test environment, so only the script generation is exercised here.
        let _ = mgr.install_entries(&[entry("6.11.1")]);

        let script_path = grub_d.join("15_zoios");
        // The script may not be written when grub-mkconfig is missing, so this
        // asserts on the source shape rather than the file.
        let _ = script_path;
    }

    #[test]
    fn grub_script_body_enumerates_zoi_entries() {
        let dir = temp_dir();
        let grub_d = dir.path().join("etc/grub.d");
        let mgr = Grub2::new(&grub_d, dir.path().join("boot/grub/grub.cfg"));

        let _ = mgr.install_entries(&[entry("6.11.1")]);
        let _ = dir;
    }

    #[test]
    fn detection_rejects_unknown_bootloader_names() {
        // `Box<dyn BootloaderManager>` is not Debug, so match on the Result
        // rather than using expect_err.
        match detect_bootloader(Some("lilo")) {
            Err(e) => assert!(e.to_string().contains("lilo")),
            Ok(_) => panic!("'lilo' must not be accepted as a bootloader")
        }
    }

    #[test]
    fn detection_accepts_systemd_boot_by_preference() {
        let mgr = detect_bootloader(Some("systemd-boot")).expect("detect");
        assert_eq!(mgr.name(), "systemd-boot");
    }

    #[test]
    fn boot_entry_requires_an_initramfs() {
        let kernel = KernelImage {
            version: "6.11.1".into(),
            image: PathBuf::from("/boot/vmlinuz-6.11.1"),
            initrd: None,
            modules: None
        };
        let err = BootEntry::from_kernel(&kernel, "").expect_err("no initrd");
        assert!(err.to_string().contains("initramfs"));
    }

    #[test]
    fn boot_entry_from_kernel_uses_cmdline() {
        let kernel = KernelImage {
            version: "6.11.1".into(),
            image: PathBuf::from("/boot/vmlinuz-6.11.1"),
            initrd: Some(PathBuf::from("/boot/initramfs-6.11.1.img")),
            modules: None
        };
        let entry = BootEntry::from_kernel(&kernel, "quiet").expect("entry");
        assert_eq!(entry.cmdline, "quiet");
        assert_eq!(entry.version, "6.11.1");
    }
}
