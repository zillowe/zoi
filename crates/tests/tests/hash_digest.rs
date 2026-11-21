//! Tests for the `<algo>-<hex>` digests Zoi records (SHA-512) and still
//! reads (SHA-256).

use zoi_core::hash::{
    DEFAULT_ALGORITHM, HashAlgorithm, bytes_digest, bytes_match_digest,
    bytes_match_hex, calculate_string_hash, format_digest, parse_digest
};

#[test]
fn zoi_records_sha512_digests() {
    assert_eq!(DEFAULT_ALGORITHM, HashAlgorithm::Sha512);
    let digest = bytes_digest(b"zoi");
    let (algo, hex) = parse_digest(&digest).expect("own digest parses");
    assert_eq!(algo, HashAlgorithm::Sha512);
    assert_eq!(hex, calculate_string_hash("zoi", HashAlgorithm::Sha512));
}

#[test]
fn digests_of_either_algorithm_are_checked() {
    let sha256 = calculate_string_hash("zoi", HashAlgorithm::Sha256);
    let sha512 = calculate_string_hash("zoi", HashAlgorithm::Sha512);

    assert!(bytes_match_digest(
        b"zoi",
        &format_digest(HashAlgorithm::Sha256, &sha256)
    ));
    assert!(bytes_match_digest(
        b"zoi",
        &format_digest(HashAlgorithm::Sha512, &sha512)
    ));
    assert!(!bytes_match_digest(
        b"other",
        &format_digest(HashAlgorithm::Sha512, &sha512)
    ));

    // Bare digests are told apart by their length.
    assert!(bytes_match_hex(b"zoi", &sha256));
    assert!(bytes_match_hex(b"zoi", &sha512));
    assert!(!bytes_match_hex(b"other", &sha512));
}

#[test]
fn malformed_digests_are_refused() {
    let sha512 = calculate_string_hash("zoi", HashAlgorithm::Sha512);
    // The hex part must have the algorithm's length and be hex.
    assert!(parse_digest(&format!("sha256-{sha512}")).is_none());
    assert!(parse_digest("sha512-xyz").is_none());
    assert!(parse_digest("md5-0123456789abcdef0123456789abcdef").is_none());
    assert!(parse_digest(&sha512).is_none());
    assert!(!bytes_match_digest(b"zoi", "sha512-"));
    assert!(!bytes_match_hex(b"zoi", "abc"));
}
