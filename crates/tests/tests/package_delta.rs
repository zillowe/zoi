//! Integration tests for `.zdelta` generation and application on packages.

use std::fs;

use tempfile::tempdir;
use zoi::pkg::install::verify;
use zoi::pkg::{local, types};

mod common;

/// Builds a package with the given file content and returns the archive.
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
    cmd("printf '{content}' > data.txt")
end

function package()
    zmkdir("${{pkgstore}}/share")
    zcp("data.txt", "${{pkgstore}}/share/data.txt")
end
"#
    );
    let pkg_lua_path = root.join(format!("{name}-{version}.pkg.lua"));
    fs::write(&pkg_lua_path, pkg_lua_content).expect("unwrap failed");

    let output_dir = root.join(format!("out-{version}"));
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
fn test_zpa_delta_roundtrip() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    // v1 and v2 share most content but differ in one file.
    let v1 =
        build_package(&root, "delta-test", "1.0.0", "shared-common-content-v1");
    let v2 =
        build_package(&root, "delta-test", "1.1.0", "shared-common-content-v2");

    let delta_path = root.join("delta-test.zdelta");
    zoi::pkg::delta::create_zpa_delta(&v1, &v2, &delta_path, None)
        .expect("delta creation should succeed");
    assert!(delta_path.exists());

    // Rebuild v2 from the cached v1 base plus the delta.
    let rebuilt = root.join("rebuilt.zpa");
    zoi::pkg::delta::apply_zpa_delta(&v1, &delta_path, &rebuilt)
        .expect("delta application should succeed");

    // The rebuilt archive must be structurally sound...
    let report = verify::verify_archive(&rebuilt).expect("unwrap failed");
    assert!(report.ok, "issues: {:?}", report.issues);

    // ...and installing it must produce exactly the v2 file content.
    let (installed_files, _digests) = zoi::pkg::install::pkg_install::run(
        &rebuilt,
        Some(types::Scope::User),
        "local",
        Some("1.1.0"),
        true,
        None,
        true,
        None
    )
    .expect("Install of rebuilt archive should succeed");

    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        "delta-test",
        "1.1.0"
    )
    .expect("unwrap failed");
    let installed_file = installed_files
        .iter()
        .find(|f| f.contains("data.txt"))
        .expect("installed files should reference data.txt");
    let expanded = zoi_core::utils::expand_placeholders(
        installed_file,
        &version_dir,
        types::Scope::User
    )
    .expect("unwrap failed");
    let content = fs::read_to_string(expanded).expect("unwrap failed");
    assert_eq!(content, "shared-common-content-v2");
}
