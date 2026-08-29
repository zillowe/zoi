//! Integration tests for `zoi package verify` functionality.

use std::fs;

use tempfile::tempdir;
use zoi::pkg::install::verify;
use zoi::pkg::{local, types};

mod common;

/// Builds a simple one-file package and returns the archive path.
fn build_package(
    root: &std::path::Path,
    name: &str,
    version: &str,
    content: &str
) -> std::path::PathBuf {
    let platform = zoi::utils::get_platform().expect("unwrap failed");
    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{name}",
    repo = "core",
    version = "{version}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    types = {{ "source" }}
}})

function prepare()
    cmd("printf '{content}' > test.txt")
end

function package()
    zmkdir("${{pkgstore}}/share")
    zcp("test.txt", "${{pkgstore}}/share/test.txt")
end
"#
    );
    let pkg_lua_path = root.join(format!("{name}.pkg.lua"));
    fs::write(&pkg_lua_path, pkg_lua_content).expect("unwrap failed");

    let output_dir = root.join("output");
    fs::create_dir_all(&output_dir).expect("unwrap failed");

    zoi::pkg::package::build::run(
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
    .expect("Build should succeed");

    output_dir.join(format!("{name}-{version}-{platform}.zpa"))
}

#[test]
fn test_verify_installed_detects_modification() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let archive =
        build_package(&root, "verify-test", "1.0.0", "original-content");

    // Install the built archive.
    let (installed_files, digests) = zoi::pkg::install::pkg_install::run(
        &archive,
        Some(types::Scope::User),
        "local",
        Some("1.0.0"),
        true,
        None,
        true,
        None
    )
    .expect("Install should succeed");

    assert!(!digests.is_empty(), "pooled installs must record digests");
    assert!(installed_files.iter().any(|f| f.contains("test.txt")));

    // pkg_install::run only extracts files; the orchestrator normally writes
    // the manifest.yaml. Write one manually so verify_installed can find it.
    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        "verify-test",
        "1.0.0"
    )
    .expect("unwrap failed");
    let manifest = types::InstallManifest {
        name: "verify-test".to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: String::new(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "source".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: "test".to_string(),
        reason: types::InstallReason::Dependency {
            parent: "root".to_string()
        },
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: Vec::new(),
        dependencies_v2: None,
        chosen_options: Vec::new(),
        chosen_optionals: Vec::new(),
        install_method: None,
        platform: zoi::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: installed_files.clone(),
        file_digests: if digests.is_empty() {
            None
        } else {
            Some(digests.clone())
        },
        installed_size: None,
        sandbox: None,
        completions: None
    };
    let manifest_path = version_dir.join("manifest.yaml");
    fs::write(
        &manifest_path,
        serde_yaml::to_string(&manifest).expect("yaml")
    )
    .expect("write manifest");
    assert!(
        manifest.file_digests.is_some(),
        "manifest should carry file digests"
    );

    let statuses = verify::verify_installed(&manifest).expect("unwrap failed");
    assert!(!statuses.is_empty());
    assert!(
        statuses
            .iter()
            .all(|s| s.status == verify::VerifyStatus::Ok)
    );

    // Tamper with the installed file.
    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )
    .expect("unwrap failed");
    let target = version_dir.join("share/test.txt");
    fs::write(&target, "tampered-content").expect("unwrap failed");

    let statuses = verify::verify_installed(&manifest).expect("unwrap failed");
    assert!(
        statuses
            .iter()
            .any(|s| s.status == verify::VerifyStatus::Modified)
    );

    // Delete it entirely.
    fs::remove_file(&target).expect("unwrap failed");
    let statuses = verify::verify_installed(&manifest).expect("unwrap failed");
    assert!(
        statuses
            .iter()
            .any(|s| s.status == verify::VerifyStatus::Missing)
    );
}

#[test]
fn test_verify_archive_passes_on_fresh_build() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");

    let archive =
        build_package(tmp.path(), "archive-verify-test", "1.0.0", "hello");

    let report = verify::verify_archive(&archive).expect("unwrap failed");
    assert!(report.ok, "issues: {:?}", report.issues);
    assert!(report.checked > 0);
}
