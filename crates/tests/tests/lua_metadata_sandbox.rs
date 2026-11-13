//! Security regression tests for the Lua package environment.
//!
//! Covers: metadata evaluation without host-side-effect APIs, the Rust-owned
//! build directory used by `cmd`, shell quoting of injected environment
//! values, `GPG_PASSWORD` isolation, `IMPORT`/`INCLUDE` confinement,
//! `UTILS.EXTRACT` output-directory confinement and truncated downloads.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

use mlua::Lua;
use tempfile::tempdir;
use zoi_lua::{functions, parser};

mod common;

/// A minimal, valid `metadata` block.
const METADATA: &str = r#"
metadata({
    name = "evil",
    repo = "core",
    version = "1.0.0",
    description = "x",
    maintainer = { name = "a", email = "a@example.com" },
    types = { "source" }
})
"#;

/// Writes `body` (appended to [`METADATA`]) to `<dir>/evil.pkg.lua`.
fn write_pkg(dir: &Path, body: &str) -> String {
    let path = dir.join("evil.pkg.lua");
    fs::write(&path, format!("{METADATA}\n{body}\n"))
        .expect("package file should be written");
    path.to_str().expect("utf-8 path").to_string()
}

/// Lua string literal for a path (Lua long brackets avoid escaping issues).
fn lua_str(path: &Path) -> String {
    format!("[[{}]]", path.display())
}

/// Sets up a full (build) environment with the given build directory.
fn full_env(lua: &Lua, build_dir: Option<&Path>, sub: Option<&str>) {
    functions::setup_lua_environment(
        lua,
        "linux-amd64",
        None,
        None,
        None,
        build_dir.map(|p| p.to_str().expect("utf-8 path")),
        None,
        sub,
        None,
        None,
        true
    )
    .expect("full environment should set up");
}

#[test]
fn metadata_parse_blocks_cmd_even_with_script_set_build_dir() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let marker = tmp.path().join("pwned");
    let pkg = write_pkg(
        tmp.path(),
        &format!(
            "BUILD_DIR = {}\ncmd(\"touch \" .. {})",
            lua_str(tmp.path()),
            lua_str(&marker)
        )
    );

    let err = parser::parse_lua_package(&pkg, None, None, true)
        .expect_err("cmd must not run during metadata evaluation");
    assert!(
        format!("{err:#}").contains("not available during metadata evaluation"),
        "unexpected error: {err:#}"
    );
    assert!(!marker.exists(), "cmd must not have executed");
}

#[test]
fn metadata_parse_blocks_host_writing_apis() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let src = tmp.path().join("src.txt");
    fs::write(&src, "data").expect("source file");
    let dest = tmp.path().join("dest.txt");

    let cases = [
        format!("UTILS.FS.copy({}, {})", lua_str(&src), lua_str(&dest)),
        format!("UTILS.FS.move({}, {})", lua_str(&src), lua_str(&dest)),
        format!("UTILS.FS.chmod({}, 511)", lua_str(&src)),
        format!("UTILS.FILE(\"http://127.0.0.1:9/x\", {})", lua_str(&dest)),
        "UTILS.DOWNLOAD(\"http://127.0.0.1:9/x\")".to_string(),
        "UTILS.EXTRACT(\"x.tar.gz\", \"out\")".to_string(),
        format!("UTILS.MAKE_ARCHIVE({}, \"out.zip\")", lua_str(&src)),
        "zsed(\"a\", \"b\", \"file\")".to_string(),
        "zpatch(\"fix.patch\")".to_string(),
        "zcp(\"a\", \"b\")".to_string(),
        "addPgpKey(\"key.asc\", \"name\")".to_string()
    ];

    for case in cases {
        let pkg = write_pkg(tmp.path(), &case);
        let err = parser::parse_lua_package(&pkg, None, None, true)
            .expect_err("dangerous API must fail at metadata time");
        assert!(
            format!("{err:#}")
                .contains("not available during metadata evaluation"),
            "unexpected error for `{case}`: {err:#}"
        );
    }

    assert!(src.exists(), "source must not have been moved");
    assert!(!dest.exists(), "nothing may be written during parsing");
}

#[test]
fn metadata_parse_still_supports_read_only_helpers_and_lifecycle_defs() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    fs::write(tmp.path().join("data.json"), r#"{"desc": "from import"}"#)
        .expect("data file");
    let marker = tmp.path().join("pwned");

    let pkg = write_pkg(
        tmp.path(),
        &format!(
            r#"
local data = IMPORT("data.json")
local parsed = UTILS.PARSE.json('{{"v": "2.0.0"}}')
ZOI.EXTRA = "allowed"
PKG.custom = parsed.v
assert(UTILS.FS.exists({tmp}))
metadata({{ description = data.desc }})

function prepare()
    cmd("touch " .. {marker})
end
"#,
            tmp = lua_str(tmp.path()),
            marker = lua_str(&marker)
        )
    );

    let pkg = parser::parse_lua_package(&pkg, None, None, true)
        .expect("metadata should parse");
    assert_eq!(pkg.name, "evil");
    assert_eq!(pkg.description, "from import");
    assert!(
        !marker.exists(),
        "lifecycle functions must not run at parse"
    );
}

#[test]
fn import_and_include_are_confined_to_the_package_dir() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let pkg_dir = tmp.path().join("pkg");
    fs::create_dir(&pkg_dir).expect("pkg dir");
    let secret = tmp.path().join("secret.json");
    fs::write(&secret, r#"{"token": "s3cr3t"}"#).expect("secret");

    for body in [
        "IMPORT(\"../secret.json\")".to_string(),
        format!("IMPORT({})", lua_str(&secret)),
        "INCLUDE(\"../other.lua\")".to_string()
    ] {
        let pkg = write_pkg(&pkg_dir, &body);
        let err = parser::parse_lua_package(&pkg, None, None, true)
            .expect_err("path outside the package dir must be refused");
        assert!(
            format!("{err:#}").contains("inside the package directory"),
            "unexpected error for `{body}`: {err:#}"
        );
    }
}

#[test]
fn cmd_uses_rust_side_build_dir_not_script_global() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let build_dir = tmp.path().join("build");
    let other = tmp.path().join("other");
    fs::create_dir(&build_dir).expect("build dir");
    fs::create_dir(&other).expect("other dir");

    let lua = Lua::new();
    full_env(&lua, Some(&build_dir), None);

    lua.load(format!("BUILD_DIR = {}", lua_str(&other)))
        .exec()
        .expect("assigning the global is allowed");
    let (stdout, _, _) = lua
        .load(r#"return cmd("pwd")"#)
        .eval::<(String, String, i32)>()
        .expect("cmd should run");
    let pwd = fs::canonicalize(stdout.trim()).expect("pwd exists");
    assert_eq!(
        pwd,
        fs::canonicalize(&build_dir).expect("build dir exists"),
        "cmd must run in the Rust-side build dir"
    );

    let (env_build_dir, _, _) = lua
        .load(r#"return cmd("printf %s \"$BUILD_DIR\"")"#)
        .eval::<(String, String, i32)>()
        .expect("cmd should run");
    assert_eq!(env_build_dir, build_dir.display().to_string());
}

#[test]
fn cmd_without_build_dir_is_refused_even_if_script_sets_one() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let marker = tmp.path().join("pwned");

    let lua = Lua::new();
    full_env(&lua, None, None);
    let result = lua
        .load(format!(
            "BUILD_DIR = {}\ncmd(\"touch \" .. {})",
            lua_str(tmp.path()),
            lua_str(&marker)
        ))
        .exec();
    assert!(result.is_err(), "cmd needs a Rust-side build dir");
    assert!(!marker.exists());
}

#[test]
fn injected_shell_env_values_are_not_expanded() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let build_dir = tmp.path().join("build");
    fs::create_dir(&build_dir).expect("build dir");
    let marker = build_dir.join("pwned");

    let sub = "x$(touch pwned)`touch pwned`'\"";
    let lua = Lua::new();
    full_env(&lua, Some(&build_dir), Some(sub));

    let (stdout, _, _) = lua
        .load(r#"return cmd("printf %s \"$SUBPKG\"")"#)
        .eval::<(String, String, i32)>()
        .expect("cmd should run");
    assert_eq!(stdout, sub, "value must arrive verbatim");
    assert!(!marker.exists(), "command substitution must not run");
}

#[test]
fn gpg_password_is_not_exported_to_cmd() {
    let mut ctx = common::TestContextGuard::acquire();
    ctx.set_env_var("GPG_PASSWORD", "hunter2");
    let tmp = tempdir().expect("tempdir");

    let lua = Lua::new();
    full_env(&lua, Some(tmp.path()), None);
    let (stdout, _, _) = lua
        .load(r#"return cmd("printf %s \"${GPG_PASSWORD:-unset}\"")"#)
        .eval::<(String, String, i32)>()
        .expect("cmd should run");
    assert_eq!(stdout, "unset");
}

#[test]
fn extract_rejects_out_dirs_escaping_the_build_dir() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let build_dir = tmp.path().join("build");
    fs::create_dir(&build_dir).expect("build dir");

    let lua = Lua::new();
    full_env(&lua, Some(&build_dir), None);

    for out in ["../escape", "/tmp/zoi-escape-test", "sub/../../escape", "."] {
        let err = lua
            .load(format!(r#"UTILS.EXTRACT("missing.zip", "{out}")"#))
            .exec()
            .expect_err("escaping out dir must be refused");
        assert!(
            err.to_string().contains("Invalid output directory"),
            "unexpected error for {out}: {err}"
        );
    }
    assert!(!tmp.path().join("escape").exists());
}

#[test]
fn truncated_download_is_an_error() {
    let mut ctx = common::TestContextGuard::acquire();
    ctx.set_env_var("NO_PROXY", "127.0.0.1,localhost");
    ctx.set_env_var("no_proxy", "127.0.0.1,localhost");
    common::TestContextGuard::set_offline(false);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: \
                  close\r\n\r\n0123456789"
            );
            let _ = stream.flush();
        }
    });

    let tmp = tempdir().expect("tempdir");
    let dest = tmp.path().join("file.bin");
    let result = zoi_lua::api::download::download_with_progress(
        &format!("http://127.0.0.1:{port}/file.bin"),
        &dest,
        true
    );
    let _ = server.join();

    assert!(
        result.is_err(),
        "a truncated body must not count as success"
    );
    assert!(!dest.exists(), "the partial file must be removed");
}
