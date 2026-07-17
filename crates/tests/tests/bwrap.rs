use std::fs;
use tempfile::tempdir;
use zoi::pkg::package::bwrap;
use zoi::pkg::utils;

mod common;

#[test]
fn test_bwrap_build_checks_command_existence() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let pkg_lua_path = root.join("test.pkg.lua");
    fs::write(&pkg_lua_path, "metadata({ name = 'test', repo = 'core', types = {'source'}, maintainer = {name='test', email='test'} })").unwrap();

    // If bwrap doesn't exist, it should return an error
    if !utils::command_exists("bwrap") {
        let result = bwrap::run(
            &pkg_lua_path,
            None,
            &["linux-amd64".to_string()],
            None,
            None,
            None,
            None,
            false,
            false,
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Bubblewrap ('bwrap') is not installed")
        );
    }
}
