use std::fs;
use tempfile::tempdir;
use zoi::pkg::package::doctor;

#[test]
fn package_doctor_accepts_valid_minimal_file() {
    let report = doctor::run(
        std::path::Path::new("test_assets/test.pkg.lua"),
        Some("linux-amd64"),
        None,
    )
    .expect("doctor should parse minimal test package");

    assert!(
        report.errors.is_empty(),
        "expected no errors, got: {:?}",
        report.errors
    );
}

#[test]
fn package_doctor_reports_invalid_dependency_and_main_subs() {
    let tmp = tempdir().expect("tempdir should be created");
    let pkg_path = tmp.path().join("broken.pkg.lua");

    let lua = r#"
metadata({
  name = "broken",
  repo = "core",
  version = "1.0.0",
  description = "broken package",
  maintainer = { name = "Maintainer", email = "maintainer@example.com" },
  license = "MIT",
  types = { "source" },
  sub_packages = { "cli" },
  main_subs = { "cli", "missing" },
})

dependencies({
  runtime = {
    required = { "native:" }
  }
})

function package()
  return true
end
"#;

    fs::write(&pkg_path, lua).expect("test package should be written");

    let report = doctor::run(&pkg_path, Some("linux-amd64"), None)
        .expect("doctor should return report for invalid package");

    assert!(!report.errors.is_empty());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("Invalid dependency")),
        "expected invalid dependency error, got: {:?}",
        report.errors
    );
    assert!(
        report.errors.iter().any(|e| e.contains("main_subs")),
        "expected main_subs error, got: {:?}",
        report.errors
    );
}
