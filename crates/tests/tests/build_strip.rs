//! Build-time stripping of ELF files and the `:debug` sub-package.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tar::Archive;
use tempfile::tempdir;
use zoi::pkg::lua::parser;
use zoi::pkg::package::{build, strip};
use zoi::pkg::types::{DebugConfig, PooledZpaManifest, StripMode};
use zoi::utils::get_platform;
use zstd::stream::read::Decoder as ZstdDecoder;

mod common;

const METADATA_HEAD: &str = r#"
    name = "striptest",
    repo = "core",
    version = "1.0.0",
    description = "strip test",
    maintainer = { name = "test", email = "test@example.com" },
    types = { "source" },
"#;

fn parse_with_debug(
    debug_field: &str
) -> anyhow::Result<zoi::pkg::types::Package> {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join("striptest.pkg.lua");
    fs::write(
        &path,
        format!("metadata({{{METADATA_HEAD}{debug_field}\n}})\n")
    )
    .expect("write pkg.lua");
    parser::parse_lua_package(
        path.to_str().expect("utf-8 path"),
        None,
        None,
        true
    )
}

#[test]
fn debug_table_is_parsed_from_metadata() {
    let pkg = parse_with_debug(
        r#"debug = {
            strip = "debug",
            sub_package = true,
            exclude = { "usr/lib/firmware/**" },
        },"#
    )
    .expect("parse");
    assert_eq!(
        pkg.debug,
        Some(DebugConfig {
            strip: StripMode::Debug,
            sub_package: true,
            exclude: vec!["usr/lib/firmware/**".to_string()]
        })
    );
}

#[test]
fn missing_debug_table_means_full_strip_without_debug_package() {
    let pkg = parse_with_debug("").expect("parse");
    assert_eq!(pkg.debug, None);
    let config = pkg.debug.unwrap_or_default();
    assert_eq!(config.strip, StripMode::Full);
    assert!(!config.sub_package);
}

#[test]
fn strip_accepts_booleans_and_rejects_unknown_levels() {
    let off = parse_with_debug("debug = { strip = false },").expect("parse");
    assert_eq!(off.debug.map(|d| d.strip), Some(StripMode::None));

    let on = parse_with_debug("debug = { strip = true },").expect("parse");
    assert_eq!(on.debug.map(|d| d.strip), Some(StripMode::Full));

    let none =
        parse_with_debug(r#"debug = { strip = "none" },"#).expect("parse");
    assert_eq!(none.debug.map(|d| d.strip), Some(StripMode::None));

    assert!(parse_with_debug(r#"debug = { strip = "most" },"#).is_err());
}

#[test]
fn strip_flags_depend_on_file_kind_and_level() {
    use strip::StripKind::{Executable, Object, SharedLibrary, StaticLibrary};

    let flags = |kind, mode| strip::strip_flags(kind, mode);
    assert_eq!(
        flags(Executable, StripMode::Full),
        Some(&["--strip-all"][..])
    );
    assert_eq!(
        flags(SharedLibrary, StripMode::Full),
        Some(&["--strip-unneeded"][..])
    );
    assert_eq!(flags(Object, StripMode::Full), Some(&["--strip-debug"][..]));
    // Static libraries keep their symbol tables, or they cannot be linked.
    assert_eq!(
        flags(StaticLibrary, StripMode::Full),
        Some(&["--strip-debug", "--enable-deterministic-archives"][..])
    );
    assert_eq!(
        flags(Executable, StripMode::Debug),
        Some(&["--strip-debug"][..])
    );
    assert_eq!(
        flags(SharedLibrary, StripMode::Debug),
        Some(&["--strip-debug"][..])
    );
    assert_eq!(flags(Executable, StripMode::None), None);
}

/// Builds a minimal little-endian ELF64 shared object with a section-name
/// table, a GNU build-id note and, optionally, a `.debug_info` section.
fn synthetic_elf(build_id: &[u8], with_debug: bool) -> Vec<u8> {
    fn name_offset(name: &str, names: &mut Vec<u8>) -> u32 {
        let at = names.len() as u32;
        names.extend_from_slice(name.as_bytes());
        names.push(0);
        at
    }

    let mut names = vec![0u8];
    let shstrtab_name = name_offset(".shstrtab", &mut names);
    let note_name = name_offset(".note.gnu.build-id", &mut names);
    let debug_name = name_offset(".debug_info", &mut names);

    let mut note = Vec::new();
    note.extend_from_slice(&4u32.to_le_bytes());
    note.extend_from_slice(&(build_id.len() as u32).to_le_bytes());
    note.extend_from_slice(&3u32.to_le_bytes());
    note.extend_from_slice(b"GNU\0");
    note.extend_from_slice(build_id);
    while note.len() % 4 != 0 {
        note.push(0);
    }
    let debug = b"debug-bytes".to_vec();

    let names_at = 64u64;
    let note_at = names_at + names.len() as u64;
    let debug_at = note_at + note.len() as u64;
    let shoff = debug_at + debug.len() as u64;

    let mut sections = vec![(0u32, 0u32, 0u64, 0u64)];
    sections.push((shstrtab_name, 3, names_at, names.len() as u64));
    sections.push((note_name, 7, note_at, note.len() as u64));
    if with_debug {
        sections.push((debug_name, 1, debug_at, debug.len() as u64));
    }

    let mut elf = Vec::new();
    elf.extend_from_slice(b"\x7fELF");
    elf.extend_from_slice(&[2, 1, 1, 0]);
    elf.extend_from_slice(&[0; 8]);
    elf.extend_from_slice(&3u16.to_le_bytes()); // ET_DYN
    elf.extend_from_slice(&62u16.to_le_bytes()); // x86-64
    elf.extend_from_slice(&1u32.to_le_bytes());
    elf.extend_from_slice(&0u64.to_le_bytes()); // entry
    elf.extend_from_slice(&0u64.to_le_bytes()); // phoff
    elf.extend_from_slice(&shoff.to_le_bytes());
    elf.extend_from_slice(&0u32.to_le_bytes()); // flags
    elf.extend_from_slice(&64u16.to_le_bytes()); // ehsize
    elf.extend_from_slice(&56u16.to_le_bytes()); // phentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // phnum
    elf.extend_from_slice(&64u16.to_le_bytes()); // shentsize
    elf.extend_from_slice(&(sections.len() as u16).to_le_bytes());
    elf.extend_from_slice(&1u16.to_le_bytes()); // shstrndx
    assert_eq!(elf.len(), 64);

    elf.extend_from_slice(&names);
    elf.extend_from_slice(&note);
    elf.extend_from_slice(&debug);
    for (name, kind, offset, size) in sections {
        elf.extend_from_slice(&name.to_le_bytes());
        elf.extend_from_slice(&kind.to_le_bytes());
        elf.extend_from_slice(&0u64.to_le_bytes()); // flags
        elf.extend_from_slice(&0u64.to_le_bytes()); // addr
        elf.extend_from_slice(&offset.to_le_bytes());
        elf.extend_from_slice(&size.to_le_bytes());
        elf.extend_from_slice(&0u32.to_le_bytes()); // link
        elf.extend_from_slice(&0u32.to_le_bytes()); // info
        elf.extend_from_slice(&1u64.to_le_bytes()); // addralign
        elf.extend_from_slice(&0u64.to_le_bytes()); // entsize
    }
    elf
}

#[test]
fn elf_headers_give_kind_build_id_and_debug_sections() {
    let tmp = tempdir().expect("tempdir");
    let id = [0xab, 0xcd, 0xef, 0x01, 0x23];

    let lib = tmp.path().join("libfoo.so.1");
    fs::write(&lib, synthetic_elf(&id, true)).expect("write");
    let info = strip::inspect_file(&lib).expect("read").expect("is ELF");
    assert_eq!(info.kind, strip::StripKind::SharedLibrary);
    assert_eq!(info.build_id.as_deref(), Some(&id[..]));
    assert!(info.has_debug_info);
    assert!(!info.has_symtab);

    let bare = tmp.path().join("libbare.so");
    fs::write(&bare, synthetic_elf(&id, false)).expect("write");
    let info = strip::inspect_file(&bare).expect("read").expect("is ELF");
    assert!(!info.has_debug_info);
}

#[test]
fn non_elf_files_and_non_library_archives_are_ignored() {
    let tmp = tempdir().expect("tempdir");

    let text = tmp.path().join("README");
    fs::write(&text, "hello").expect("write");
    assert_eq!(strip::inspect_file(&text).expect("read"), None);

    // A truncated file with only the ELF magic is not touched.
    let fake = tmp.path().join("fake");
    fs::write(&fake, b"\x7fELF short").expect("write");
    assert_eq!(strip::inspect_file(&fake).expect("read"), None);

    let archive = tmp.path().join("libfoo.a");
    fs::write(&archive, b"!<arch>\nmembers").expect("write");
    let info = strip::inspect_file(&archive)
        .expect("read")
        .expect("archive");
    assert_eq!(info.kind, strip::StripKind::StaticLibrary);

    // `ar` is also the container of .deb files; only `.a` is a library.
    let deb = tmp.path().join("pkg.deb");
    fs::write(&deb, b"!<arch>\nmembers").expect("write");
    assert_eq!(strip::inspect_file(&deb).expect("read"), None);
}

#[test]
fn debug_files_use_build_id_layout_in_the_binary_root() {
    let with_id = strip::FileInfo {
        kind: strip::StripKind::Executable,
        build_id: Some(vec![0xab, 0xcd, 0xef]),
        has_debug_info: true,
        has_symtab: true
    };
    assert_eq!(
        strip::debug_file_path(Path::new("usrroot/usr/bin/tool"), &with_id),
        PathBuf::from("usrroot/usr/lib/debug/.build-id/ab/cdef.debug")
    );
    assert_eq!(
        strip::debug_file_path(Path::new("pkgstore/bin/tool"), &with_id),
        PathBuf::from("pkgstore/lib/debug/.build-id/ab/cdef.debug")
    );

    let without_id = strip::FileInfo {
        build_id: None,
        ..with_id
    };
    assert_eq!(
        strip::debug_file_path(Path::new("usrroot/usr/bin/tool"), &without_id),
        PathBuf::from("usrroot/usr/lib/debug/usr/bin/tool.debug")
    );
    assert_eq!(
        strip::debug_file_path(Path::new("pkgstore/bin/tool"), &without_id),
        PathBuf::from("pkgstore/lib/debug/bin/tool.debug")
    );
}

fn binutils_available() -> bool {
    ["strip", "objcopy"].iter().all(|tool| {
        Command::new(tool)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Returns this test executable when it can be used as a strippable ELF
/// file, i.e. binutils is installed and the binary still has symbols.
fn strippable_test_binary() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") || !binutils_available() {
        eprintln!("skipping: binutils not available");
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let info = strip::inspect_file(&exe).ok()??;
    if !(info.has_symtab || info.has_debug_info) {
        eprintln!("skipping: test binary is already stripped");
        return None;
    }
    Some(exe)
}

fn size(path: &Path) -> u64 {
    fs::metadata(path).expect("metadata").len()
}

#[test]
fn staging_strip_splits_debug_info_and_honors_excludes() {
    let Some(exe) = strippable_test_binary() else {
        return;
    };
    let tmp = tempdir().expect("tempdir");
    let staging = tmp.path().join("staging");
    let debug = tmp.path().join("debug");
    let bin = staging.join("usrroot/usr/bin");
    let firmware = staging.join("usrroot/usr/lib/firmware");
    fs::create_dir_all(&bin).expect("mkdir");
    fs::create_dir_all(&firmware).expect("mkdir");

    let tool = bin.join("tool");
    let kept = firmware.join("blob");
    fs::copy(&exe, &tool).expect("copy");
    fs::copy(&exe, &kept).expect("copy");
    let original = size(&exe);

    // A read-only file must still be stripped and keep its mode.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o555))
            .expect("chmod");
    }

    let config = DebugConfig {
        strip: StripMode::Full,
        sub_package: true,
        exclude: vec!["usr/lib/firmware/**".to_string()]
    };
    let report = strip::strip_staging(&staging, &config, Some(&debug), true)
        .expect("strip");

    assert!(!report.skipped);
    assert_eq!(report.stripped, 1);
    assert_eq!(report.debug_files, 1);
    assert!(size(&tool) < original, "tool was not stripped");
    assert_eq!(size(&kept), original, "excluded file was stripped");

    let info = strip::inspect_file(&tool).expect("read").expect("ELF");
    assert!(!info.has_symtab && !info.has_debug_info);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&tool).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o555);
    }

    let debug_files: Vec<_> = walkdir::WalkDir::new(&debug)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(&debug).expect("prefix").to_path_buf())
        .collect();
    assert_eq!(debug_files.len(), 1, "{debug_files:?}");
    assert!(
        debug_files
            .iter()
            .all(|f| f.starts_with("usrroot/usr/lib/debug")),
        "{debug_files:?}"
    );
}

#[test]
fn debug_level_keeps_the_symbol_table() {
    let Some(exe) = strippable_test_binary() else {
        return;
    };
    let tmp = tempdir().expect("tempdir");
    let bin = tmp.path().join("pkgstore/bin");
    fs::create_dir_all(&bin).expect("mkdir");
    let tool = bin.join("tool");
    fs::copy(&exe, &tool).expect("copy");

    let config = DebugConfig {
        strip: StripMode::Debug,
        ..DebugConfig::default()
    };
    strip::strip_staging(tmp.path(), &config, None, true).expect("strip");

    let info = strip::inspect_file(&tool).expect("read").expect("ELF");
    assert!(!info.has_debug_info);
    let original = strip::inspect_file(&exe).expect("read").expect("ELF");
    assert_eq!(info.has_symtab, original.has_symtab);
}

#[test]
fn strip_none_leaves_files_untouched() {
    let Some(exe) = strippable_test_binary() else {
        return;
    };
    let tmp = tempdir().expect("tempdir");
    let bin = tmp.path().join("pkgstore/bin");
    fs::create_dir_all(&bin).expect("mkdir");
    let tool = bin.join("tool");
    fs::copy(&exe, &tool).expect("copy");

    let config = DebugConfig {
        strip: StripMode::None,
        ..DebugConfig::default()
    };
    let report =
        strip::strip_staging(tmp.path(), &config, None, true).expect("strip");
    assert_eq!(report.stripped, 0);
    assert_eq!(
        fs::read(&tool).expect("read"),
        fs::read(&exe).expect("read")
    );
}

fn read_manifest(archive_path: &Path) -> PooledZpaManifest {
    let file = fs::File::open(archive_path).expect("open archive");
    let decoder = ZstdDecoder::new(file).expect("zstd");
    let mut archive = Archive::new(decoder);
    for entry in archive.entries().expect("entries") {
        let mut entry = entry.expect("entry");
        if entry
            .path()
            .expect("path")
            .to_string_lossy()
            .ends_with("manifest.json")
        {
            let mut json = String::new();
            entry.read_to_string(&mut json).expect("read manifest");
            return serde_json::from_str(&json).expect("pooled manifest");
        }
    }
    panic!("manifest.json missing from {}", archive_path.display());
}

#[test]
fn build_ships_debug_info_in_a_separate_debug_archive() {
    let Some(exe) = strippable_test_binary() else {
        return;
    };
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let platform = get_platform().expect("platform");
    fs::copy(&exe, root.join("tool-bin")).expect("copy");
    let pkg_lua = root.join("striptest.pkg.lua");
    fs::write(
        &pkg_lua,
        format!(
            r#"
metadata({{{METADATA_HEAD}
    debug = {{ sub_package = true }},
}})

function package()
    zmkdir("${{pkgstore}}/bin")
    zcp("tool-bin", "${{pkgstore}}/bin/tool")
end
"#
        )
    )
    .expect("write pkg.lua");

    let output = root.join("output");
    fs::create_dir(&output).expect("mkdir");
    build::run(
        &pkg_lua,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        zoi_core::types::SignMode::Embed,
        Some(&output),
        Some("1.0.0"),
        None,
        true,
        "native",
        None,
        true,
        false,
        false
    )
    .expect("build");

    let main =
        read_manifest(&output.join(format!("striptest-1.0.0-{platform}.zpa")));
    let debug = read_manifest(
        &output.join(format!("striptest-1.0.0-{platform}.debug.zpa"))
    );

    // The main archive holds the stripped binary and no debug files.
    let main_files: Vec<_> = main
        .mappings
        .values()
        .flat_map(|m| m.scopes.values())
        .flat_map(|s| s.files.iter())
        .collect();
    assert!(!main_files.is_empty());
    assert!(main_files.iter().all(|f| !f.dest.contains("/lib/debug/")));
    let tool = main_files
        .iter()
        .find(|f| f.dest == "${pkgstore}/bin/tool")
        .expect("tool in main archive");
    let tool_size = main.pool.get(&tool.hash).expect("tool in pool").size;
    assert!(tool_size < size(&exe), "tool not stripped");

    // The debug archive holds only the `debug` mapping with the split-off
    // debug file, laid out where debuggers look for it.
    assert_eq!(debug.mappings.keys().collect::<Vec<_>>(), vec!["debug"]);
    let debug_files: Vec<_> = debug
        .mappings
        .get("debug")
        .expect("debug mapping")
        .scopes
        .values()
        .flat_map(|s| s.files.iter())
        .collect();
    assert!(!debug_files.is_empty());
    assert!(
        debug_files
            .iter()
            .all(|f| f.dest.starts_with("${pkgstore}/lib/debug/")),
        "{:?}",
        debug_files.iter().map(|f| &f.dest).collect::<Vec<_>>()
    );
    assert!(debug.pool.keys().all(|h| !main.pool.contains_key(h)));
}

#[test]
fn debug_sub_package_name_cannot_be_declared_twice() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let pkg_lua = root.join("striptest.pkg.lua");
    fs::write(
        &pkg_lua,
        format!(
            "metadata({{{METADATA_HEAD}
    sub_packages = {{ \"debug\" }},
    debug = {{ sub_package = true }},
}})
function package() end
"
        )
    )
    .expect("write pkg.lua");

    let platform = get_platform().expect("platform");
    let result = build::run(
        &pkg_lua,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        zoi_core::types::SignMode::Embed,
        Some(root.as_path()),
        Some("1.0.0"),
        None,
        true,
        "native",
        None,
        true,
        false,
        false
    );
    let error = result.expect_err("duplicate debug sub-package").to_string();
    assert!(error.contains("sub_packages already declares"), "{error}");
}

fn archive_pkg(sub_packages: Option<Vec<&str>>) -> zoi::pkg::types::Package {
    zoi::pkg::types::Package {
        name: "striptest".to_string(),
        sub_packages: sub_packages
            .map(|subs| subs.into_iter().map(String::from).collect()),
        ..Default::default()
    }
}

#[test]
fn source_builds_pick_the_archive_of_the_requested_sub_package() {
    use zoi_install::prebuilt::built_archive_path;

    let tmp = tempdir().expect("tempdir");
    let dir = tmp.path();
    let plat = "linux-amd64";
    let touch = |name: &str| fs::write(dir.join(name), "").expect("write");
    touch("striptest-1.0.0-linux-amd64.zpa");
    touch("striptest-1.0.0-linux-amd64.debug.zpa");

    // `pkg:debug` of a plain package must not install the main archive.
    let plain = archive_pkg(None);
    assert_eq!(
        built_archive_path(dir, &plain, Some("debug"), "1.0.0", plat),
        dir.join("striptest-1.0.0-linux-amd64.debug.zpa")
    );
    assert_eq!(
        built_archive_path(dir, &plain, None, "1.0.0", plat),
        dir.join("striptest-1.0.0-linux-amd64.zpa")
    );

    // A split package's main archive is `.main.zpa`.
    touch("striptest-1.0.0-linux-amd64.main.zpa");
    touch("striptest-1.0.0-linux-amd64.headers.zpa");
    let split = archive_pkg(Some(vec!["headers"]));
    assert_eq!(
        built_archive_path(dir, &split, None, "1.0.0", plat),
        dir.join("striptest-1.0.0-linux-amd64.main.zpa")
    );
    assert_eq!(
        built_archive_path(dir, &split, Some("headers"), "1.0.0", plat),
        dir.join("striptest-1.0.0-linux-amd64.headers.zpa")
    );
}
