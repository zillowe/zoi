//! Regression tests for symlinks captured into `.zsa` source bundles and for
//! picking the bundled package definition.
//!
//! Upstream bootstrap scripts (gnulib's, for instance) create absolute links
//! into the temporary fetch directory. Stored verbatim they dangle once the
//! bundle is extracted elsewhere, which broke `configure` with "cannot find
//! required auxiliary files".

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use zoi_package::bundle::{relocate_symlink_target, select_bundle_pkg_lua};

#[test]
fn absolute_link_inside_root_becomes_relative() {
    let root = Path::new("/tmp/zoi-bundle-fetch-XXXX");
    let relocated = relocate_symlink_target(
        root,
        Path::new("libtool/build-aux/config.guess"),
        &root.join("libtool/gnulib/build-aux/config.guess")
    );
    assert_eq!(
        relocated,
        Some(PathBuf::from("../gnulib/build-aux/config.guess"))
    );
}

#[test]
fn link_at_root_and_target_in_same_directory() {
    let root = Path::new("/tmp/fetch");
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("link"),
            Path::new("/tmp/fetch/a/b")
        ),
        Some(PathBuf::from("a/b"))
    );
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("dir/link"),
            Path::new("/tmp/fetch/dir/target")
        ),
        Some(PathBuf::from("target"))
    );
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("dir/link"),
            Path::new("/tmp/fetch/./dir/../dir/target")
        ),
        Some(PathBuf::from("target"))
    );
}

#[test]
fn relative_and_outside_targets_are_left_alone() {
    let root = Path::new("/tmp/fetch");
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("link"),
            Path::new("../elsewhere")
        ),
        None
    );
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("link"),
            Path::new("/etc/passwd")
        ),
        None
    );
    // A sibling whose name merely starts with the root's name is outside.
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("link"),
            Path::new("/tmp/fetch-other/file")
        ),
        None
    );
    // `..` cannot be used to climb out of the root either.
    assert_eq!(
        relocate_symlink_target(
            root,
            Path::new("link"),
            Path::new("/tmp/fetch/../secret")
        ),
        None
    );
}

#[cfg(unix)]
#[test]
fn relocated_link_resolves_after_moving_the_tree() {
    let tmp = tempdir().expect("tempdir");
    let fetch = tmp.path().join("fetch");
    let real = fetch.join("libtool/gnulib/build-aux");
    fs::create_dir_all(&real).expect("dirs");
    fs::write(real.join("config.guess"), "#!/bin/sh\n").expect("file");
    fs::create_dir_all(fetch.join("libtool/build-aux")).expect("dirs");

    let link_rel = Path::new("libtool/build-aux/config.guess");
    let relocated =
        relocate_symlink_target(&fetch, link_rel, &real.join("config.guess"))
            .expect("target inside the fetch dir is relocated");

    // Recreate the link with the stored target in a different location, as
    // extracting the bundle would.
    let moved = tmp.path().join("extracted");
    fs::create_dir_all(moved.join("libtool/gnulib/build-aux")).expect("dirs");
    fs::create_dir_all(moved.join("libtool/build-aux")).expect("dirs");
    fs::write(
        moved.join("libtool/gnulib/build-aux/config.guess"),
        "#!/bin/sh\n"
    )
    .expect("file");
    std::os::unix::fs::symlink(&relocated, moved.join(link_rel))
        .expect("symlink");
    assert_eq!(
        fs::read_to_string(moved.join(link_rel)).expect("link resolves"),
        "#!/bin/sh\n"
    );
}

#[test]
fn signed_bundle_definition_is_preferred() {
    let tmp = tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("a.pkg.lua"), "-- a").expect("a");
    fs::write(dir.join("b.pkg.lua"), "-- b").expect("b");
    fs::write(dir.join("b.pkg.lua.sig"), "sig").expect("sig");
    fs::create_dir_all(dir.join("nested")).expect("nested");
    fs::write(dir.join("nested/0.pkg.lua"), "-- nested").expect("nested");

    assert_eq!(
        select_bundle_pkg_lua(dir).expect("select"),
        Some(dir.join("b.pkg.lua"))
    );

    fs::remove_file(dir.join("b.pkg.lua.sig")).expect("remove sig");
    assert_eq!(
        select_bundle_pkg_lua(dir).expect("select"),
        Some(dir.join("a.pkg.lua")),
        "unsigned bundles resolve by name order, top level only"
    );
}
