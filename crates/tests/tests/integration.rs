//! General integration tests, including Lua API interactions.

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
    .expect("unwrap failed",);

    lua.load(r#"zcp("src", "dest")"#,)
        .exec()
        .expect("unwrap failed",);

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations",)
        .expect("unwrap failed",);
    let op: Table = ops.get(1,).expect("unwrap failed",);
    let op_type: String = op.get("op",).expect("unwrap failed",);
    let source: String = op.get("source",).expect("unwrap failed",);
    let dest: String = op.get("destination",).expect("unwrap failed",);

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
    .expect("unwrap failed",);

    lua.load(r#"zlicense("LICENSE.txt")"#,)
        .exec()
        .expect("unwrap failed",);

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations",)
        .expect("unwrap failed",);
    let op: Table = ops.get(1,).expect("unwrap failed",);
    assert_eq!(op.get::<String>("op").expect("unwrap failed"), "zcp");
    assert_eq!(
        op.get::<String>("source").expect("unwrap failed"),
        "LICENSE.txt"
    );
    assert_eq!(
        op.get::<String>("destination").expect("unwrap failed"),
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
    .expect("unwrap failed",);

    lua.load(r#"zdoc("docs/README.md")"#,)
        .exec()
        .expect("unwrap failed",);

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations",)
        .expect("unwrap failed",);
    let op: Table = ops.get(1,).expect("unwrap failed",);
    assert_eq!(op.get::<String>("op").expect("unwrap failed"), "zcp");
    assert_eq!(
        op.get::<String>("source").expect("unwrap failed"),
        "docs/README.md"
    );
    assert_eq!(
        op.get::<String>("destination").expect("unwrap failed"),
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
    .expect("unwrap failed",);

    lua.load(r#"zln("target", "link")"#,)
        .exec()
        .expect("unwrap failed",);

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations",)
        .expect("unwrap failed",);
    let op: Table = ops.get(1,).expect("unwrap failed",);
    assert_eq!(op.get::<String>("op").expect("unwrap failed"), "zln");
    assert_eq!(op.get::<String>("target").expect("unwrap failed"), "target");
    assert_eq!(op.get::<String>("link").expect("unwrap failed"), "link");
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
