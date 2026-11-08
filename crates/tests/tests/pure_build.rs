//! Integration tests for `zoi package build --pure` dependency handling.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::tempdir;
use zoi::cmd::package::build::{
    BuildCommand, SignModeArg, collect_build_dependencies, run
};
use zoi::pkg::dependencies::parse_dependency_string;
use zoi::pkg::install::dep_install::install_dependency;
use zoi::pkg::package::build::non_zoi_dependencies;
use zoi::pkg::sysroot;
use zoi::pkg::types::Scope;

mod common;

const METADATA: &str = r#"
metadata({
    name = "purepkg",
    repo = "core",
    version = "1.0.0",
    description = "pure build test",
    license = "MIT",
    maintainer = { name = "test", email = "test@example.com" },
    platforms = { "linux-amd64", "linux-arm64" },
    types = { "source" },
})
"#;

fn lua_list(deps: &[&str]) -> String {
    deps.iter()
        .map(|d| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_raw_package(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("purepkg.pkg.lua");
    fs::write(
        &path,
        format!("{METADATA}\n{body}\nfunction package() end\n")
    )
    .expect("failed to write package file");
    path
}

fn write_package(dir: &Path, build_deps: &[&str]) -> PathBuf {
    write_raw_package(
        dir,
        &format!(
            r"
dependencies({{
    build = {{
        types = {{
            source = {{
                required = {{ {} }},
            }},
        }},
    }},
}})
",
            lua_list(build_deps)
        )
    )
}

fn pure_args(package_file: PathBuf) -> BuildCommand {
    BuildCommand {
        package_file,
        r#type: None,
        platform: vec!["linux-amd64".to_string()],
        sub: None,
        sign: None,
        sign_mode: SignModeArg::Embed,
        test: false,
        output_dir: None,
        install_deps: false,
        version_override: None,
        method: "native".to_string(),
        image: None,
        fakeroot: false,
        pure: true,
        root_package: "@core/base:dev".to_string(),
        no_zsa: false
    }
}

/// Asserts the pure build left no global state behind.
fn assert_pure_state_cleared() {
    assert!(sysroot::get_sysroot().is_none(), "sysroot was not cleared");
    assert!(
        !sysroot::host_managers_denied(),
        "host manager restriction was not cleared"
    );
}

#[test]
fn non_zoi_dependencies_keeps_only_foreign_managers() {
    let deps = vec![
        "zoi:@core/gcc".to_string(),
        "@core/make".to_string(),
        "pacman:gcc".to_string(),
        "native:make".to_string(),
        "apt:build-essential".to_string(),
        "pacman:gcc".to_string(),
    ];

    let foreign = non_zoi_dependencies(&deps).expect("parse failed");

    assert_eq!(
        foreign,
        vec![
            "pacman:gcc".to_string(),
            "native:make".to_string(),
            "apt:build-essential".to_string(),
        ]
    );
}

#[test]
fn non_zoi_dependencies_accepts_pure_zoi_set() {
    let deps = vec![
        "zoi:@core/gcc".to_string(),
        "zoi:@core/base:dev".to_string(),
        "@core/pkgconf>=2.0".to_string(),
    ];

    assert_eq!(
        non_zoi_dependencies(&deps).expect("parse failed"),
        [] as [std::string::String; 0]
    );
}

#[test]
fn pure_build_rejects_host_manager_dependencies() {
    let _ctx = common::TestContextGuard::acquire();
    sysroot::clear_sysroot();
    let tmp = tempdir().expect("failed to create temp dir");
    let pkg = write_package(
        tmp.path(),
        &["zoi:@core/gcc", "pacman:make", "pacman:patch"]
    );

    let err = run(pure_args(pkg)).expect_err("pure build should be rejected");
    let msg = err.to_string();

    assert!(msg.contains("--pure"), "unexpected error: {msg}");
    assert!(msg.contains("pacman:make"), "unexpected error: {msg}");
    assert!(msg.contains("pacman:patch"), "unexpected error: {msg}");
    assert!(!msg.contains("zoi:@core/gcc"), "unexpected error: {msg}");
    assert_pure_state_cleared();
}

#[test]
fn pure_build_rejects_host_manager_test_only_dependencies() {
    let _ctx = common::TestContextGuard::acquire();
    sysroot::clear_sysroot();
    let tmp = tempdir().expect("failed to create temp dir");
    // No build dependencies at all, only test dependencies.
    let pkg = write_raw_package(
        tmp.path(),
        r#"
dependencies({
    test = {
        required = { "pacman:check" },
    },
})
"#
    );
    let mut args = pure_args(pkg);
    args.test = true;

    let err = run(args).expect_err("pure build should be rejected");
    let msg = err.to_string();

    assert!(msg.contains("--pure"), "unexpected error: {msg}");
    assert!(msg.contains("pacman:check"), "unexpected error: {msg}");
    assert_pure_state_cleared();
}

#[test]
fn pure_build_resolves_dependencies_for_the_zoios_target() {
    let _ctx = common::TestContextGuard::acquire();
    sysroot::clear_sysroot();
    let tmp = tempdir().expect("failed to create temp dir");
    // Zoi dependencies on ZoiOS, host-manager dependencies elsewhere. The
    // pure sysroot is ZoiOS, so the check must accept this package on any
    // build host.
    let pkg = write_raw_package(
        tmp.path(),
        r#"
local deps = { "pacman:make" }
if SYSTEM.DISTRO == "zoios" then
    deps = { "zoi:@core/make" }
end

dependencies({
    build = {
        types = {
            source = {
                required = deps,
            },
        },
    },
})
"#
    );
    let mut args = pure_args(pkg);
    // A host-manager root package fails fast and offline, after the
    // dependency check has passed.
    args.root_package = "pacman:base-devel".to_string();

    let err = run(args).expect_err("host-manager root package must fail");
    let msg = err.to_string();

    assert!(
        !msg.contains("--pure builds can only use Zoi build dependencies"),
        "ZoiOS-target dependencies were rejected: {msg}"
    );
    assert!(
        msg.contains("not allowed in an isolated (--pure) build"),
        "unexpected error: {msg}"
    );
    assert_pure_state_cleared();
}

#[test]
fn host_managers_denied_blocks_nested_dependency_installs() {
    let _ctx = common::TestContextGuard::acquire();
    sysroot::set_host_managers_denied(true);

    let dep = parse_dependency_string("apt:build-essential").expect("parse");
    let result = install_dependency(
        &dep,
        "zoi:@core/tool",
        Scope::System,
        true,
        true,
        &Mutex::new(HashSet::new()),
        &mut Vec::new(),
        None
    );
    sysroot::set_host_managers_denied(false);

    let msg = result.expect_err("host manager must be denied").to_string();
    assert!(
        msg.contains("apt:build-essential"),
        "unexpected error: {msg}"
    );
    assert!(msg.contains("zoi:@core/tool"), "unexpected error: {msg}");
    assert!(
        msg.contains("not allowed in an isolated (--pure) build"),
        "unexpected error: {msg}"
    );
}

#[test]
fn build_dependencies_are_grouped_per_platform() {
    let _ctx = common::TestContextGuard::acquire();
    sysroot::clear_sysroot();
    let tmp = tempdir().expect("failed to create temp dir");
    let pkg = write_package(tmp.path(), &["zoi:@core/make", "zoi:@core/make"]);
    let mut args = pure_args(pkg);
    args.platform = vec!["linux-amd64".to_string(), "linux-arm64".to_string()];

    let sets = collect_build_dependencies(&args, false).expect("collect");

    // One set per platform, so each platform's requirements are installed
    // with their own de-duplication state; duplicates within a platform are
    // dropped.
    assert_eq!(
        sets,
        vec![
            vec!["zoi:@core/make".to_string()],
            vec!["zoi:@core/make".to_string()],
        ]
    );
}

#[test]
fn pure_build_still_rejects_docker_method() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let pkg = write_package(tmp.path(), &["pacman:make"]);
    let mut args = pure_args(pkg);
    args.method = "docker".to_string();

    let err = run(args).expect_err("docker + pure should be rejected");
    assert!(err.to_string().contains("--method docker"));
}
