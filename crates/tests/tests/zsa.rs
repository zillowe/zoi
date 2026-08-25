//! Integration tests for Zoi Storage Architecture (ZSA) and scoping.

use std::fs;

use tempfile::tempdir;
use zoi::Scope;

mod common;

#[test]
fn test_zsa_bundle_build_install() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_dir = root.join("my-pkg");
    fs::create_dir_all(&pkg_dir).expect("unwrap failed");

    let pkg_lua = pkg_dir.join("my-pkg.pkg.lua");
    let asset_file = pkg_dir.join("hello.txt");
    fs::write(&asset_file, "hello from asset").expect("unwrap failed");

    let lua_code = r#"
metadata({
    name = "my-pkg",
    repo = "test",
    version = "1.0.0",
    description = "Test package",
    maintainer = { name = "test", email = "test@example.com" },
    types = { "source" },
    bins = { "hello-bin" }
})

function package()
    zcp("${pkgluadir}/hello.txt", "${pkgstore}/bin/hello-bin")
end
"#;
    fs::write(&pkg_lua, lua_code).expect("unwrap failed");

    // Bundle
    zoi::bundle_package(&pkg_lua, Some(&root), None, None, None)
        .expect("bundling failed");
    let zsa_path = root.join("my-pkg-1.0.0.zsa");
    assert!(zsa_path.exists(), ".zsa bundle should exist");

    // Build from .zsa
    let build_options = zoi::BuildOptions {
        build_type: Some("source"),
        output_dir: Some(root.clone()),
        ..Default::default()
    };
    zoi::build_with_options(&zsa_path, &build_options)
        .expect("build from .zsa failed");

    let platform = zoi::utils::get_platform().expect("unwrap failed");
    let zpa_path = root.join(format!("my-pkg-1.0.0-{platform}.zpa"));
    assert!(
        zpa_path.exists(),
        ".zpa archive should exist after build from .zsa"
    );

    // Install from .zsa (end-to-end)
    let install_options = zoi::SourceInstallOptions {
        scope_override: Some(Scope::User),
        yes: true,
        ..Default::default()
    };

    // We'll use a clean sysroot to verify installation
    let sysroot = root.join("sysroot");
    fs::create_dir_all(&sysroot).expect("unwrap failed");
    common::TestContextGuard::set_sysroot(sysroot.clone());

    zoi::install_sources(
        &[zsa_path.to_string_lossy().to_string()],
        &install_options
    )
    .expect("install from .zsa failed");

    // Verify installation
    // Path should be
    // sysroot/home/.zoi/pkgs/store/8f...-my-pkg/1.0.0/bin/hello-bin
    let mut found = false;
    for entry in walkdir::WalkDir::new(&sysroot) {
        let entry = entry.expect("unwrap failed");
        let path = entry.path();
        // Skip shims which are in .../bin/ and are copies of zoi binary
        if path.to_string_lossy().contains("store")
            && entry.file_name().to_string_lossy() == "hello-bin"
        {
            let content = fs::read_to_string(path).expect("unwrap failed");
            if content == "hello from asset" {
                found = true;
                break;
            }
        }
    }
    assert!(
        found,
        "installed file 'hello-bin' not found in store or has wrong content"
    );
}

fn write_simple_pkg(root: &std::path::Path) -> std::path::PathBuf {
    let pkg_dir = root.join("my-pkg");
    fs::create_dir_all(&pkg_dir).expect("unwrap failed");

    let pkg_lua = pkg_dir.join("my-pkg.pkg.lua");
    let asset_file = pkg_dir.join("hello.txt");
    fs::write(&asset_file, "hello from asset").expect("unwrap failed");

    let lua_code = r#"
metadata({
    name = "my-pkg",
    repo = "test",
    version = "1.0.0",
    description = "Test package",
    maintainer = { name = "test", email = "test@example.com" },
    types = { "source" },
    bins = { "hello-bin" }
})

function package()
    zcp("${pkgluadir}/hello.txt", "${pkgstore}/bin/hello-bin")
end
"#;
    fs::write(&pkg_lua, lua_code).expect("unwrap failed");
    pkg_lua
}

#[test]
fn test_build_command_bundles_zsa_then_builds_from_it() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_lua = write_simple_pkg(&root);

    let args = zoi::cmd::package::build::BuildCommand {
        package_file: pkg_lua.clone(),
        r#type: Some("source".to_string()),
        platform: vec!["current".to_string()],
        sub: None,
        sign: None,
        test: false,
        output_dir: Some(root.clone()),
        install_deps: false,
        version_override: None,
        method: "native".to_string(),
        image: None,
        fakeroot: false,
        pure: false,
        root_package: "@core/base:dev".to_string(),
        no_zsa: false
    };

    zoi::cmd::package::build::run(args).expect("two-stage build failed");

    let platform = zoi::utils::get_platform().expect("unwrap failed");
    let zpa_path = root.join(format!("my-pkg-1.0.0-{platform}.zpa"));
    assert!(
        zpa_path.exists(),
        ".zpa archive should be built into the output dir"
    );

    // The intermediate bundle is temporary and must not leak into any of the
    // user-visible directories.
    assert!(
        !root.join("my-pkg-1.0.0.zsa").exists(),
        ".zsa must stay in a temporary location"
    );
    assert!(
        !pkg_lua
            .parent()
            .expect("unwrap failed")
            .join("my-pkg-1.0.0.zsa")
            .exists(),
        ".zsa must not be written next to the .pkg.lua"
    );
}

#[test]
fn test_build_command_no_zsa_skips_bundling() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_lua = write_simple_pkg(&root);

    let args = zoi::cmd::package::build::BuildCommand {
        package_file: pkg_lua.clone(),
        r#type: Some("source".to_string()),
        platform: vec!["current".to_string()],
        sub: None,
        sign: None,
        test: false,
        output_dir: Some(root.clone()),
        install_deps: false,
        version_override: None,
        method: "native".to_string(),
        image: None,
        fakeroot: false,
        pure: false,
        root_package: "@core/base:dev".to_string(),
        no_zsa: true
    };

    zoi::cmd::package::build::run(args)
        .expect("direct build with --no-zsa failed");

    let platform = zoi::utils::get_platform().expect("unwrap failed");
    let zpa_path = root.join(format!("my-pkg-1.0.0-{platform}.zpa"));
    assert!(
        zpa_path.exists(),
        ".zpa archive should be built directly from source"
    );
    assert!(
        !root.join("my-pkg-1.0.0.zsa").exists(),
        "--no-zsa must not produce a .zsa bundle"
    );
}
