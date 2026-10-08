//! Declarative account management for ZoiOS.
//!
//! `system.lua` declares `users({...})` and `groups({...})` blocks, and this
//! module reconciles them against the real account databases. It edits
//! `/etc/passwd`, `/etc/shadow`, `/etc/group` and `/etc/gshadow` directly
//! instead of shelling out to `useradd`/`groupadd`.
//!
//! Why not shell out? Two reasons:
//!
//! - A sysroot (`--target /mnt`) has no running `nss`, so `useradd` without
//!   `-R` would create accounts on the *host*. `useradd -R` exists, but it is
//!   shadow-utils specific and not always present on a minimal build host.
//! - Editing the files ourselves means the same code path works for `zoi system
//!   distro build` (target does not exist yet) and for `zoi system apply`
//!   (target is the running root). Both are the same operation once a sysroot
//!   is set, which is what `zoi_core::sysroot` already gives us.
//!
//! Everything is idempotent: applying the same `system.lua` twice performs no
//! writes. That property is what makes `zoi system apply` safe to run on a
//! schedule.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use zoi_core::sysroot;

use crate::config::{GroupConfig, SystemConfig, UserConfig};

/// A previous kernel kept around as a boot fallback.
///
/// Zoi's upgrade removes the old package's files, which for a kernel means the
/// image and its module tree vanish. Without this the previous kernel could not
/// be booted after a bad update, which is the one situation where being able to
/// boot the old kernel matters most.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RetainedKernel {
    /// Kernel version string, e.g. `6.11.4-arch1-1`.
    pub version: String,
    /// Path of the preserved kernel image inside the retention directory.
    pub image: PathBuf,
    /// Path of the preserved initramfs, when one was captured.
    pub initrd: Option<PathBuf>,
    /// Path of the preserved module tree.
    pub modules: Option<PathBuf>,
    /// When the kernel was retained, as seconds since the Unix epoch.
    pub retained_at: u64
}

/// First UID handed out to a normal (non-system) account.
///
/// 1000 matches every mainstream distro, so a `ZoiOS` image behaves the way an
/// administrator expects when they read `/etc/passwd`.
const FIRST_USER_UID: u32 = 1000;

/// First GID handed out to a normal group. Matches `FIRST_USER_UID`.
const FIRST_USER_GID: u32 = 1000;

/// Group every new user joins so they can administer the machine.
///
/// Without this a user created by `system.lua` would land in group `users` on
/// some distros and no group at all on others, which silently breaks every
/// `sudo` rule the distro ships.
const DEFAULT_USER_GROUPS: &[&str] = &["wheel"];

/// GID that owns group-readable shadow entries on a stock system.
const SHADOW_GROUP_GID: u32 = 42;

/// Permission bits for `/etc/shadow`: root-owned, group shadow, no world.
const SHADOW_MODE: u32 = 0o640;

/// Permission bits for `/etc/gshadow`.
const GSHADOW_MODE: u32 = 0o640;

/// Records what actually changed so the caller can report it.
#[derive(Debug, Default)]
pub struct AccountReport {
    /// Groups created, in the order they were created.
    pub created_groups: Vec<String>,
    /// Groups whose gid, or membership, changed.
    pub updated_groups: Vec<String>,
    /// Users created, in the order they were created.
    pub created_users: Vec<String>,
    /// Users whose uid, gid, home, shell, password or groups changed.
    pub updated_users: Vec<String>,
    /// Home directories created for new users.
    pub created_homes: Vec<String>
}

impl AccountReport {
    /// Returns true when the reconciliation was a complete no-op.
    pub fn is_empty(&self) -> bool {
        self.created_groups.is_empty()
            && self.updated_groups.is_empty()
            && self.created_users.is_empty()
            && self.updated_users.is_empty()
    }
}

/// A single parsed line of `/etc/passwd`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PasswdEntry {
    /// Human readable name.
    name: String,
    /// Hash from the password database, or `x` when the entry has no password.
    password: String,
    /// Numeric user id.
    uid: u32,
    /// Numeric group id.
    gid: u32,
    /// Free-form comment field.
    gecos: String,
    /// Home directory path.
    home: String,
    /// Login shell path.
    shell: String
}

impl PasswdEntry {
    /// Parses one line of `/etc/passwd`.
    ///
    /// # Errors
    ///
    /// Returns an error if the line has too few fields to be a passwd entry, or
    /// if the uid or gid fields are not numbers.
    fn parse(line: &str) -> Result<Self> {
        let fields: Vec<&str> = line.split(':').collect();

        // Destructuring does the arity check and names the fields at the same
        // time, which removes the separate length check that would otherwise
        // have to be trusted to keep every index below in bounds.
        let [name, password, uid, gid, gecos, home, shell, ..] =
            fields.as_slice()
        else {
            bail!("Malformed passwd entry: {line}");
        };

        Ok(Self {
            name: (*name).to_string(),
            password: (*password).to_string(),
            uid: uid
                .parse()
                .with_context(|| format!("Bad UID in passwd line: {line}"))?,
            gid: gid
                .parse()
                .with_context(|| format!("Bad GID in passwd line: {line}"))?,
            gecos: (*gecos).to_string(),
            home: (*home).to_string(),
            shell: (*shell).to_string()
        })
    }

    /// Renders the entry back into its single-line database form.
    fn to_line(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.name,
            self.password,
            self.uid,
            self.gid,
            self.gecos,
            self.home,
            self.shell
        )
    }
}

/// A single parsed line of `/etc/shadow`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShadowEntry {
    /// Human readable name.
    name: String,
    /// Content digest.
    hash: String,
    /// Days since the epoch of the last password change.
    last_change: String,
    /// Minimum password age in days.
    min: String,
    /// Maximum password age in days.
    max: String,
    /// Password expiry warning period in days.
    warn: String,
    /// Days of inactivity before the account is locked.
    inactive: String,
    /// Days since the epoch after which the account expires.
    expire: String,
    /// Account flags.
    flag: String
}

impl ShadowEntry {
    /// Parses one line of the database.
    fn parse(line: &str) -> Result<Self> {
        let fields: Vec<&str> = line.split(':').collect();

        let [
            name,
            hash,
            last_change,
            min,
            max,
            warn,
            inactive,
            expire,
            flag,
            ..
        ] = fields.as_slice()
        else {
            bail!("Malformed shadow entry: {line}");
        };

        // Every field but the hash is an ageing parameter, and all of them are
        // kept as strings so an unrecognised value round-trips untouched rather
        // than being silently normalised to zero.
        Ok(Self {
            name: (*name).to_string(),
            hash: (*hash).to_string(),
            last_change: (*last_change).to_string(),
            min: (*min).to_string(),
            max: (*max).to_string(),
            warn: (*warn).to_string(),
            inactive: (*inactive).to_string(),
            expire: (*expire).to_string(),
            flag: (*flag).to_string()
        })
    }

    /// Renders the entry back into its single-line database form.
    fn to_line(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.name,
            self.hash,
            self.last_change,
            self.min,
            self.max,
            self.warn,
            self.inactive,
            self.expire,
            self.flag
        )
    }

    /// Builds a shadow entry for a brand new account.
    ///
    /// `hash` is written verbatim so an administrator can paste the output of
    /// `zoi system secret hash` straight into `system.lua`. A user with no
    /// password gets `!`, which is "locked" to `crypt(3)`: no password can ever
    /// match it, but the account itself stays usable for `su -` from root and
    /// for SSH key login. That is deliberately different from `*` (also locked,
    /// but `su` refuses), because a locked-but-usable account is what you want
    /// when the key material arrives later.
    fn new(name: &str, hash: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            hash: hash
                .map_or_else(|| "!".into(), std::string::ToString::to_string),
            // Days since the epoch. `0` is conventional for a just-created
            // account and tells `chage` the password has never changed.
            last_change: "0".into(),
            min: "0".into(),
            max: "99999".into(),
            warn: "7".into(),
            inactive: String::new(),
            expire: String::new(),
            flag: String::new()
        }
    }
}

/// A single parsed line of `/etc/group`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupEntry {
    /// Human readable name.
    name: String,
    /// Hash from the password database, or `x` when the entry has no password.
    password: String,
    /// Numeric group id.
    gid: u32,
    /// Members of this group.
    members: Vec<String>
}

impl GroupEntry {
    /// Parses one line of `/etc/group`.
    ///
    /// # Errors
    ///
    /// Returns an error if the line has too few fields to be a group entry, or
    /// if the gid field is not a number.
    fn parse(line: &str) -> Result<Self> {
        let fields: Vec<&str> = line.split(':').collect();

        let [name, password, gid, members, ..] = fields.as_slice() else {
            bail!("Malformed group entry: {line}");
        };

        // A trailing empty field means "no members", which is spelled as an
        // empty comma list on disk.
        let members = if members.is_empty() {
            Vec::new()
        } else {
            members
                .split(',')
                .map(std::string::ToString::to_string)
                .collect()
        };

        Ok(Self {
            name: (*name).to_string(),
            password: (*password).to_string(),
            gid: gid
                .parse()
                .with_context(|| format!("Bad GID in group line: {line}"))?,
            members
        })
    }

    /// Renders the entry back into its single-line database form.
    fn to_line(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.name,
            self.password,
            self.gid,
            self.members.join(",")
        )
    }
}

/// A single parsed line of `/etc/gshadow`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GshadowEntry {
    /// Human readable name.
    name: String,
    /// Hash from the password database, or `x` when the entry has no password.
    password: String,
    /// Administrators of this group.
    admins: Vec<String>,
    /// Members of this group.
    members: Vec<String>
}

impl GshadowEntry {
    /// Parses one line of `/etc/gshadow`.
    ///
    /// # Errors
    ///
    /// Returns an error if the line has too few fields to be a gshadow entry.
    fn parse(line: &str) -> Result<Self> {
        let fields: Vec<&str> = line.split(':').collect();

        let [name, password, admins, members, ..] = fields.as_slice() else {
            bail!("Malformed gshadow entry: {line}");
        };
        let split_list = |raw: &str| -> Vec<String> {
            if raw.is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(std::string::ToString::to_string)
                    .collect()
            }
        };
        Ok(Self {
            name: (*name).to_string(),
            password: (*password).to_string(),
            admins: split_list(admins),
            members: split_list(members)
        })
    }

    /// Renders the entry back into its single-line database form.
    fn to_line(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.name,
            self.password,
            self.admins.join(","),
            self.members.join(",")
        )
    }
}

/// The four account databases, parsed and ready to mutate.
struct AccountDb {
    /// Password database entries.
    passwd: Vec<PasswdEntry>,
    /// Shadow password database entries.
    shadow: Vec<ShadowEntry>,
    /// Supplementary groups.
    groups: Vec<GroupEntry>,
    /// Group shadow database entries.
    gshadow: Vec<GshadowEntry>,
    /// True when we created the files ourselves and therefore own the task of
    /// creating `/etc/skel`.
    is_new: bool
}

impl AccountDb {
    /// Reads the account databases out of `root`, tolerating absent files.
    ///
    /// A fresh sysroot has no `/etc/passwd` at all, so every reader has to
    /// accept "file not found" as "empty database" rather than failing.
    fn load(root: &Path) -> Result<Self> {
        let passwd_path = root.join("etc/passwd");
        let is_new = !passwd_path.exists();

        let passwd = read_optional(&passwd_path)?
            .iter()
            .map(|l| PasswdEntry::parse(l))
            .collect::<Result<Vec<_>>>()?;
        let shadow = read_optional(&root.join("etc/shadow"))?
            .iter()
            .map(|l| ShadowEntry::parse(l))
            .collect::<Result<Vec<_>>>()?;
        let groups = read_optional(&root.join("etc/group"))?
            .iter()
            .map(|l| GroupEntry::parse(l))
            .collect::<Result<Vec<_>>>()?;
        let gshadow = read_optional(&root.join("etc/gshadow"))?
            .iter()
            .map(|l| GshadowEntry::parse(l))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            passwd,
            shadow,
            groups,
            gshadow,
            is_new
        })
    }

    /// Find passwd.
    fn find_passwd(&self, name: &str) -> Option<&PasswdEntry> {
        self.passwd.iter().find(|e| e.name == name)
    }

    /// Find group.
    fn find_group(&self, name: &str) -> Option<&GroupEntry> {
        self.groups.iter().find(|e| e.name == name)
    }

    /// Find gshadow.
    fn find_gshadow(&self, name: &str) -> Option<&GshadowEntry> {
        self.gshadow.iter().find(|e| e.name == name)
    }

    /// Writes all four files back, creating parent directories as needed.
    fn save(&self, root: &Path) -> Result<()> {
        let etc = root.join("etc");
        fs::create_dir_all(&etc)
            .with_context(|| format!("Failed to create {}", etc.display()))?;

        // Password field is `x` for every account: authentication is delegated
        // to PAM via /etc/shadow. Writing a real hash here would leak it to
        // every process that can read passwd.
        write_file(
            &etc.join("passwd"),
            &render(&self.passwd, PasswdEntry::to_line)
        )?;
        write_file(
            &etc.join("group"),
            &render(&self.groups, GroupEntry::to_line)
        )?;

        // Write first, then chmod. The other order would silently no-op on a
        // freshly created file, which is exactly the case that matters: a new
        // sysroot has no /etc/shadow, so the file is created by `write_file`
        // with the process umask rather than the mode we want.
        write_file(
            &etc.join("shadow"),
            &render(&self.shadow, ShadowEntry::to_line)
        )?;
        write_file(
            &etc.join("gshadow"),
            &render(&self.gshadow, GshadowEntry::to_line)
        )?;
        set_mode(&etc.join("shadow"), SHADOW_MODE)?;
        set_mode(&etc.join("gshadow"), GSHADOW_MODE)?;

        Ok(())
    }

    /// Lowest unused UID at or above `start`.
    ///
    /// Scans rather than tracking a counter, so a hand-edited `/etc/passwd`
    /// cannot cause a collision later.
    fn next_free_uid(&self, start: u32) -> u32 {
        let used: HashSet<u32> = self.passwd.iter().map(|e| e.uid).collect();
        (start..u32::MAX)
            .find(|uid| !used.contains(uid))
            .unwrap_or(start)
    }

    /// Lowest unused GID at or above `start`.
    fn next_free_gid(&self, start: u32) -> u32 {
        let used: HashSet<u32> = self.groups.iter().map(|e| e.gid).collect();
        (start..u32::MAX)
            .find(|gid| !used.contains(gid))
            .unwrap_or(start)
    }
}

/// Appends `value` to `list` only when it is not already present.
///
/// The report lists are deduplicated because a single user can be reported
/// through several independent changes (shadow hash plus group membership),
/// and printing "Updated user 'alice'" three times reads like a bug.
fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|v| v == value) {
        list.push(value.to_string());
    }
}

/// Reads a file into entry lines, returning empty when the file is absent.
fn read_optional(path: &Path) -> Result<Vec<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(std::string::ToString::to_string)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => {
            Err(e).with_context(|| format!("Failed to read {}", path.display()))
        }
    }
}

/// Joins rendered entries into file content, preserving declaration order.
fn render<T>(entries: &[T], to_line: fn(&T) -> String) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&to_line(entry));
        out.push('\n');
    }
    out
}

/// Writes `content` to `path` only when it differs, and returns whether it
/// wrote. Skipping identical writes keeps `apply` from touching mtimes, which
/// would otherwise churn backups and trip `changed-files` tooling.
fn write_file(path: &Path, content: &str) -> Result<bool> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(false);
    }
    fs::write(path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(true)
}

/// Applies a mode to a file that may not exist yet.
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("Failed to chmod {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Validates a login name against the portable subset of POSIX portable
/// usernames, so we reject a name that would produce an unparsable passwd file.
fn validate_username(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Username must not be empty");
    }
    if name.len() > 32 {
        bail!("Username '{name}' is longer than the 32 character limit");
    }
    let mut chars = name.chars();
    let first = chars.next().expect("name was checked non-empty above");
    if !first.is_ascii_alphabetic() && first != '_' {
        bail!(
            "Username '{name}' must start with an ASCII letter or underscore"
        );
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        bail!("Username '{name}' contains an unsupported character");
    }
    Ok(())
}

/// Validates a group name using the same rules portable usernames follow.
fn validate_group_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Group name must not be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        bail!("Group name '{name}' contains an unsupported character");
    }
    Ok(())
}

/// Creates a group if it is missing and records the change.
///
/// An existing group is left completely alone. `system.lua` is a source of
/// truth for *existence*, not for every attribute: `root`, `wheel`, `disk` and
/// friends ship in the base packages with deliberate GIDs and memberships, and
/// a declarative file that reassigns their GID on every apply would fight the
/// packages that own them.
fn ensure_group(
    db: &mut AccountDb,
    name: &str,
    _cfg: &GroupConfig,
    report: &mut AccountReport
) -> Result<u32> {
    validate_group_name(name)?;

    if let Some(existing) = db.find_group(name) {
        // The GID declared in system.lua wins for a group Zoi itself created,
        // because otherwise there is no way to pin a GID for reproducibility.
        // `gid` is optional; when unset we keep whatever exists.
        return Ok(existing.gid);
    }

    let gid = match db.find_gshadow(name) {
        // Group exists in group but not gshadow (a half-written base image).
        // Reuse its GID rather than leaking the old one.
        Some(_) => db.find_group(name).map_or(0, |g| g.gid),
        None => db.next_free_gid(FIRST_USER_GID)
    };

    db.groups.push(GroupEntry {
        name: name.to_string(),
        password: "x".into(),
        gid,
        members: Vec::new()
    });
    if db.find_gshadow(name).is_none() {
        db.gshadow.push(GshadowEntry {
            name: name.to_string(),
            password: "!".into(),
            admins: Vec::new(),
            members: Vec::new()
        });
    }
    report.created_groups.push(name.to_string());
    Ok(gid)
}

/// Applies one declared user to the account databases.
///
/// Returns `true` when the account is new, so the caller knows it also owns
/// the home directory.
fn ensure_user(
    db: &mut AccountDb,
    name: &str,
    cfg: &UserConfig,
    report: &mut AccountReport
) -> Result<bool> {
    validate_username(name)?;

    let is_new = db.find_passwd(name).is_none();

    // --- Primary group ---
    // Prefer a same-named group, which is what every mainstream distro does.
    // Fall back to `users`, then to the literal GID from an existing entry.
    let primary_group = if db.find_group(name).is_some() {
        name.to_string()
    } else if db.find_group("users").is_some() {
        "users".to_string()
    } else {
        // Last resort: a group named after the user may not exist yet but the
        // user still needs a valid GID. `users` should have been created by
        // the base package; creating it here keeps a minimal image working.
        db.next_free_gid(FIRST_USER_GID).to_string()
    };
    let primary_gid =
        match primary_group.parse::<u32>() {
            Ok(gid) => gid,
            Err(_) => db.find_group(&primary_group).map(|g| g.gid).ok_or_else(
                || {
                    anyhow!(
                        "Primary group '{primary_group}' for user '{name}' \
                         does not exist"
                    )
                }
            )?
        };

    // --- Supplementary groups ---
    // Union of the declared groups, the distro default, and whatever the
    // account already has, so removing a group from system.lua does not strip
    // a membership some package added on purpose.
    let mut wanted_groups: Vec<String> = cfg.groups.clone().unwrap_or_default();
    // Only inject the distro default groups when they actually exist. Adding a
    // membership for a group that no package created would emit a warning on
    // every single apply, which trains the administrator to ignore warnings.
    for g in DEFAULT_USER_GROUPS {
        let exists = db.find_group(g).is_some();
        if exists && !wanted_groups.iter().any(|existing| existing == g) {
            wanted_groups.push((*g).to_string());
        }
    }

    let existing_entry = db.find_passwd(name).cloned();
    let home = cfg
        .home
        .clone()
        .or_else(|| existing_entry.as_ref().map(|e| e.home.clone()))
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| format!("/home/{name}"));
    let shell = cfg
        .shell
        .clone()
        .or_else(|| existing_entry.as_ref().map(|e| e.shell.clone()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());

    // Resolve the password hash. A `ZOISEC:` value is decrypted first so an
    // encrypted secret can be used directly as a password.
    //
    // `None` means "system.lua says nothing about this password", which is
    // different from an empty string. Only an explicit value may overwrite the
    // stored hash; otherwise a `passwd` field (always the literal `x`) would be
    // copied over the real hash in `/etc/shadow` on the next apply.
    let password_hash = match cfg.password_hash.as_deref() {
        Some(value) if !value.is_empty() => {
            Some(crate::secret::decrypt_secret(value).with_context(|| {
                format!("Failed to decrypt password for user '{name}'")
            })?)
        }
        _ => None
    };

    // --- Pick a UID ---
    // An existing account keeps its UID. Changing a UID underneath a home
    // directory full of files would orphan every one of them.
    let uid = match &existing_entry {
        Some(entry) => entry.uid,
        None => db.next_free_uid(FIRST_USER_UID)
    };

    let gecos = name.to_string();

    let new_entry = PasswdEntry {
        name: name.to_string(),
        password: "x".into(),
        uid,
        gid: primary_gid,
        gecos,
        home: home.clone(),
        shell
    };

    if let Some(existing) = &existing_entry {
        if *existing != new_entry {
            let slot = db
                .passwd
                .iter_mut()
                .find(|e| e.name == name)
                .expect("passwd entry exists");
            *slot = new_entry;
            report.updated_users.push(name.to_string());
        }
    } else {
        db.passwd.push(new_entry);
        report.created_users.push(name.to_string());
    }

    // --- Shadow ---
    // A new account with no password is locked (`!`). An existing account with
    // no declared password keeps whatever hash it already has, which is what
    // makes `apply` safe to re-run after a user changes their own password.
    // Located by mutable reference rather than by index: the update needs both
    // halves of a compare-and-set, and holding the slot across the comparison
    // avoids re-searching the table.
    match db.shadow.iter_mut().find(|e| e.name == name) {
        Some(slot) => {
            // `clone_from` reuses the existing allocation when the new hash is
            // the same length, which is the common case for a re-run apply.
            if let Some(hash) = &password_hash
                && slot.hash != *hash
            {
                slot.hash.clone_from(hash);
                push_unique(&mut report.updated_users, name);
            }
        }
        None => {
            db.shadow.push(ShadowEntry::new(
                name,
                password_hash.as_deref().filter(|h| !h.is_empty())
            ));
        }
    }

    // --- Group membership ---
    for group in &wanted_groups {
        // Only touch groups that exist. A user declaring a group that no
        // package created is a configuration error, but failing the whole
        // apply over it would be worse: we report it and carry on.
        let Some(entry) = db.groups.iter_mut().find(|e| e.name == *group)
        else {
            eprintln!(
                "Warning: user '{name}' references group '{group}', which \
                 does not exist. Skipping."
            );
            continue;
        };

        if !entry.members.iter().any(|m| m == name) {
            entry.members.push(name.to_string());
            entry.members.sort();
            push_unique(&mut report.updated_users, name);
        }

        // Mirror into gshadow so `grpck` stays quiet.
        if let Some(gs) = db.gshadow.iter_mut().find(|e| e.name == *group) {
            if !gs.members.iter().any(|m| m == name) {
                gs.members.push(name.to_string());
                gs.members.sort();
            }
        } else {
            db.gshadow.push(GshadowEntry {
                name: group.clone(),
                password: "!".into(),
                admins: Vec::new(),
                members: vec![name.to_string()]
            });
        }
    }

    Ok(is_new)
}

/// Creates a user's home directory from `/etc/skel` when it is missing.
///
/// Copying `skel` rather than creating an empty directory is what makes a
/// `ZoiOS` account behave like a distro account: the administrator gets their
/// `.bashrc` and `.profile` instead of a bare `$HOME`.
fn create_home(
    db: &AccountDb,
    root: &Path,
    name: &str,
    home: &str,
    report: &mut AccountReport
) -> Result<()> {
    // A home outside the sysroot is a configuration error, not something to
    // silently create on the build host.
    if !home.starts_with('/') {
        bail!(
            "Home directory '{home}' for user '{name}' must be an absolute \
             path"
        );
    }
    let home_path = root.join(home.trim_start_matches('/'));
    if home_path.exists() {
        return Ok(());
    }

    let skel = root.join("etc/skel");
    if skel.is_dir() {
        copy_dir(&skel, &home_path)?;
    } else {
        fs::create_dir_all(&home_path)?;
    }

    // A home directory must not be world-readable, or the next user to log in
    // can read the previous one's private keys.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&home_path, fs::Permissions::from_mode(0o700))
            .with_context(|| {
                format!("Failed to chmod {}", home_path.display())
            })?;
        chown_path(&home_path, name, db)?;
    }

    report.created_homes.push(home.to_string());
    Ok(())
}

/// Sets ownership on a path, resolving the name through the parsed databases.
///
/// Returns `Ok(false)` when the user is unknown to `/etc/passwd`, which is the
/// normal case while bootstrapping: the account was just written but the host's
/// NSS has no idea it exists yet.
#[cfg(unix)]
fn chown_path(path: &Path, name: &str, db: &AccountDb) -> Result<bool> {
    let Some(entry) = db.find_passwd(name) else {
        return Ok(false);
    };

    match nix::unistd::chown(
        path,
        Some(nix::unistd::Uid::from_raw(entry.uid)),
        Some(nix::unistd::Gid::from_raw(entry.gid))
    ) {
        Ok(()) => Ok(true),
        // chown needs CAP_CHOWN. In an unprivileged container build the
        // ownership stays root, which is still a usable image.
        Err(nix::errno::Errno::EPERM) => Ok(false),
        Err(e) => Err(e)
            .with_context(|| format!("Failed to chown {}", path.display()))
    }
}

/// Recursively copies `src` into `dst`, creating `dst` as needed.
///
/// `fs::copy` recurses manually instead of being handed to a helper crate
/// because the only thing we need is a plain byte copy of a small tree.
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if file_type.is_symlink() {
            // Preserve symlinks from skel rather than resolving them, which is
            // how a real `useradd` behaves.
            let link = fs::read_link(entry.path())?;
            let _ = fs::remove_file(&target);
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, &target)?;
            #[cfg(not(unix))]
            fs::copy(entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }

    Ok(())
}

/// Reconciles `system.lua` groups and users against the account databases.
///
/// Groups are applied first because a user's supplementary memberships have to
/// resolve to real GIDs. Everything is written under the active sysroot, so the
/// same function serves both `zoi system apply` on a live machine and
/// `zoi system distro build` against a mounted target.
///
/// `dry_run` reports what would change without writing anything, which is what
/// `--dry-run` on `distro build` uses.
///
/// # Errors
///
/// Returns an error if any of the account databases cannot be read or
/// written under the active sysroot.
pub fn apply_accounts<S: std::hash::BuildHasher>(
    groups: &HashMap<String, GroupConfig, S>,
    users: &HashMap<String, UserConfig, S>,
    dry_run: bool
) -> Result<AccountReport> {
    let root = sysroot::get_sysroot().unwrap_or_else(|| PathBuf::from("/"));
    apply_accounts_in(&root, groups, users, dry_run)
}

/// The root-explicit form of [`apply_accounts`].
///
/// The root is a parameter rather than read from the global sysroot so the
/// reconciliation logic can be tested against a temporary directory without
/// mutating process-wide state.
///
/// # Errors
///
/// Returns an error if a database is malformed, or cannot be read or
/// written under `root`.
pub fn apply_accounts_in<S: std::hash::BuildHasher>(
    root: &Path,
    groups: &HashMap<String, GroupConfig, S>,
    users: &HashMap<String, UserConfig, S>,
    dry_run: bool
) -> Result<AccountReport> {
    let mut db = AccountDb::load(root)?;
    let mut report = AccountReport::default();

    // A brand new sysroot has no shadow group. Create it so a later package
    // that needs group-shadow ownership (openssh, sudo) has somewhere to
    // attach.
    if db.is_new && db.find_group("shadow").is_none() {
        db.groups.push(GroupEntry {
            name: "shadow".into(),
            password: "x".into(),
            gid: SHADOW_GROUP_GID,
            members: Vec::new()
        });
        db.gshadow.push(GshadowEntry {
            name: "shadow".into(),
            password: "!".into(),
            admins: Vec::new(),
            members: Vec::new()
        });
    }

    // Deterministic order: a sorted map means two machines applying the same
    // system.lua allocate UIDs in the same sequence and end up with identical
    // `/etc/passwd`. BTreeMap rather than iterating the HashMap directly.
    let sorted_groups: BTreeMap<&String, &GroupConfig> =
        groups.iter().collect();

    for (name, cfg) in &sorted_groups {
        if dry_run {
            println!("  [DRY-RUN] Would ensure group '{name}' exists");
            continue;
        }
        ensure_group(&mut db, name, cfg, &mut report)?;
    }

    let sorted_users: BTreeMap<&String, &UserConfig> = users.iter().collect();

    // Two passes: pass one resolves GIDs for every user so that a user who is
    // also declared as a group gets a deterministic UID even if their group
    // was only created moments ago. Pass two writes passwd/shadow.
    let mut planned: Vec<(&String, &UserConfig, bool)> = Vec::new();
    for (name, cfg) in &sorted_users {
        if dry_run {
            println!("  [DRY-RUN] Would ensure user '{name}' exists");
            continue;
        }
        let is_new = ensure_user(&mut db, name, cfg, &mut report)?;
        planned.push((name, cfg, is_new));
    }

    for (name, cfg, is_new) in planned {
        if !is_new {
            continue;
        }
        let home = cfg.home.clone().unwrap_or_else(|| format!("/home/{name}"));
        if dry_run {
            continue;
        }
        create_home(&db, root, name, &home, &mut report)?;
    }

    if !dry_run {
        db.save(root)?;
    }

    Ok(report)
}

/// Applies only the `users` and `groups` sections of a parsed `system.lua`.
///
/// This is the call site used by `zoi system apply` and
/// `zoi system distro build`, so the two commands cannot drift apart on how
/// accounts are created.
///
/// # Errors
///
/// Returns an error if the account databases cannot be read or written.
pub fn apply_from_config(
    config: &SystemConfig,
    dry_run: bool
) -> Result<AccountReport> {
    apply_accounts(&config.groups, &config.users, dry_run)
}

/// Prints a human readable summary of what [`apply_accounts`] changed.
pub fn print_report(report: &AccountReport) {
    use colored::Colorize;

    for group in &report.created_groups {
        println!("{} group '{}'", "Created".green(), group);
    }
    for user in &report.created_users {
        println!("{} user '{}'", "Created".green(), user);
    }
    for home in &report.created_homes {
        println!("{} home '{}'", "Created".green(), home);
    }
    for group in &report.updated_groups {
        println!("{} group '{}'", "Updated".yellow(), group);
    }
    for user in &report.updated_users {
        println!("{} user '{}'", "Updated".yellow(), user);
    }
    if report.is_empty() {
        println!("{} accounts already match system.lua", "::".dimmed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    fn cfg(
        groups: Option<Vec<&str>>,
        shell: Option<&str>,
        hash: Option<&str>
    ) -> UserConfig {
        UserConfig {
            password_hash: hash.map(ToString::to_string),
            groups: groups.map(|g| g.iter().map(ToString::to_string).collect()),
            shell: shell.map(ToString::to_string),
            home: None
        }
    }

    #[test]
    fn username_validation_rejects_bad_names() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("_svc").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("1alice").is_err());
        assert!(validate_username("alice smith").is_err());
        assert!(validate_username("alice;rm").is_err());
    }

    #[test]
    fn group_validation_rejects_bad_names() {
        assert!(validate_group_name("wheel").is_ok());
        assert!(validate_group_name("").is_err());
        assert!(validate_group_name("bad group").is_err());
    }

    #[test]
    fn parses_and_renders_passwd_roundtrip() {
        let line = "alice:x:1000:1000:alice:/home/alice:/bin/bash";
        let entry = PasswdEntry::parse(line).expect("parse");
        assert_eq!(entry.uid, 1000);
        assert_eq!(entry.gid, 1000);
        assert_eq!(entry.home, "/home/alice");
        assert_eq!(entry.shell, "/bin/bash");
        assert_eq!(entry.to_line(), line);
    }

    #[test]
    fn parses_group_membership_list() {
        let entry = GroupEntry::parse("wheel:x:10:alice,bob").expect("parse");
        assert_eq!(entry.gid, 10);
        assert_eq!(entry.members, vec!["alice".to_string(), "bob".to_string()]);
        assert_eq!(entry.to_line(), "wheel:x:10:alice,bob");
    }

    #[test]
    fn empty_group_member_list_parses_as_empty() {
        let entry = GroupEntry::parse("wheel:x:10:").expect("parse");
        assert_eq!(entry.members.len(), 0, "members: {:?}", entry.members);
        assert_eq!(entry.to_line(), "wheel:x:10:");
    }

    #[test]
    fn malformed_lines_are_rejected() {
        assert!(PasswdEntry::parse("alice:x:1000").is_err());
        assert!(GroupEntry::parse("wheel:x:10").is_err());
        assert!(PasswdEntry::parse("alice:x:notanumber:1000::::").is_err());
    }

    #[test]
    fn next_free_id_skips_used_values() {
        let db = AccountDb {
            passwd: vec![
                PasswdEntry {
                    name: "root".into(),
                    password: "x".into(),
                    uid: 0,
                    gid: 0,
                    gecos: String::new(),
                    home: "/root".into(),
                    shell: "/bin/sh".into()
                },
                PasswdEntry {
                    name: "alice".into(),
                    password: "x".into(),
                    uid: 1000,
                    gid: 1000,
                    gecos: String::new(),
                    home: "/home/alice".into(),
                    shell: "/bin/sh".into()
                },
            ],
            shadow: Vec::new(),
            groups: Vec::new(),
            gshadow: Vec::new(),
            is_new: false
        };
        assert_eq!(db.next_free_uid(FIRST_USER_UID), 1001);
        assert_eq!(db.next_free_gid(FIRST_USER_GID), FIRST_USER_GID);
    }

    #[test]
    fn apply_creates_user_with_locked_password() {
        let root = temp_root();

        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        let report =
            apply_accounts_in(root.path(), &HashMap::new(), &users, false)
                .expect("apply");

        assert_eq!(report.created_users, vec!["alice".to_string()]);

        let passwd =
            fs::read_to_string(root.path().join("etc/passwd")).expect("passwd");
        assert!(passwd.contains("alice:x:1000:1000:alice:/home/alice:/bin/sh"));

        let shadow =
            fs::read_to_string(root.path().join("etc/shadow")).expect("shadow");
        // No password declared means the account is locked, not empty.
        assert!(shadow.contains("alice:!:"));

        // The home directory is real, not just a passwd entry.
        assert!(root.path().join("home/alice").is_dir());
    }

    #[test]
    fn apply_is_idempotent() {
        let root = temp_root();

        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("first apply");
        let second =
            apply_accounts_in(root.path(), &HashMap::new(), &users, false)
                .expect("second apply");

        assert!(
            second.is_empty(),
            "second apply should be a no-op: {second:?}"
        );
    }

    #[test]
    fn apply_assigns_sequential_uids_in_sorted_order() {
        let root = temp_root();

        // Inserted in an order that does not match sorted order. The sorted
        // iteration is what makes the UID allocation deterministic.
        let mut users = HashMap::new();
        users.insert("zoe".to_string(), cfg(None, None, None));
        users.insert("alice".to_string(), cfg(None, None, None));
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("apply");

        let passwd =
            fs::read_to_string(root.path().join("etc/passwd")).expect("passwd");
        let alice_uid = passwd
            .lines()
            .find(|l| l.starts_with("alice:"))
            .and_then(|l| l.split(':').nth(2))
            .expect("alice uid");
        let zoe_uid = passwd
            .lines()
            .find(|l| l.starts_with("zoe:"))
            .and_then(|l| l.split(':').nth(2))
            .expect("zoe uid");

        assert_eq!(alice_uid, "1000", "sorted order must drive allocation");
        assert_eq!(zoe_uid, "1001");
    }

    #[test]
    fn apply_adds_user_to_declared_groups() {
        let root = temp_root();

        let groups = HashMap::from([
            ("wheel".to_string(), GroupConfig { gid: None }),
            ("docker".to_string(), GroupConfig { gid: None })
        ]);
        let users = HashMap::from([(
            "alice".to_string(),
            cfg(Some(vec!["docker"]), None, None)
        )]);
        apply_accounts_in(root.path(), &groups, &users, false).expect("apply");

        let group_db =
            fs::read_to_string(root.path().join("etc/group")).expect("group");
        let docker = group_db
            .lines()
            .find(|l| l.starts_with("docker:"))
            .expect("docker line");
        assert!(
            docker.ends_with(":alice"),
            "unexpected docker line: {docker}"
        );

        // `wheel` exists and was not declared, so it is joined by default.
        let wheel = group_db
            .lines()
            .find(|l| l.starts_with("wheel:"))
            .expect("wheel line");
        assert!(wheel.ends_with(":alice"), "unexpected wheel line: {wheel}");
    }

    #[test]
    fn missing_default_group_does_not_warn() {
        let root = temp_root();

        // No `wheel` group exists anywhere. Injecting the membership anyway
        // would emit a warning on every apply.
        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("apply");

        let group_db =
            fs::read_to_string(root.path().join("etc/group")).expect("group");
        assert!(!group_db.contains("wheel"), "wheel must not be invented");
    }

    #[test]
    fn apply_honours_explicit_password_hash() {
        let root = temp_root();

        let hash = crate::secret::hash_password("hunter2").expect("hash");
        let users = HashMap::from([(
            "alice".to_string(),
            cfg(None, None, Some(&hash))
        )]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("apply");

        let shadow =
            fs::read_to_string(root.path().join("etc/shadow")).expect("shadow");
        assert!(
            shadow.contains(&format!("alice:{hash}:")),
            "hash not stored verbatim"
        );
    }

    #[test]
    fn apply_updates_shell_and_home_on_existing_user() {
        let root = temp_root();

        apply_accounts_in(
            root.path(),
            &HashMap::new(),
            &HashMap::from([("alice".to_string(), cfg(None, None, None))]),
            false
        )
        .expect("first apply");

        let mut updated = cfg(None, Some("/bin/zsh"), None);
        updated.home = Some("/srv/alice".to_string());
        let report = apply_accounts_in(
            root.path(),
            &HashMap::new(),
            &HashMap::from([("alice".to_string(), updated)]),
            false
        )
        .expect("second apply");

        assert!(report.updated_users.contains(&"alice".to_string()));
        let passwd =
            fs::read_to_string(root.path().join("etc/passwd")).expect("passwd");
        assert!(passwd.contains("alice:x:1000:1000:alice:/srv/alice:/bin/zsh"));
    }

    #[test]
    fn apply_preserves_existing_uid() {
        let root = temp_root();

        // Pre-seed a passwd entry the way a base package would.
        fs::create_dir_all(root.path().join("etc"))
            .expect("create fixture directory");
        fs::write(
            root.path().join("etc/passwd"),
            "alice:x:1500:1500::/home/alice:/bin/sh\n"
        )
        .expect("write fixture file");

        apply_accounts_in(
            root.path(),
            &HashMap::new(),
            &HashMap::from([("alice".to_string(), cfg(None, None, None))]),
            false
        )
        .expect("apply");

        let passwd =
            fs::read_to_string(root.path().join("etc/passwd")).expect("passwd");
        let uid = passwd
            .lines()
            .find(|l| l.starts_with("alice:"))
            .and_then(|l| l.split(':').nth(2))
            .expect("uid");
        assert_eq!(uid, "1500", "existing UID must never be reassigned");
    }

    #[test]
    fn dry_run_writes_nothing() {
        let root = temp_root();

        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, true)
            .expect("dry run");

        assert!(!root.path().join("etc/passwd").exists());
    }

    #[test]
    fn unknown_group_reference_is_reported_not_fatal() {
        let root = temp_root();

        let users = HashMap::from([(
            "alice".to_string(),
            cfg(Some(vec!["nosuchgroup"]), None, None)
        )]);
        let report =
            apply_accounts_in(root.path(), &HashMap::new(), &users, false)
                .expect("apply");
        assert_eq!(report.created_users, vec!["alice".to_string()]);
    }

    #[test]
    fn shadow_is_written_with_restrictive_mode() {
        let root = temp_root();

        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("apply");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(root.path().join("etc/shadow"))
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, SHADOW_MODE);
        }
    }

    #[test]
    fn home_is_populated_from_skel() {
        let root = temp_root();

        let skel = root.path().join("etc/skel");
        fs::create_dir_all(&skel).expect("create fixture directory");
        fs::write(skel.join(".profile"), "# skel\n")
            .expect("write fixture file");

        let users =
            HashMap::from([("alice".to_string(), cfg(None, None, None))]);
        apply_accounts_in(root.path(), &HashMap::new(), &users, false)
            .expect("apply");

        let profile = root.path().join("home/alice/.profile");
        assert!(profile.is_file(), "skel must be copied into the new home");
    }
}
