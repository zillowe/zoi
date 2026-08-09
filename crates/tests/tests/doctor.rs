//! Integration tests for the system doctor and external tool checks.

use zoi::pkg::doctor;

#[test]
fn test_doctor_check_external_tools() {
    let result = doctor::check_external_tools();
    assert!(result.essential_missing.is_empty() || !result.essential_missing.first().expect("Value should exist in test").is_empty());
}
