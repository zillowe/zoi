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
        true
    )
    .expect("unwrap failed");

    lua.load(r#"zcp("src", "dest")"#)
        .exec()
        .expect("unwrap failed");

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations")
        .expect("unwrap failed");
    let op: Table = ops.get(1).expect("unwrap failed");
    let op_type: String = op.get("op").expect("unwrap failed");
    let source: String = op.get("source").expect("unwrap failed");
    let dest: String = op.get("destination").expect("unwrap failed");

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
        true
    )
    .expect("unwrap failed");

    lua.load(r#"zlicense("LICENSE.txt")"#)
        .exec()
        .expect("unwrap failed");

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations")
        .expect("unwrap failed");
    let op: Table = ops.get(1).expect("unwrap failed");
    assert_eq!(op.get::<String>("op").expect("unwrap failed"), "zcp");
    assert_eq!(
        op.get::<String>("source").expect("unwrap failed"),
        "LICENSE.txt"
    );
    assert_eq!(
        op.get::<String>("destination").expect("unwrap failed"),
        "${pkgstore}/LICENSE.txt"
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
        true
    )
    .expect("unwrap failed");

    lua.load(r#"zdoc("docs/README.md")"#)
        .exec()
        .expect("unwrap failed");

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations")
        .expect("unwrap failed");
    let op: Table = ops.get(1).expect("unwrap failed");
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

/// The destinations `zlicense`/`zdoc` record for a system-scope build of
/// package `demo`, optionally as sub-package `sub`.
fn system_staging_destinations(sub: Option<&str>, code: &str) -> Vec<String> {
    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        None,
        None,
        sub,
        Some(zoi::pkg::types::Scope::System),
        None,
        true
    )
    .expect("lua environment");
    let pkg = lua.create_table().expect("table");
    pkg.set("name", "demo").expect("set name");
    lua.globals().set("PKG", pkg).expect("set PKG");
    lua.load(code).exec().expect("staging call");
    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations")
        .expect("operations");
    ops.sequence_values::<Table>()
        .map(|op| {
            op.expect("op")
                .get::<String>("destination")
                .expect("destination")
        })
        .collect()
}

#[test]
fn zlicense_and_zdoc_use_the_package_directory_in_system_scope() {
    for sub in [None, Some("main")] {
        assert_eq!(
            system_staging_destinations(
                sub,
                r#"zlicense("src/COPYING"); zdoc("src/README.md")"#
            ),
            [
                "${usrroot}/usr/share/licenses/demo/COPYING",
                "${usrroot}/usr/share/doc/demo/README.md"
            ]
        );
    }
}

#[test]
fn zlicense_and_zdoc_give_each_sub_package_its_own_directory() {
    assert_eq!(
        system_staging_destinations(
            Some("libs"),
            r#"zlicense("src/COPYING"); zdoc("src/NEWS")"#
        ),
        [
            "${usrroot}/usr/share/licenses/demo-libs/COPYING",
            "${usrroot}/usr/share/doc/demo-libs/NEWS"
        ]
    );
}

#[test]
fn zlicense_and_zdoc_accept_a_file_name() {
    assert_eq!(
        system_staging_destinations(
            None,
            r#"zlicense("src/lib/LICENSE", "LICENSE.lib")
               zdoc("src/doc/README", "README.doc")"#
        ),
        [
            "${usrroot}/usr/share/licenses/demo/LICENSE.lib",
            "${usrroot}/usr/share/doc/demo/README.doc"
        ]
    );
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
        true
    )
    .expect("lua environment");
    assert!(
        lua.load(r#"zlicense("LICENSE", "../escape")"#)
            .exec()
            .is_err(),
        "a name with a path must be rejected"
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
        true
    )
    .expect("unwrap failed");

    lua.load(r#"zln("target", "link")"#)
        .exec()
        .expect("unwrap failed");

    let ops: Table = lua
        .globals()
        .get("__ZoiBuildOperations")
        .expect("unwrap failed");
    let op: Table = ops.get(1).expect("unwrap failed");
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
