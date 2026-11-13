//! Security regression tests for the PGP keyring.
//!
//! Covers: key-name validation, refusing to replace a stored key with a
//! different fingerprint, exact (non-prefix) trusted-key matching, and
//! `addPgpKey` name validation from Lua.

use std::fs;
use std::path::Path;

use mlua::Lua;
use tempfile::tempdir;
use zoi_core::pgp::{self, sequoia_openpgp as openpgp};

mod common;

use openpgp::cert::CertBuilder;
use openpgp::serialize::SerializeInto;

/// Generates a fresh certificate and returns it with its armored public key.
fn new_cert(uid: &str) -> (openpgp::Cert, Vec<u8>) {
    let (cert, _) = CertBuilder::general_purpose([uid])
        .generate()
        .expect("certificate should generate");
    let armored = cert.armored().to_vec().expect("certificate should armor");
    (cert, armored)
}

/// Points Zoi's user data directory at `dir`.
fn isolate_keyring(ctx: &mut common::TestContextGuard, dir: &Path) {
    ctx.set_env_var("HOME", dir.join("home"));
    ctx.set_env_var("XDG_DATA_HOME", dir.join("data"));
}

/// Running these as root would touch the system keyring.
fn running_as_root() -> bool {
    zoi_core::utils::is_effective_root()
}

#[test]
fn key_names_must_be_single_safe_components() {
    for bad in ["", "../evil", "a/b", "a\\b", ".hidden", "x..y", "nul\0"] {
        assert!(
            pgp::validate_key_name(bad).is_err(),
            "{bad:?} should be rejected"
        );
    }
    for good in ["zillowe-main", "alice_2024", "Key.Name"] {
        assert!(
            pgp::validate_key_name(good).is_ok(),
            "{good:?} should be accepted"
        );
    }
}

#[test]
fn stored_key_is_not_replaced_by_a_different_fingerprint() {
    if running_as_root() {
        return;
    }
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    isolate_keyring(&mut ctx, tmp.path());

    let (_, alice) = new_cert("Alice <alice@example.org>");
    let (_, mallory) = new_cert("Mallory <mallory@example.org>");

    pgp::add_key_from_bytes(&alice, "maintainer", true)
        .expect("first key should be stored");
    pgp::add_key_from_bytes(&alice, "maintainer", true)
        .expect("re-adding the same key is fine");

    let err = pgp::add_key_from_bytes(&mallory, "maintainer", true)
        .expect_err("a different key must not replace the stored one");
    assert!(err.to_string().contains("different key"), "{err}");

    let stored =
        fs::read(pgp::get_pgp_dir().expect("pgp dir").join("maintainer.asc"))
            .expect("stored key");
    assert_eq!(stored, alice, "the original key must be kept");

    assert!(
        pgp::add_key_from_bytes(&alice, "../escape", true).is_err(),
        "path-like names must be refused"
    );
    assert!(!tmp.path().join("data/zoi/escape.asc").exists());
}

#[test]
fn trusted_keys_require_exact_name_or_full_fingerprint() {
    if running_as_root() {
        return;
    }
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    isolate_keyring(&mut ctx, tmp.path());

    let (cert, armored) = new_cert("Trusted <trusted@example.org>");
    pgp::add_key_from_bytes(&armored, "trusted", true).expect("store key");
    let fpr = cert.fingerprint().to_hex();

    let spaced: String = fpr
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).expect("hex"))
        .collect::<Vec<_>>()
        .join(" ");

    for ok in [
        "trusted".to_string(),
        fpr.clone(),
        fpr.to_lowercase(),
        spaced
    ] {
        let certs =
            pgp::get_certs_by_name_or_fingerprint(std::slice::from_ref(&ok))
                .unwrap_or_else(|e| panic!("{ok} should resolve: {e}"));
        assert_eq!(certs.len(), 1);
    }

    let prefix = fpr.get(..16).expect("prefix").to_string();
    let suffix = fpr.get(fpr.len() - 16..).expect("suffix").to_string();
    for bad in [prefix, suffix, "TRUSTED".to_string(), "trust".to_string()] {
        assert!(
            pgp::get_certs_by_name_or_fingerprint(std::slice::from_ref(&bad))
                .is_err(),
            "{bad} must not match"
        );
    }
}

#[test]
fn add_pgp_key_from_lua_rejects_unsafe_names() {
    if running_as_root() {
        return;
    }
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    isolate_keyring(&mut ctx, tmp.path());
    let (_, armored) = new_cert("Lua <lua@example.org>");
    let key_file = tmp.path().join("key.asc");
    fs::write(&key_file, &armored).expect("key file");

    let lua = Lua::new();
    zoi_lua::functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        Some(tmp.path().to_str().expect("utf-8")),
        None,
        None,
        None,
        None,
        true
    )
    .expect("environment");

    let result = lua
        .load(format!(
            "return addPgpKey([[{}]], \"../../evil\")",
            key_file.display()
        ))
        .exec();
    assert!(result.is_err(), "unsafe key names must raise an error");

    let ok: bool = lua
        .load(format!(
            "return addPgpKey([[{}]], \"lua-key\")",
            key_file.display()
        ))
        .eval()
        .expect("valid name should work");
    assert!(ok);
}
