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
    fs::create_dir_all(&pkg_dir).unwrap();

    let pkg_lua = pkg_dir.join("my-pkg.pkg.lua");
    let asset_file = pkg_dir.join("hello.txt");
    fs::write(&asset_file, "hello from asset").unwrap();

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
    fs::write(&pkg_lua, lua_code).unwrap();

    // 1. Bundle
    zoi::bundle_package(&pkg_lua, Some(&root), None, None).expect("bundling failed");
    let zsa_path = root.join("my-pkg-1.0.0.zsa");
    assert!(zsa_path.exists(), ".zsa bundle should exist");

    // 2. Build from .zsa
    let build_options = zoi::BuildOptions {
        build_type: Some("source"),
        output_dir: Some(root.clone()),
        ..Default::default()
    };
    zoi::build_with_options(&zsa_path, &build_options).expect("build from .zsa failed");

    let platform = zoi::utils::get_platform().unwrap();
    let zpa_path = root.join(format!("my-pkg-1.0.0-{}.zpa", platform));
    assert!(
        zpa_path.exists(),
        ".zpa archive should exist after build from .zsa"
    );

    // 3. Install from .zsa (end-to-end)
    let install_options = zoi::SourceInstallOptions {
        scope_override: Some(Scope::User),
        yes: true,
        ..Default::default()
    };

    // We'll use a clean sysroot to verify installation
    let sysroot = root.join("sysroot");
    fs::create_dir_all(&sysroot).unwrap();
    ctx.set_sysroot(sysroot.clone());

    zoi::install_sources(&[zsa_path.to_string_lossy().to_string()], &install_options)
        .expect("install from .zsa failed");

    // Verify installation
    // Path should be sysroot/home/.zoi/pkgs/store/8f...-my-pkg/1.0.0/bin/hello-bin
    let mut found = false;
    for entry in walkdir::WalkDir::new(&sysroot) {
        let entry = entry.unwrap();
        let path = entry.path();
        // Skip shims which are in .../bin/ and are copies of zoi binary
        if path.to_string_lossy().contains("store")
            && entry.file_name().to_string_lossy() == "hello-bin"
        {
            let content = fs::read_to_string(path).unwrap();
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
