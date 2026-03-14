use zoi::pkg::lua::parser;

#[test]
fn test_parse_lua_package() {
    let pkg_path = "test_assets/test.pkg.lua";
    let pkg = parser::parse_lua_package(pkg_path, None, true).unwrap();

    assert_eq!(pkg.name, "test-pkg");
    assert_eq!(pkg.repo, "core");
    assert_eq!(pkg.version, Some("1.0.0".to_string()));
    assert_eq!(pkg.description, "Test package");
    assert_eq!(pkg.maintainer.name, "Zoi");
    assert!(pkg.types.contains(&"source".to_string()));
}
