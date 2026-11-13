//! Build-time stripping of ELF files and debug-information splitting.
//!
//! After `package()` has staged a package, every ELF file in the staging area
//! (found by its header, not by name or by the `bins` list) is stripped with
//! binutils, the way distribution build tools such as `makepkg` do it:
//!
//! - executables: `--strip-all`
//! - shared libraries: `--strip-unneeded`, which keeps the symbols the dynamic
//!   linker needs
//! - static libraries (`.a`) and object files: `--strip-debug`, which keeps
//!   their symbol tables and LTO data usable for linking
//!
//! When the package asks for a debug sub-package, the debug information of
//! each executable and shared library is first copied out with `objcopy
//! --only-keep-debug` to `usr/lib/debug/.build-id/xx/yyyy.debug`, where gdb
//! and debuginfod look for it.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow};
use colored::Colorize;
use glob::{MatchOptions, Pattern};
use rayon::prelude::*;
use walkdir::WalkDir;
use zoi_core::types::{DebugConfig, StripMode};

/// The kind of strippable file, which decides the strip flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripKind {
    /// An executable program (`ET_EXEC`, or a PIE `ET_DYN` with an
    /// interpreter).
    Executable,
    /// A shared library (`ET_DYN`).
    SharedLibrary,
    /// A relocatable object (`.o`, kernel module).
    Object,
    /// A static library archive (`.a`).
    StaticLibrary
}

/// What was learned about a file from its header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// How the file should be stripped.
    pub kind: StripKind,
    /// The GNU build-id, if the file carries one.
    pub build_id: Option<Vec<u8>>,
    /// Whether the file has DWARF (`.debug_*`/`.zdebug_*`) sections.
    pub has_debug_info: bool,
    /// Whether the file has a static symbol table (`.symtab`).
    pub has_symtab: bool
}

impl FileInfo {
    /// Whether stripping this file can remove anything.
    fn has_strippable_data(&self) -> bool {
        self.has_debug_info || self.has_symtab
    }
}

/// Summary of a staging-area strip pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StripReport {
    /// Number of files that were stripped.
    pub stripped: usize,
    /// Number of debug files written to the debug staging area.
    pub debug_files: usize,
    /// Total size of the stripped files before stripping.
    pub bytes_before: u64,
    /// Total size of the stripped files after stripping.
    pub bytes_after: u64,
    /// Set when stripping was skipped because binutils was not found.
    pub skipped: bool
}

/// The binutils programs used for stripping.
#[derive(Debug, Clone)]
struct Tools {
    /// The `strip` program.
    strip: OsString,
    /// The `objcopy` program, used to split off debug information.
    objcopy: OsString
}

impl Tools {
    /// Finds `strip` and `objcopy`, honoring the conventional `STRIP` and
    /// `OBJCOPY` overrides (for cross toolchains).
    fn find(need_objcopy: bool) -> Option<Self> {
        let strip = std::env::var_os("STRIP").unwrap_or_else(|| "strip".into());
        let objcopy =
            std::env::var_os("OBJCOPY").unwrap_or_else(|| "objcopy".into());
        if !tool_works(&strip) || (need_objcopy && !tool_works(&objcopy)) {
            return None;
        }
        Some(Self { strip, objcopy })
    }
}

/// Whether `program --version` runs successfully.
fn tool_works(program: &OsStr) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Returns the strip flags for a file of `kind` at strip level `mode`, or
/// `None` when nothing should be stripped.
#[must_use]
pub fn strip_flags(
    kind: StripKind,
    mode: StripMode
) -> Option<&'static [&'static str]> {
    match (mode, kind) {
        (StripMode::None, _) => None,
        (StripMode::Full, StripKind::Executable) => Some(&["--strip-all"]),
        (StripMode::Full, StripKind::SharedLibrary) => {
            Some(&["--strip-unneeded"])
        }
        (_, StripKind::StaticLibrary) => {
            Some(&["--strip-debug", "--enable-deterministic-archives"])
        }
        (StripMode::Debug | StripMode::Full, _) => Some(&["--strip-debug"])
    }
}

/// Strips every ELF file and static library under `staging_dir` according
/// to `config`.
///
/// When `debug_staging` is given, the debug information of each executable
/// and shared library is first written below it, in the same root
/// (`usrroot/usr/lib/debug/...` or `pkgstore/lib/debug/...`) as the file it
/// came from.
///
/// Failing to strip one file only prints a warning and leaves that file as it
/// was. When `strip` (or `objcopy`, if needed) is not installed, a warning is
/// printed and nothing is changed.
///
/// # Errors
///
/// Returns an error if an `exclude` pattern is invalid or the staging
/// directory cannot be walked.
pub fn strip_staging(
    staging_dir: &Path,
    config: &DebugConfig,
    debug_staging: Option<&Path>,
    quiet: bool
) -> Result<StripReport> {
    let mut report = StripReport::default();
    if config.strip == StripMode::None {
        return Ok(report);
    }

    let excludes = config
        .exclude
        .iter()
        .map(|p| {
            Pattern::new(p.trim_start_matches('/')).map_err(|e| {
                anyhow!("Invalid debug.exclude pattern '{p}': {e}")
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut candidates = Vec::new();
    for entry in WalkDir::new(staging_dir).min_depth(1) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(staging_dir)?.to_path_buf();
        if is_excluded(&rel, &excludes) {
            continue;
        }
        match inspect_file(entry.path()) {
            // Already-stripped files are left alone.
            Ok(Some(info)) if info.has_strippable_data() => {
                candidates.push((entry.into_path(), rel, info));
            }
            Ok(_) => {}
            Err(e) => eprintln!(
                "{} Could not read {}: {e}",
                "Warning:".yellow(),
                rel.display()
            )
        }
    }
    if candidates.is_empty() {
        return Ok(report);
    }

    let Some(tools) = Tools::find(debug_staging.is_some()) else {
        eprintln!(
            "{} binutils 'strip'/'objcopy' not found; ELF files are packaged \
             unstripped. Install binutils (or set debug.strip = \"none\") to \
             silence this.",
            "Warning:".yellow()
        );
        report.skipped = true;
        return Ok(report);
    };

    if !quiet {
        println!("{} Stripping ELF files...", "::".bold().blue());
    }

    // Debug targets are assigned up front so that two files with the same
    // build-id (hard links, identical copies) never write the same debug
    // file concurrently.
    let mut seen_targets = HashSet::new();
    let jobs: Vec<_> = candidates
        .into_iter()
        .map(|(path, rel, info)| {
            let target = debug_staging
                .filter(|_| {
                    matches!(
                        info.kind,
                        StripKind::Executable | StripKind::SharedLibrary
                    )
                })
                .map(|dir| dir.join(debug_file_path(&rel, &info)))
                .filter(|t| seen_targets.insert(t.clone()));
            (path, rel, info, target)
        })
        .collect();

    let results: Vec<_> = jobs
        .par_iter()
        .map(|(path, rel, info, target)| {
            let before = fs::metadata(path).map_or(0, |m| m.len());
            let outcome =
                strip_one(&tools, path, info, config.strip, target.as_deref());
            let after = fs::metadata(path).map_or(before, |m| m.len());
            (rel, outcome, before, after)
        })
        .collect();

    for (rel, outcome, before, after) in results {
        match outcome {
            Ok(wrote_debug) => {
                report.stripped += 1;
                report.bytes_before += before;
                report.bytes_after += after;
                if wrote_debug {
                    report.debug_files += 1;
                }
            }
            Err(e) => eprintln!(
                "{} Failed to strip {}: {e}",
                "Warning:".yellow(),
                rel.display()
            )
        }
    }

    if !quiet && report.stripped > 0 {
        println!(
            "{} Stripped {} file(s): {} -> {}.",
            "::".bold().green(),
            report.stripped,
            zoi_core::utils::format_bytes(report.bytes_before),
            zoi_core::utils::format_bytes(report.bytes_after)
        );
    }
    Ok(report)
}

/// Strips one file, first extracting its debug information to
/// `debug_target` when given. Returns whether a debug file was written.
///
/// The file's permissions are restored afterwards; read-only files are made
/// writable only for the duration of the strip.
fn strip_one(
    tools: &Tools,
    path: &Path,
    info: &FileInfo,
    mode: StripMode,
    debug_target: Option<&Path>
) -> Result<bool> {
    let Some(flags) = strip_flags(info.kind, mode) else {
        return Ok(false);
    };
    let original = fs::metadata(path)?.permissions();
    let result = with_writable(path, &original, || {
        let mut wrote_debug = false;
        if let Some(target) = debug_target {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            run(Command::new(&tools.objcopy)
                .arg("--only-keep-debug")
                .arg(path)
                .arg(target))?;
            set_mode(target, 0o644)?;
            wrote_debug = true;
        }

        if let Err(e) = run(Command::new(&tools.strip).args(flags).arg(path)) {
            // The file stays unstripped, so its debug information must not
            // ship separately as well.
            if let Some(target) = debug_target.filter(|_| wrote_debug) {
                let _ = fs::remove_file(target);
            }
            return Err(e);
        }

        // Without a build-id, debuggers find the split file through a
        // `.gnu_debuglink` section naming it.
        if wrote_debug
            && info.build_id.is_none()
            && let Some(target) = debug_target
        {
            let mut link = OsString::from("--add-gnu-debuglink=");
            link.push(target);
            run(Command::new(&tools.objcopy).arg(link).arg(path))?;
        }
        Ok(wrote_debug)
    });
    fs::set_permissions(path, original)?;
    result
}

/// Runs `f` with `path` writable by its owner.
fn with_writable<T>(
    path: &Path,
    original: &fs::Permissions,
    f: impl FnOnce() -> Result<T>
) -> Result<T> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = original.mode();
        if mode & 0o200 == 0 {
            let writable = fs::Permissions::from_mode(mode | 0o200);
            fs::set_permissions(path, writable)?;
        }
    }
    #[cfg(not(unix))]
    let _ = (path, original);
    f()
}

/// Sets the Unix permission bits of `path`; a no-op elsewhere.
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Runs `command`, turning a non-zero exit into an error carrying its
/// stderr.
fn run(command: &mut Command) -> Result<()> {
    let output = command.stdin(Stdio::null()).output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!(
        "{} exited with {}: {}",
        command.get_program().to_string_lossy(),
        output.status,
        stderr.trim()
    ))
}

/// Splits a staging-relative path into its staging root (`usrroot`,
/// `pkgstore`, ...) and the path inside that root.
fn split_root(rel: &Path) -> (&str, PathBuf) {
    let mut components = rel.components();
    let first = components
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("");
    match first {
        "pkgstore" | "usrroot" | "usrhome" | "createpkgdir" => {
            (first, components.as_path().to_path_buf())
        }
        // Files written straight into the staging directory are pooled into
        // the package store.
        _ => ("pkgstore", rel.to_path_buf())
    }
}

/// Whether `rel` (relative to the staging directory) matches one of the
/// `exclude` patterns, either as the path inside its root or as the full
/// placeholder path (`${usrroot}/usr/...`).
fn is_excluded(rel: &Path, excludes: &[Pattern]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let (root, inner) = split_root(rel);
    let inner = inner.to_string_lossy().replace('\\', "/");
    let placeholder = format!("${{{root}}}/{inner}");
    let options = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false
    };
    excludes.iter().any(|p| {
        p.matches_with(&inner, options) || p.matches_with(&placeholder, options)
    })
}

/// Returns where, relative to the debug staging directory, the debug
/// information of the staged file `rel` is written.
///
/// Debug files sit in the same root as the file they belong to: system files
/// (`usrroot`) under `usrroot/usr/lib/debug`, everything else under
/// `pkgstore/lib/debug`. Files with a build-id use the
/// `.build-id/xx/yyyy.debug` layout; others mirror their own path.
#[must_use]
pub fn debug_file_path(rel: &Path, info: &FileInfo) -> PathBuf {
    let (root, inner) = split_root(rel);
    let base = if root == "usrroot" {
        PathBuf::from("usrroot/usr/lib/debug")
    } else {
        PathBuf::from("pkgstore/lib/debug")
    };
    match &info.build_id {
        Some(id) if id.len() > 1 => {
            let hex = hex::encode(id);
            let (dir, file) = hex.split_at(2);
            base.join(".build-id")
                .join(dir)
                .join(format!("{file}.debug"))
        }
        _ => {
            let mut name = inner.into_os_string();
            name.push(".debug");
            base.join(name)
        }
    }
}

/// Magic bytes at the start of every ELF file.
const ELF_MAGIC: &[u8] = b"\x7fELF";
/// Magic bytes at the start of an `ar` archive (static library).
const AR_MAGIC: &[u8] = b"!<arch>\n";
/// ELF type of a relocatable object.
const ET_REL: u16 = 1;
/// ELF type of a non-PIE executable.
const ET_EXEC: u16 = 2;
/// ELF type of a shared object (shared library or PIE executable).
const ET_DYN: u16 = 3;
/// Program header type naming the program interpreter.
const PT_INTERP: u32 = 3;
/// Section type of the static symbol table.
const SHT_SYMTAB: u32 = 2;
/// Note type of the GNU build-id.
const NT_GNU_BUILD_ID: u32 = 3;
/// Upper bound for header tables and string tables read from a file, so a
/// corrupt header cannot make the build allocate gigabytes.
const MAX_TABLE: u64 = 64 * 1024 * 1024;

/// Reads a file's header and decides whether and how it can be stripped.
///
/// Returns `None` for anything that is neither an ELF file of a strippable
/// type nor a `.a` static library.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub fn inspect_file(path: &Path) -> Result<Option<FileInfo>> {
    let mut file = File::open(path)?;
    let mut magic = Vec::with_capacity(8);
    file.by_ref().take(8).read_to_end(&mut magic)?;

    if magic == AR_MAGIC && path.extension().is_some_and(|e| e == "a") {
        return Ok(Some(FileInfo {
            kind: StripKind::StaticLibrary,
            build_id: None,
            has_debug_info: true,
            has_symtab: false
        }));
    }
    if !magic.starts_with(ELF_MAGIC) {
        return Ok(None);
    }
    // A file that looks like ELF but whose headers do not parse is left
    // alone rather than failing the build.
    Ok(parse_elf(&mut file, path).ok().flatten())
}

/// Byte-order aware reader over an in-memory slice.
struct Bytes<'a> {
    /// The bytes being read.
    data: &'a [u8],
    /// Whether multi-byte fields are big-endian.
    big_endian: bool
}

impl Bytes<'_> {
    /// Reads a 16-bit field at `at`.
    fn u16(&self, at: usize) -> Result<u16> {
        let b: [u8; 2] = self.slice(at, 2)?.try_into()?;
        Ok(if self.big_endian {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    /// Reads a 32-bit field at `at`.
    fn u32(&self, at: usize) -> Result<u32> {
        let b: [u8; 4] = self.slice(at, 4)?.try_into()?;
        Ok(if self.big_endian {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    /// Reads a 64-bit field at `at`.
    fn u64(&self, at: usize) -> Result<u64> {
        let b: [u8; 8] = self.slice(at, 8)?.try_into()?;
        Ok(if self.big_endian {
            u64::from_be_bytes(b)
        } else {
            u64::from_le_bytes(b)
        })
    }

    /// Reads an address-sized field (4 bytes for 32-bit ELF, 8 for 64-bit).
    fn word(&self, at: usize, is_64: bool) -> Result<u64> {
        if is_64 {
            self.u64(at)
        } else {
            self.u32(at).map(u64::from)
        }
    }

    /// Returns `len` bytes at `at`, or an error if they are out of range.
    fn slice(&self, at: usize, len: usize) -> Result<&[u8]> {
        at.checked_add(len)
            .and_then(|end| self.data.get(at..end))
            .ok_or_else(|| anyhow!("truncated ELF data"))
    }
}

/// Reads `len` bytes at `offset`, refusing implausibly large tables.
fn read_at(file: &mut File, offset: u64, len: u64) -> Result<Vec<u8>> {
    if len > MAX_TABLE {
        return Err(anyhow!("ELF table too large"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; usize::try_from(len)?];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

/// One section header, reduced to the fields used here.
struct Section {
    /// Offset of the section name in the section-name string table.
    name: u32,
    /// Section type (`sh_type`).
    kind: u32,
    /// File offset of the section contents.
    offset: u64,
    /// Size of the section contents in bytes.
    size: u64
}

/// Parses the ELF headers of `file` (named `path`) into a [`FileInfo`].
fn parse_elf(file: &mut File, path: &Path) -> Result<Option<FileInfo>> {
    let ident = read_at(file, 0, 64)?;
    let is_64 = match ident.get(4) {
        Some(1) => false,
        Some(2) => true,
        _ => return Ok(None)
    };
    let big_endian = match ident.get(5) {
        Some(1) => false,
        Some(2) => true,
        _ => return Ok(None)
    };
    let header = Bytes {
        data: &ident,
        big_endian
    };

    let e_type = header.u16(16)?;
    let (segment_offset, section_offset) = if is_64 {
        (header.u64(32)?, header.u64(40)?)
    } else {
        (u64::from(header.u32(28)?), u64::from(header.u32(32)?))
    };
    let base = if is_64 { 54 } else { 42 };
    let segment_entry_size = u64::from(header.u16(base)?);
    let segment_count = u64::from(header.u16(base + 2)?);
    let section_entry_size = u64::from(header.u16(base + 4)?);
    let section_count = u64::from(header.u16(base + 6)?);
    let section_names_index = usize::from(header.u16(base + 8)?);

    let has_interp = if segment_offset != 0 && segment_count > 0 {
        let table =
            read_at(file, segment_offset, segment_entry_size * segment_count)?;
        let table = Bytes {
            data: &table,
            big_endian
        };
        let mut found = false;
        for i in 0..usize::try_from(segment_count)? {
            let at = i * usize::try_from(segment_entry_size)?;
            if table.u32(at)? == PT_INTERP {
                found = true;
                break;
            }
        }
        found
    } else {
        false
    };

    let kind = match e_type {
        ET_EXEC => StripKind::Executable,
        ET_REL => StripKind::Object,
        ET_DYN => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            // `libfoo.so`, or a versioned `libfoo.so.1.2`.
            let looks_like_library =
                Path::new(&name).extension().is_some_and(|ext| ext == "so")
                    || name.contains(".so.");
            if has_interp && !looks_like_library {
                StripKind::Executable
            } else {
                StripKind::SharedLibrary
            }
        }
        _ => return Ok(None)
    };

    let mut sections = Vec::new();
    if section_offset != 0 && section_count > 0 {
        let table =
            read_at(file, section_offset, section_entry_size * section_count)?;
        let table = Bytes {
            data: &table,
            big_endian
        };
        for i in 0..usize::try_from(section_count)? {
            let at = i * usize::try_from(section_entry_size)?;
            let (offset, size) = if is_64 {
                (table.u64(at + 24)?, table.u64(at + 32)?)
            } else {
                (table.word(at + 16, false)?, table.word(at + 20, false)?)
            };
            sections.push(Section {
                name: table.u32(at)?,
                kind: table.u32(at + 4)?,
                offset,
                size
            });
        }
    }

    let names = match sections.get(section_names_index) {
        Some(s) => read_at(file, s.offset, s.size)?,
        None => Vec::new()
    };
    let name_of = |s: &Section| -> &[u8] {
        let start = usize::try_from(s.name).unwrap_or(usize::MAX);
        names.get(start..).map_or(&[][..], |rest| {
            rest.split(|&b| b == 0).next().unwrap_or(&[])
        })
    };

    let mut info = FileInfo {
        kind,
        build_id: None,
        has_debug_info: false,
        has_symtab: false
    };
    let mut build_id_section = None;
    for section in &sections {
        let name = name_of(section);
        if name.starts_with(b".debug_") || name.starts_with(b".zdebug_") {
            info.has_debug_info = true;
        }
        if section.kind == SHT_SYMTAB {
            info.has_symtab = true;
        }
        if name == b".note.gnu.build-id" {
            build_id_section = Some((section.offset, section.size));
        }
    }
    if let Some((offset, size)) = build_id_section {
        let notes = read_at(file, offset, size)?;
        info.build_id = parse_build_id(&notes, big_endian);
    }
    Ok(Some(info))
}

/// Finds the `NT_GNU_BUILD_ID` descriptor in a note section.
fn parse_build_id(notes: &[u8], big_endian: bool) -> Option<Vec<u8>> {
    let notes = Bytes {
        data: notes,
        big_endian
    };
    let align = |n: usize| n.checked_add(3).map(|v| v & !3);
    let mut at = 0;
    while at + 12 <= notes.data.len() {
        let namesz = usize::try_from(notes.u32(at).ok()?).ok()?;
        let descsz = usize::try_from(notes.u32(at + 4).ok()?).ok()?;
        let kind = notes.u32(at + 8).ok()?;
        let name_at = at + 12;
        let desc_at = name_at.checked_add(align(namesz)?)?;
        let name = notes.slice(name_at, namesz).ok()?;
        if kind == NT_GNU_BUILD_ID && name == b"GNU\0" {
            return notes.slice(desc_at, descsz).ok().map(<[u8]>::to_vec);
        }
        at = desc_at.checked_add(align(descsz)?)?;
    }
    None
}
