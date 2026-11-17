//! `zchmod` modes: integers as written, strings as octal.

use mlua::Lua;
use zoi::pkg::lua::api::fs::parse_mode;

#[test]
fn string_modes_are_octal() {
    let lua = Lua::new();
    let s = |v: &str| mlua::Value::String(lua.create_string(v).expect("str"));
    assert_eq!(parse_mode(&s("0755")), Ok(0o755));
    assert_eq!(parse_mode(&s("755")), Ok(0o755));
    assert_eq!(parse_mode(&s("0o644")), Ok(0o644));
    assert_eq!(parse_mode(&s("1777")), Ok(0o1777));
    assert!(parse_mode(&s("0789")).is_err());
    assert!(parse_mode(&s("rwx")).is_err());
}

#[test]
fn integer_modes_are_used_as_written() {
    assert_eq!(parse_mode(&mlua::Value::Integer(493)), Ok(0o755));
    assert_eq!(parse_mode(&mlua::Value::Number(420.0)), Ok(0o644));
    assert!(parse_mode(&mlua::Value::Integer(-1)).is_err());
    assert!(parse_mode(&mlua::Value::Integer(0o10000)).is_err());
    assert!(parse_mode(&mlua::Value::Number(1.5)).is_err());
}
