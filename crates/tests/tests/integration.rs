use mlua::{Lua, Table};
use zoi::pkg::lua::functions;

#[test]
fn test_lua_zcp_records_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
    )
    .unwrap();

    lua.load(r#"zcp("src", "dest")"#).exec().unwrap();

    let ops: Table = lua.globals().get("__ZoiBuildOperations").unwrap();
    let op: Table = ops.get(1).unwrap();
    let op_type: String = op.get("op").unwrap();
    let source: String = op.get("source").unwrap();
    let dest: String = op.get("destination").unwrap();

    assert_eq!(op_type, "zcp");
    assert_eq!(source, "src");
    assert_eq!(dest, "dest");
}

#[test]
fn test_lua_zlicense_records_zcp_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
    )
    .unwrap();

    lua.load(r#"zlicense("LICENSE.txt")"#).exec().unwrap();

    let ops: Table = lua.globals().get("__ZoiBuildOperations").unwrap();
    let op: Table = ops.get(1).unwrap();
    assert_eq!(op.get::<String>("op").unwrap(), "zcp");
    assert_eq!(op.get::<String>("source").unwrap(), "LICENSE.txt");
    assert_eq!(
        op.get::<String>("destination").unwrap(),
        "${pkgstore}/LICENSE"
    );
}

#[test]
fn test_lua_zdoc_records_zcp_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
    )
    .unwrap();

    lua.load(r#"zdoc("docs/README.md")"#).exec().unwrap();

    let ops: Table = lua.globals().get("__ZoiBuildOperations").unwrap();
    let op: Table = ops.get(1).unwrap();
    assert_eq!(op.get::<String>("op").unwrap(), "zcp");
    assert_eq!(op.get::<String>("source").unwrap(), "docs/README.md");
    assert_eq!(
        op.get::<String>("destination").unwrap(),
        "${pkgstore}/doc/README.md"
    );
}

#[test]
fn test_lua_zln_records_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        true,
    )
    .unwrap();

    lua.load(r#"zln("target", "link")"#).exec().unwrap();

    let ops: Table = lua.globals().get("__ZoiBuildOperations").unwrap();
    let op: Table = ops.get(1).unwrap();
    assert_eq!(op.get::<String>("op").unwrap(), "zln");
    assert_eq!(op.get::<String>("target").unwrap(), "target");
    assert_eq!(op.get::<String>("link").unwrap(), "link");
}

#[test]
fn test_is_platform_compatible() {
    use zoi::utils::is_platform_compatible;

    let allowed = vec!["linux".to_string(), "macos".to_string()];
    assert!(is_platform_compatible("linux-amd64", &allowed));
    assert!(is_platform_compatible("macos-arm64", &allowed));
    assert!(is_platform_compatible("darwin-amd64", &allowed));
    assert!(!is_platform_compatible("windows-amd64", &allowed));

    let allowed_arch = vec!["linux-arm64".to_string()];
    assert!(is_platform_compatible("linux-arm64", &allowed_arch));
    assert!(!is_platform_compatible("linux-amd64", &allowed_arch));

    let allowed_all = vec!["all".to_string()];
    assert!(is_platform_compatible("any-platform", &allowed_all));
}
