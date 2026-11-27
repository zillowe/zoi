//! Password hashes from `zoi system secret hash`.
use argon2::password_hash::PasswordVerifier;
use zoi_system::secret::{HashAlgorithm, hash_password, hash_password_with};

#[test]
fn the_default_hash_is_yescrypt_with_libxcrypts_default_parameters() {
    let hash = hash_password("hunter2").expect("hash");
    // `j9T` encodes N = 4096, r = 32, p = 1: libxcrypt's default cost, so
    // the hash looks like one `passwd` writes on Arch.
    assert!(hash.starts_with("$y$j9T$"), "{hash}");
}

#[test]
fn a_yescrypt_hash_verifies_only_its_own_password() {
    let hash =
        hash_password_with("hunter2", HashAlgorithm::Yescrypt).expect("hash");
    let yescrypt = yescrypt::Yescrypt::default();
    assert!(yescrypt.verify_password(b"hunter2", hash.as_str()).is_ok());
    assert!(yescrypt.verify_password(b"hunter3", hash.as_str()).is_err());
}

#[test]
fn an_argon2_hash_is_argon2id_in_phc_format_and_verifies() {
    let hash =
        hash_password_with("hunter2", HashAlgorithm::Argon2).expect("hash");
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    let argon2 = argon2::Argon2::default();
    assert!(argon2.verify_password(b"hunter2", hash.as_str()).is_ok());
    assert!(argon2.verify_password(b"hunter3", hash.as_str()).is_err());
}

#[test]
fn every_hash_gets_a_fresh_salt() {
    for algorithm in [HashAlgorithm::Yescrypt, HashAlgorithm::Argon2] {
        let first = hash_password_with("same", algorithm).expect("hash");
        let second = hash_password_with("same", algorithm).expect("hash");
        assert_ne!(first, second, "{algorithm:?}");
    }
}
