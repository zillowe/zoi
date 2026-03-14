use mlua::{Lua, Table};
use zoi::pkg::lua::functions;

#[test]
fn test_lua_zcp_records_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(&lua, "linux-amd64", None, None, None, None, true).unwrap();

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
fn test_lua_zln_records_operation() {
    let lua = Lua::new();
    functions::setup_lua_environment(&lua, "linux-amd64", None, None, None, None, true).unwrap();

    lua.load(r#"zln("target", "link")"#).exec().unwrap();

    let ops: Table = lua.globals().get("__ZoiBuildOperations").unwrap();
    let op: Table = ops.get(1).unwrap();
    assert_eq!(op.get::<String>("op").unwrap(), "zln");
    assert_eq!(op.get::<String>("target").unwrap(), "target");
    assert_eq!(op.get::<String>("link").unwrap(), "link");
}
