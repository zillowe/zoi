//! System settings from `system.lua`: hostname, time zone, locales, console
//! and keyboard layout, clock synchronisation, networking and `/etc/fstab`.
//!
//! Everything is written under a root directory, so the same code configures
//! a target being assembled by `zoi system distro build` and the live system
//! during `zoi system apply`. Files are only rewritten when their content
//! changes, and files an administrator may also edit (`/etc/hosts`,
//! `/etc/locale.gen`) only get a block Zoi manages, so running it again is
//! harmless.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;

use crate::config::{KeyboardConfig, SystemConfig};

/// First line of a block Zoi manages inside a shared file.
const BLOCK_BEGIN: &str = "# BEGIN zoi system.lua (managed, do not edit)";
/// Last line of a block Zoi manages inside a shared file.
const BLOCK_END: &str = "# END zoi system.lua";
/// Header of files Zoi writes completely.
const HEADER: &str =
    "# Written by Zoi from system.lua; changes are overwritten.";

/// `/etc/locale.conf` variables other than `LANG` (see locale.conf(5)).
const LOCALE_VARS: &[&str] = &[
    "LANGUAGE",
    "LC_ADDRESS",
    "LC_COLLATE",
    "LC_CTYPE",
    "LC_IDENTIFICATION",
    "LC_MEASUREMENT",
    "LC_MESSAGES",
    "LC_MONETARY",
    "LC_NAME",
    "LC_NUMERIC",
    "LC_PAPER",
    "LC_TELEPHONE",
    "LC_TIME"
];

/// The services `network = "networkd"` needs.
const NETWORKD_SERVICES: &[&str] =
    &["systemd-networkd.service", "systemd-resolved.service"];

/// The service `ntp = true` needs.
const NTP_SERVICE: &str = "systemd-timesyncd.service";

/// What [`apply_settings`] changed, and what the caller still has to do.
#[derive(Debug, Default)]
pub struct SettingsReport {
    /// Files written or changed (root-relative, e.g. `/etc/localtime`).
    pub changed: Vec<String>,
    /// Settings that could not be applied, with the reason.
    pub warnings: Vec<String>,
    /// Services to enable (`true`) or disable (`false`): the ones the
    /// settings imply (networkd, timesyncd), overridden by `services({...})`.
    pub services: BTreeMap<String, bool>
}

impl SettingsReport {
    /// Records that `path` was written.
    fn changed(&mut self, path: &str) {
        self.changed.push(path.to_string());
    }
}

/// Prints what [`apply_settings`] changed and any warnings.
pub fn print_report(report: &SettingsReport) {
    for path in &report.changed {
        println!("{} {}", "Configured".green(), path);
    }
    for warning in &report.warnings {
        println!("{} {}", "Warning:".yellow(), warning);
    }
    if report.changed.is_empty() && report.warnings.is_empty() {
        println!("{} settings already match system.lua", "::".dimmed());
    }
}

/// Applies the `system({...})` settings and `filesystems({...})` to the
/// system under `root` (`/` for the live system).
///
/// Values that are malformed or name something the system does not have (an
/// unknown time zone, locale or keymap) are errors. Settings whose data is
/// not installed at all (no time zone database, no console keymaps) are
/// skipped with a warning naming the package to add.
///
/// # Errors
///
/// Returns an error for an invalid setting, or if a file cannot be written.
pub fn apply_settings(
    root: &Path,
    config: &SystemConfig,
    dry_run: bool
) -> Result<SettingsReport> {
    let system = &config.system;
    let mut report = SettingsReport::default();
    let mut w = Writer {
        root,
        dry_run,
        report: &mut report
    };

    if let Some(hostname) = &system.hostname {
        apply_hostname(&mut w, hostname)?;
    }
    if let Some(timezone) = &system.timezone {
        apply_timezone(&mut w, timezone)?;
    }
    apply_locale(&mut w, config)?;
    apply_console(
        &mut w,
        system.keymap.as_deref(),
        system.console_font.as_deref()
    )?;
    if let Some(keyboard) = &system.keyboard {
        apply_keyboard(&mut w, keyboard)?;
    }
    if !config.filesystems.is_empty() {
        let fstab = crate::mount::generate_fstab(&config.filesystems);
        w.write("/etc/fstab", &fstab)?;
    }

    let mut services = BTreeMap::new();
    if let Some(ntp) = system.ntp {
        services.insert(NTP_SERVICE.to_string(), ntp);
    }
    match system.network.as_deref() {
        None | Some("none") => {}
        Some("networkd") => {
            apply_networkd(&mut w)?;
            for service in NETWORKD_SERVICES {
                services.insert((*service).to_string(), true);
            }
        }
        Some(other) => bail!(
            "Unknown network '{other}' in system.lua: expected \"networkd\" \
             or \"none\""
        )
    }
    for (name, service) in &config.services {
        services.insert(name.clone(), service.enable);
    }
    report.services = services;
    Ok(report)
}

/// Writes files under the root, recording what changed.
struct Writer<'a> {
    /// The system root (`/` for the live system).
    root: &'a Path,
    /// Report changes without writing anything.
    dry_run: bool,
    /// Where changes and warnings are recorded.
    report: &'a mut SettingsReport
}

impl Writer<'_> {
    /// The host path of a root-relative path such as `/etc/hostname`.
    fn path(&self, guest: &str) -> PathBuf {
        self.root.join(guest.trim_start_matches('/'))
    }

    /// Writes `content` to `guest` unless it already has it.
    fn write(&mut self, guest: &str, content: &str) -> Result<()> {
        let path = self.path(guest);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file())
            && fs::read_to_string(&path).is_ok_and(|old| old == content)
        {
            return Ok(());
        }
        if !self.dry_run {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            // A symlink (e.g. a resolv.conf pointing elsewhere) is replaced,
            // never written through.
            if fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink()) {
                fs::remove_file(&path)?;
            }
            fs::write(&path, content)
                .with_context(|| format!("Failed to write {guest}"))?;
        }
        self.report.changed(guest);
        Ok(())
    }

    /// Points the symlink `guest` at `target` unless it already does.
    fn symlink(&mut self, guest: &str, target: &str) -> Result<()> {
        let path = self.path(guest);
        if fs::read_link(&path).is_ok_and(|t| t == Path::new(target)) {
            return Ok(());
        }
        if !self.dry_run {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            if fs::symlink_metadata(&path).is_ok() {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to replace {guest}"))?;
            }
            std::os::unix::fs::symlink(target, &path)
                .with_context(|| format!("Failed to link {guest}"))?;
        }
        self.report.changed(guest);
        Ok(())
    }

    /// Replaces Zoi's block in `guest` (keeping the rest of the file), or
    /// appends it.
    fn managed_block(&mut self, guest: &str, lines: &[String]) -> Result<()> {
        let existing = fs::read_to_string(self.path(guest)).unwrap_or_default();
        let content = with_managed_block(&existing, lines);
        self.write(guest, &content)
    }

    /// Records a setting that could not be applied.
    fn warn(&mut self, message: String) {
        self.report.warnings.push(message);
    }
}

/// `existing` with Zoi's block replaced by (or extended with) `lines`.
#[must_use]
pub fn with_managed_block(existing: &str, lines: &[String]) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in existing.lines() {
        if line == BLOCK_BEGIN {
            skipping = true;
        } else if line == BLOCK_END && skipping {
            skipping = false;
        } else if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !lines.is_empty() {
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(BLOCK_BEGIN);
        out.push('\n');
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(BLOCK_END);
        out.push('\n');
    }
    out
}

/// Resolves `.` and `..` in an absolute guest path without touching the
/// filesystem (so it works for paths inside another root).
#[must_use]
pub fn normalize_guest_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    out
}

/// Fails unless every character of `value` is allowed by `ok`.
fn check_chars(
    what: &str,
    value: &str,
    ok: impl Fn(char) -> bool
) -> Result<()> {
    if value.is_empty() || !value.chars().all(ok) {
        bail!("Invalid {what} '{value}' in system.lua");
    }
    Ok(())
}

/// Writes `/etc/hostname` and the `127.0.1.1` line in `/etc/hosts`.
fn apply_hostname(w: &mut Writer, hostname: &str) -> Result<()> {
    // RFC 1123 host name: dot-separated labels of letters, digits and '-'.
    let valid = hostname.len() <= 64
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !valid {
        bail!("Invalid hostname '{hostname}' in system.lua");
    }
    w.write("/etc/hostname", &format!("{hostname}\n"))?;
    let short = hostname.split('.').next().unwrap_or(hostname);
    let entry = if short == hostname {
        format!("127.0.1.1        {hostname}")
    } else {
        format!("127.0.1.1        {hostname} {short}")
    };
    w.managed_block("/etc/hosts", &[entry])
}

/// Points `/etc/localtime` at the time zone's file in
/// `/usr/share/zoneinfo`.
fn apply_timezone(w: &mut Writer, timezone: &str) -> Result<()> {
    check_chars("time zone", timezone, |c| {
        c.is_ascii_alphanumeric() || "/_+-".contains(c)
    })?;
    if timezone.starts_with('/') || timezone.split('/').any(str::is_empty) {
        bail!("Invalid time zone '{timezone}' in system.lua");
    }
    let zoneinfo = w.path("/usr/share/zoneinfo");
    if !zoneinfo.is_dir() {
        w.warn(format!(
            "time zone '{timezone}' not set: no time zone database in \
             /usr/share/zoneinfo (install @core/tzdata)"
        ));
        return Ok(());
    }
    if !zoneinfo.join(timezone).is_file() {
        bail!(
            "Unknown time zone '{timezone}' in system.lua (see \
             /usr/share/zoneinfo, e.g. \"Europe/Berlin\" or \"UTC\")"
        );
    }
    w.symlink(
        "/etc/localtime",
        &format!("../usr/share/zoneinfo/{timezone}")
    )
}

/// `true` for a locale name such as `en_US.UTF-8` or `sr_RS@latin`.
fn is_locale_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.@-".contains(c)
}

/// Selects the locales in `/etc/locale.gen`, compiles them, and writes
/// `/etc/locale.conf`.
fn apply_locale(w: &mut Writer, config: &SystemConfig) -> Result<()> {
    let system = &config.system;
    let settings = system.locale_settings.clone().unwrap_or_default();
    if system.locale.is_none()
        && settings.is_empty()
        && system.locales.is_none()
    {
        return Ok(());
    }

    for (key, value) in &settings {
        if !LOCALE_VARS.contains(&key.as_str()) {
            bail!(
                "Unknown locale setting '{key}' in system.lua (expected one \
                 of {})",
                LOCALE_VARS.join(", ")
            );
        }
        let ok = |c: char| is_locale_char(c) || (key == "LANGUAGE" && c == ':');
        check_chars(&format!("{key} value"), value, ok)?;
    }

    // Locales to generate: as given, or every locale the settings use.
    let wanted: Vec<String> = match &system.locales {
        Some(list) => list.clone(),
        None => system
            .locale
            .iter()
            .chain(
                settings
                    .iter()
                    .filter(|(key, _)| key.as_str() != "LANGUAGE")
                    .map(|(_, value)| value)
            )
            .cloned()
            .collect()
    };
    let mut entries = Vec::new();
    for name in &wanted {
        check_chars("locale", name, is_locale_char)?;
        let entry = locale_gen_entry(w, name)?;
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }

    if !entries.is_empty() {
        let before = w.report.changed.len();
        w.managed_block("/etc/locale.gen", &entries)?;
        let gen_changed = w.report.changed.len() > before;
        let archive = w.path("/usr/lib/locale/locale-archive");
        if (gen_changed || !archive.exists()) && !w.dry_run {
            run_locale_gen(w);
        }
    }

    let mut conf = String::new();
    if let Some(lang) = &system.locale {
        check_chars("locale", lang, is_locale_char)?;
        let _ = writeln!(conf, "LANG={lang}");
    }
    for (key, value) in &settings {
        let _ = writeln!(conf, "{key}={value}");
    }
    if !conf.is_empty() {
        w.write("/etc/locale.conf", &conf)?;
    }
    Ok(())
}

/// The `/etc/locale.gen` line for `name` ("en_US.UTF-8 UTF-8"), checked
/// against glibc's list of supported locales when the system has it.
fn locale_gen_entry(w: &Writer, name: &str) -> Result<String> {
    let supported = w.path("/usr/share/i18n/SUPPORTED");
    if let Ok(list) = fs::read_to_string(&supported) {
        return list
            .lines()
            .find(|line| line.split_whitespace().next() == Some(name))
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .ok_or_else(|| {
                anyhow!(
                    "Unknown locale '{name}' in system.lua (see \
                     /usr/share/i18n/SUPPORTED, e.g. \"en_US.UTF-8\")"
                )
            });
    }
    // No list to check against: derive the charset from the name, as glibc
    // names its locales ("de_DE.UTF-8" -> UTF-8).
    let charset = name.split_once('.').map_or("ISO-8859-1", |(_, rest)| {
        rest.split('@').next().unwrap_or(rest)
    });
    Ok(format!("{name} {charset}"))
}

/// Compiles the configured locales by running `locale-gen` in the root.
fn run_locale_gen(w: &mut Writer) {
    if !w.path("/usr/bin/locale-gen").exists() {
        w.warn(
            "locales not generated: /usr/bin/locale-gen is missing (install \
             @core/glibc)"
                .to_string()
        );
        return;
    }
    let mut cmd = Command::new("/usr/bin/locale-gen");
    match crate::kernel::run_in_root(w.root, &mut cmd) {
        Ok(true) => w.report.changed("/usr/lib/locale/locale-archive"),
        Ok(false) => w.warn("locale-gen failed".to_string()),
        Err(e) => w.warn(format!("locale-gen could not run: {e}"))
    }
}

/// Writes the console keymap and font to `/etc/vconsole.conf`.
fn apply_console(
    w: &mut Writer,
    keymap: Option<&str>,
    font: Option<&str>
) -> Result<()> {
    if keymap.is_none() && font.is_none() {
        return Ok(());
    }
    let kbd = w.path("/usr/share/kbd");
    let mut conf = String::new();
    if let Some(keymap) = keymap {
        check_chars("keymap", keymap, |c| {
            c.is_ascii_alphanumeric() || "_.-".contains(c)
        })?;
        if !kbd.join("keymaps").is_dir() {
            w.warn(format!(
                "console keymap '{keymap}' not checked: no keymaps in \
                 /usr/share/kbd (install @core/kbd)"
            ));
        } else if !has_file_named(&kbd.join("keymaps"), keymap, ".map") {
            bail!(
                "Unknown console keymap '{keymap}' in system.lua (see \
                 `localectl list-keymaps`)"
            );
        }
        let _ = writeln!(conf, "KEYMAP={keymap}");
    }
    if let Some(font) = font {
        check_chars("console font", font, |c| {
            c.is_ascii_alphanumeric() || "_.-".contains(c)
        })?;
        let fonts = kbd.join("consolefonts");
        if !fonts.is_dir() {
            w.warn(format!(
                "console font '{font}' not checked: no fonts in \
                 /usr/share/kbd/consolefonts (install @core/kbd)"
            ));
        } else if !has_file_named(&fonts, font, ".") {
            bail!(
                "Unknown console font '{font}' in system.lua (see \
                 /usr/share/kbd/consolefonts)"
            );
        }
        let _ = writeln!(conf, "FONT={font}");
    }
    w.write("/etc/vconsole.conf", &conf)
}

/// Whether `dir` (searched recursively) has a file `<name><ext>...`, e.g.
/// `us.map.gz` for keymap `us`.
fn has_file_named(dir: &Path, name: &str, ext: &str) -> bool {
    let prefix = format!("{name}{ext}");
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            has_file_named(&path, name, ext)
        } else {
            entry.file_name().to_string_lossy().starts_with(&prefix)
        }
    })
}

/// Writes the graphical keyboard layout for X11 and Wayland compositors
/// that read it.
fn apply_keyboard(w: &mut Writer, keyboard: &KeyboardConfig) -> Result<()> {
    let options = [
        ("XkbLayout", &keyboard.layout),
        ("XkbVariant", &keyboard.variant),
        ("XkbModel", &keyboard.model),
        ("XkbOptions", &keyboard.options)
    ];
    let mut body = String::new();
    for (option, value) in options {
        if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
            check_chars(&format!("keyboard {option}"), value, |c| {
                c.is_ascii_alphanumeric() || ",:_+-()".contains(c)
            })?;
            let _ = writeln!(body, "        Option \"{option}\" \"{value}\"");
        }
    }
    if body.is_empty() {
        return Ok(());
    }
    let conf = format!(
        "{HEADER}\nSection \"InputClass\"\n        Identifier \
         \"system-keyboard\"\n        MatchIsKeyboard \
         \"on\"\n{body}EndSection\n"
    );
    w.write("/etc/X11/xorg.conf.d/00-keyboard.conf", &conf)
}

/// DHCP on every wired interface, and name resolution through
/// systemd-resolved's stub resolver.
fn apply_networkd(w: &mut Writer) -> Result<()> {
    let network = format!(
        "{HEADER}\n[Match]\nType=ether\n\n[Network]\nDHCP=yes\n\n[DHCPv4]\\
         nRouteMetric=100\n\n[IPv6AcceptRA]\nRouteMetric=100\n"
    );
    w.write("/etc/systemd/network/20-wired.network", &network)?;
    w.symlink("/etc/resolv.conf", "/run/systemd/resolve/stub-resolv.conf")
}
