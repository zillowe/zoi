use std::fs;
use tempfile::tempdir;

mod common;

#[test]
fn test_zoiignore_bundle() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_dir = root.join("my-pkg");
    fs::create_dir_all(&pkg_dir).unwrap();

    let pkg_lua = pkg_dir.join("my-pkg.pkg.lua");
    let asset_file = pkg_dir.join("hello.txt");
    let ignore_file = pkg_dir.join("secret.txt");
    let zoiignore = pkg_dir.join(".zoiignore");

    fs::write(&asset_file, "hello from asset").unwrap();
    fs::write(&ignore_file, "top secret").unwrap();
    fs::write(&zoiignore, "secret.txt\n*.tmp\n").unwrap();
    fs::write(pkg_dir.join("temp.tmp"), "trash").unwrap();

    let lua_code = r#"
metadata({
    name = "my-pkg",
    repo = "test",
    version = "1.0.0",
    description = "Test package",
    maintainer = { name = "test", email = "test@example.com" },
    types = { "source" }
})

function package()
    zcp("${pkgluadir}/hello.txt", "${pkgstore}/hello.txt")
    zcp("${pkgluadir}/secret.txt", "${pkgstore}/secret.txt")
    zcp("${pkgluadir}/temp.tmp", "${pkgstore}/temp.tmp")
end
"#;
    fs::write(&pkg_lua, lua_code).unwrap();

    // Bundle
    zoi::bundle_package(&pkg_lua, Some(&root), None, None, None).expect("bundling failed");
    let zsa_path = root.join("my-pkg-1.0.0.zsa");
    assert!(zsa_path.exists(), ".zsa bundle should exist");

    // Inspect bundle contents
    let file = fs::File::open(&zsa_path).unwrap();
    let decoder = zstd::stream::read::Decoder::new(file).unwrap();
    let mut archive = tar::Archive::new(decoder);

    let mut found_hello = false;
    let mut found_secret = false;
    let mut found_temp = false;

    for entry in archive.entries().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path().unwrap();
        let path_str = path.to_string_lossy();

        if path_str == "hello.txt" {
            found_hello = true;
        } else if path_str == "secret.txt" {
            found_secret = true;
        } else if path_str == "temp.tmp" {
            found_temp = true;
        }
    }

    assert!(found_hello, "hello.txt should be in the bundle");
    assert!(!found_secret, "secret.txt should NOT be in the bundle");
    assert!(!found_temp, "temp.tmp should NOT be in the bundle");
}
