//! Regression tests for how installation lays files down: pooled directory
//! records, modes on shared directories, reinstall replacing the version
//! directory, and dependent links.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use zoi_core::types::{
    self, InstallReason, MappedDir, MappedFile, PoolFileEntry,
    PooledZpaManifest, Scope, ScopeMapping, SubPackageMapping
};
use zoi_install::archive_trust;
use zoi_resolver::local;

mod common;

/// Writes a tar + zstd archive holding the given regular-file entries.
fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("create archive");
    let encoder = zstd::stream::write::Encoder::new(file, 0)
        .expect("zstd encoder")
        .auto_finish();
    let mut builder = tar::Builder::new(encoder);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *data)
            .expect("append entry");
    }
    builder.into_inner().expect("finish tar");
}

/// A minimal package definition the installer can parse.
fn pkg_lua(name: &str, version: &str) -> String {
    format!(
        r#"
metadata({{
    name = "{name}",
    repo = "core",
    version = "{version}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    types = {{ "source" }}
}})
"#
    )
}

/// Builds a package whose `package()` copies the listed files into
/// `${pkgstore}/share`.
fn build_package(
    root: &Path,
    name: &str,
    version: &str,
    files: &[&str],
    out: &str
) -> PathBuf {
    let platform = zoi_core::utils::get_platform().expect("platform");
    let mut prepare = String::new();
    let mut package = String::from("    zmkdir(\"${pkgstore}/share\")\n");
    for file in files {
        writeln!(prepare, "    cmd(\"printf '{file}' > {file}\")")
            .expect("writing to a String cannot fail");
        writeln!(
            package,
            "    zcp(\"{file}\", \"${{pkgstore}}/share/{file}\")"
        )
        .expect("writing to a String cannot fail");
    }
    let content = format!(
        "{}\nfunction prepare()\n{prepare}end\n\nfunction \
         package()\n{package}end\n",
        pkg_lua(name, version)
    );
    let src_dir = root.join(format!("src-{out}"));
    fs::create_dir_all(&src_dir).expect("src dir");
    let pkg_lua_path = src_dir.join(format!("{name}.pkg.lua"));
    fs::write(&pkg_lua_path, content).expect("write pkg.lua");

    let output_dir = root.join(out);
    fs::create_dir_all(&output_dir).expect("output dir");
    zoi_package::build::run(
        &pkg_lua_path,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        zoi_core::types::SignMode::Embed,
        Some(&output_dir),
        Some(version),
        None,
        true,
        "native",
        None,
        false,
        false,
        false
    )
    .expect("build should succeed");
    output_dir.join(format!("{name}-{version}-{platform}.zpa"))
}

fn install(archive: &Path, version: &str) -> Vec<String> {
    zoi_install::pkg_install::run(
        archive,
        Some(Scope::User),
        "local",
        Some(version),
        true,
        None,
        false,
        None
    )
    .expect("install should succeed")
    .0
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).expect("metadata").permissions().mode() & 0o7777
}

#[cfg(unix)]
#[test]
fn pool_records_only_owned_directories_outside_the_store() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir().expect("tempdir");
    let staging = tmp.path().join("staging");
    let pool_dir = tmp.path().join("pool");
    fs::create_dir_all(&pool_dir).expect("pool dir");

    let bin_dir = staging.join("usrroot/usr/bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(bin_dir.join("tool"), "tool").expect("tool");
    for dir in ["usrroot/usr", "usrroot/usr/bin"] {
        fs::set_permissions(
            staging.join(dir),
            fs::Permissions::from_mode(0o755)
        )
        .expect("chmod");
    }
    fs::create_dir_all(staging.join("usrroot/etc/empty")).expect("empty dir");
    let store_dir = staging.join("pkgstore/share");
    fs::create_dir_all(&store_dir).expect("store dir");
    fs::write(store_dir.join("data"), "data").expect("data");

    let mut pool = BTreeMap::new();
    let mut mapping = ScopeMapping::default();
    zoi_package::pool::pool_files(
        &staging,
        &pool_dir,
        &mut pool,
        &mut mapping,
        false
    )
    .expect("pool files");

    let dirs: Vec<&str> =
        mapping.dirs.iter().map(|d| d.path.as_str()).collect();
    assert!(dirs.contains(&"${usrroot}/etc/empty"), "dirs: {dirs:?}");
    assert!(dirs.contains(&"${pkgstore}/share"), "dirs: {dirs:?}");
    assert!(!dirs.contains(&"${usrroot}/usr"), "dirs: {dirs:?}");
    assert!(!dirs.contains(&"${usrroot}/usr/bin"), "dirs: {dirs:?}");
}

#[cfg(unix)]
#[test]
fn pooled_install_leaves_existing_dirs_alone_and_masks_modes() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    // A directory the user already has, with its own mode.
    let existing = root.join("perm-test/existing");
    fs::create_dir_all(&existing).expect("existing dir");
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o755))
            .expect("chmod");
    }

    let content = b"#!/bin/sh\necho tool\n";
    let hash = format!(
        "sha256-{}",
        archive_trust::sha256_hex(content).expect("hash")
    );
    let mut pool = BTreeMap::new();
    pool.insert(
        hash.clone(),
        PoolFileEntry {
            size: content.len() as u64
        }
    );
    let scope_mapping = ScopeMapping {
        files: vec![MappedFile {
            dest: "${usrhome}/perm-test/existing/tool".to_string(),
            hash: hash.clone(),
            mode: 0o4755,
            owner: None,
            group: None
        }],
        symlinks: Vec::new(),
        dirs: vec![
            MappedDir {
                path: "${usrhome}/perm-test/existing".to_string(),
                mode: Some(0o700),
                owner: None,
                group: None
            },
            MappedDir {
                path: "${usrhome}/perm-test/fresh".to_string(),
                mode: Some(0o700),
                owner: None,
                group: None
            },
        ]
    };
    let mut scopes = BTreeMap::new();
    scopes.insert(Scope::User, scope_mapping);
    let mut mappings = BTreeMap::new();
    mappings.insert(String::new(), SubPackageMapping { scopes });
    let manifest = PooledZpaManifest {
        version: "2".to_string(),
        pool,
        mappings
    };
    let manifest_json = serde_json::to_vec(&manifest).expect("manifest json");
    let definition = pkg_lua("perm-test", "1.0.0");
    let pool_entry = format!("pool/{hash}");

    let archive = root.join("perm-test.zpa");
    write_archive(
        &archive,
        &[
            ("perm-test.pkg.lua", definition.as_bytes()),
            ("manifest.json", manifest_json.as_slice()),
            (pool_entry.as_str(), content.as_slice())
        ]
    );

    install(&archive, "1.0.0");

    assert_eq!(
        mode_of(&existing),
        0o755,
        "an existing directory must keep its mode"
    );
    assert_eq!(
        mode_of(&root.join("perm-test/fresh")),
        0o700,
        "a directory the install creates gets the recorded mode"
    );
    assert_eq!(
        mode_of(&existing.join("tool")),
        0o755,
        "setuid/setgid/sticky bits must be dropped"
    );
}

#[test]
fn reinstall_replaces_the_version_directory() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let first = build_package(
        &root,
        "replace-test",
        "1.0.0",
        &["a.txt", "b.txt"],
        "o1"
    );
    let second =
        build_package(&root, "replace-test", "1.0.0", &["a.txt"], "o2");

    install(&first, "1.0.0");
    let version_dir = local::get_package_version_dir(
        Scope::User,
        "local",
        "core",
        "replace-test",
        "1.0.0"
    )
    .expect("version dir");
    assert!(version_dir.join("share/b.txt").exists());
    // Stand-in for the record the installer writes after `pkg_install::run`.
    fs::write(version_dir.join("package.pkg.lua"), "-- recorded")
        .expect("record");

    install(&second, "1.0.0");
    assert!(version_dir.join("share/a.txt").exists());
    assert!(
        !version_dir.join("share/b.txt").exists(),
        "files the new build no longer ships must not linger"
    );
    assert_eq!(
        fs::read_to_string(version_dir.join("package.pkg.lua"))
            .expect("record carried over"),
        "-- recorded"
    );
    let package_dir = version_dir.parent().expect("package dir");
    let leftovers: Vec<_> = fs::read_dir(package_dir)
        .expect("read package dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "leftover staging dirs: {leftovers:?}");
}

#[test]
fn dependent_id_matches_the_shared_source_format() {
    assert_eq!(
        zoi_install::installer::dependent_id(
            "zoidberg", "core", "foo", None, "1.0.0"
        ),
        "#zoidberg@core/foo@1.0.0"
    );
    assert_eq!(
        zoi_install::installer::dependent_id(
            "zoidberg",
            "core",
            "foo",
            Some("dev"),
            "1.0.0"
        ),
        local::package_source_string(
            "zoidberg",
            "core",
            "foo",
            Some("dev"),
            "1.0.0"
        )
    );
}

/// Writes an install manifest for `name@version` in the user scope.
fn record_installed(name: &str, version: &str) {
    let manifest = types::InstallManifest {
        name: name.to_string(),
        version: version.to_string(),
        epoch: 0,
        revision: String::new(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "source".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: "test".to_string(),
        reason: InstallReason::Direct,
        scope: Scope::User,
        bins: None,
        alternatives: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: Vec::new(),
        dependencies_v2: None,
        chosen_options: Vec::new(),
        chosen_optionals: Vec::new(),
        install_method: None,
        platform: zoi_core::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: Vec::new(),
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    };
    local::write_manifest(&manifest).expect("write manifest");
}

#[test]
fn every_parent_is_recorded_as_a_dependent() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    // `shared` is already installed (so it would be pruned from a new
    // install plan) and is required by two parents.
    record_installed("shared", "1.0.0");

    for parent in ["parent-a", "parent-b"] {
        let node = zoi_install::resolver::InstallNode {
            pkg: types::Package {
                name: parent.to_string(),
                repo: "core".to_string(),
                scope: Scope::User,
                ..Default::default()
            },
            version: "2.0.0".to_string(),
            revision: "1".to_string(),
            sub_package: None,
            repo_type: "source".to_string(),
            description: String::new(),
            reason: InstallReason::Direct,
            source: String::new(),
            registry_handle: "local".to_string(),
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
            dependencies: vec![
                format!(
                    "zoi:{}",
                    local::package_source_string(
                        "local", "core", "shared", None, "1.0.0"
                    )
                ),
                "native:some-lib".to_string(),
            ],
            git_sha: None
        };
        zoi_install::installer::link_dependents(&node)
            .expect("link dependents");
    }

    let shared_dir =
        local::get_package_dir(Scope::User, "local", "core", "shared")
            .expect("package dir");
    let mut dependents =
        local::get_dependents(&shared_dir).expect("dependents");
    dependents.sort();
    assert_eq!(
        dependents,
        vec![
            "#local@core/parent-a@2.0.0".to_string(),
            "#local@core/parent-b@2.0.0".to_string(),
        ]
    );
}
