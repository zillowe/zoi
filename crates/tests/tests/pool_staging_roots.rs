//! The staging roots (`pkgstore`, `usrroot`, `usrhome`, `createpkgdir`) are
//! destinations, never directories of the package itself.
//!
//! Zoi 1.29.12 recorded the top-level `usrroot` staging directory as a
//! `${pkgstore}/usrroot` directory, so every archive listed it first in its
//! `.files` sidecar and installs created an empty `usrroot` directory in the
//! package store.

use std::collections::BTreeMap;
use std::fs;

use tempfile::tempdir;
use zoi_core::types::ScopeMapping;
use zoi_package::pool::pool_files;

#[test]
fn staging_roots_are_not_mapped_as_package_directories() {
    let tmp = tempdir().expect("tempdir");
    let staging = tmp.path().join("staging");
    let pool_dir = tmp.path().join("pool");
    fs::create_dir_all(&pool_dir).expect("mkdir pool");

    fs::create_dir_all(staging.join("usrroot/usr/bin")).expect("mkdir");
    fs::write(staging.join("usrroot/usr/bin/tool"), "tool").expect("write");
    fs::create_dir_all(staging.join("pkgstore/share")).expect("mkdir");
    fs::write(staging.join("pkgstore/share/data"), "data").expect("write");
    for root in ["usrhome", "createpkgdir"] {
        fs::create_dir_all(staging.join(root)).expect("mkdir");
    }

    let mut pool = BTreeMap::new();
    let mut mapping = ScopeMapping::default();
    pool_files(&staging, &pool_dir, &mut pool, &mut mapping, true)
        .expect("pool files");

    let dirs: Vec<&str> =
        mapping.dirs.iter().map(|d| d.path.as_str()).collect();
    for root in ["pkgstore", "usrroot", "usrhome", "createpkgdir"] {
        assert!(
            !dirs.contains(&format!("${{pkgstore}}/{root}").as_str()),
            "staging root '{root}' mapped as a package directory: {dirs:?}"
        );
    }
    // Shared system directories that only hold files are not owned either.
    assert!(!dirs.contains(&"${usrroot}/usr"), "{dirs:?}");
    assert!(!dirs.contains(&"${usrroot}/usr/bin"), "{dirs:?}");

    let files: Vec<&str> =
        mapping.files.iter().map(|f| f.dest.as_str()).collect();
    assert!(files.contains(&"${usrroot}/usr/bin/tool"), "{files:?}");
    assert!(files.contains(&"${pkgstore}/share/data"), "{files:?}");
}
