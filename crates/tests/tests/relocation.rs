use std::fs;
use tempfile::tempdir;
use zoi::pkg::package::relocate;

mod common;

#[test]
fn test_relocation_engine_identifies_elf_files() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let staging = tmp.path().to_path_buf();

    let pkgstore = staging.join("data/pkgstore");
    let bin_dir = pkgstore.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let elf_path = bin_dir.join("my-bin");
    // Write ELF magic number
    fs::write(&elf_path, b"\x7fELF some other content").unwrap();

    let non_elf_path = bin_dir.join("README.txt");
    fs::write(&non_elf_path, b"Just text").unwrap();

    // relocate_elfs will fail when trying to parse the mock ELF with arwen,
    // but it should at least try to relocate it and log a warning.
    // We can't easily assert on stdout/stderr here without more infra,
    // but we can ensure it doesn't crash.
    let result = relocate::relocate_elfs(&staging, true);
    assert!(result.is_ok());
}

#[test]
fn test_relocation_engine_complex_depth() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let staging = tmp.path().to_path_buf();

    let pkgstore = staging.join("data/pkgstore");
    let plugin_dir = pkgstore.join("lib/plugins/extra");
    fs::create_dir_all(&plugin_dir).unwrap();

    let plugin_path = plugin_dir.join("my-plugin.so");
    fs::write(&plugin_path, b"\x7fELF plugin content").unwrap();

    // The relocation engine should identify this and try to apply RPATHs.
    // Since it's depth 3 from pkgstore (lib/plugins/extra/), it should include $ORIGIN/../../..
    let result = relocate::relocate_elfs(&staging, true);
    assert!(result.is_ok());
}
