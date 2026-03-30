use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use zoi::pkg::resolve;
use zoi::pkg::{config, sysroot, types};

#[test]
fn test_parse_source_string_basic() {
    let req = resolve::parse_source_string("hello").unwrap();
    assert_eq!(req.name, "hello");
    assert_eq!(req.repo, None);
    assert_eq!(req.handle, None);
    assert_eq!(req.version_spec, None);
}

#[test]
fn test_parse_source_string_repo() {
    let req = resolve::parse_source_string("@community/hello").unwrap();
    assert_eq!(req.name, "hello");
    assert_eq!(req.repo, Some("community".to_string()));
    assert_eq!(req.handle, None);
}

#[test]
fn test_parse_source_string_handle() {
    let req = resolve::parse_source_string("#zoidberg@core/hello").unwrap();
    assert_eq!(req.name, "hello");
    assert_eq!(req.repo, Some("core".to_string()));
    assert_eq!(req.handle, Some("zoidberg".to_string()));
}

#[test]
fn test_parse_source_string_version() {
    let req = resolve::parse_source_string("hello@1.2.3").unwrap();
    assert_eq!(req.name, "hello");
    assert_eq!(req.version_spec, Some("1.2.3".to_string()));
}

#[test]
fn test_parse_source_string_subpackage() {
    let req = resolve::parse_source_string("linux:headers").unwrap();
    assert_eq!(req.name, "linux");
    assert_eq!(req.sub_package, Some("headers".to_string()));
}

#[test]
fn test_parse_source_string_complex() {
    let req = resolve::parse_source_string("#my-reg@extra/pkg:sub@v2.0.0").unwrap();
    assert_eq!(req.handle, Some("my-reg".to_string()));
    assert_eq!(req.repo, Some("extra".to_string()));
    assert_eq!(req.name, "pkg");
    assert_eq!(req.sub_package, Some("sub".to_string()));
    assert_eq!(req.version_spec, Some("v2.0.0".to_string()));
}

#[test]
fn test_parse_source_string_local_file_with_relative_prefix() {
    let req = resolve::parse_source_string("./athas.pkg.lua").unwrap();
    assert_eq!(req.name, "athas");
    assert_eq!(req.repo, None);
    assert_eq!(req.handle, None);
    assert_eq!(req.version_spec, None);
}

#[test]
fn test_parse_source_string_local_file_with_version_and_subpackage() {
    let req = resolve::parse_source_string("athas.pkg.lua:dev@1.2.3").unwrap();
    assert_eq!(req.name, "athas");
    assert_eq!(req.sub_package, Some("dev".to_string()));
    assert_eq!(req.version_spec, Some("1.2.3".to_string()));
}

#[test]
fn test_parse_source_string_nested_local_file_with_version() {
    let req = resolve::parse_source_string("test_assets/test.pkg.lua@1.0.0").unwrap();
    assert_eq!(req.name, "test");
    assert_eq!(req.sub_package, None);
    assert_eq!(req.version_spec, Some("1.0.0".to_string()));
}

#[test]
fn test_resolve_requested_version_spec_local_channel_stable() {
    let version = resolve::resolve_requested_version_spec(
        "test_assets/test_channels.pkg.lua@stable",
        true,
        true,
    )
    .unwrap();
    assert_eq!(version, Some("1.0.0".to_string()));
}

#[test]
fn test_resolve_requested_version_spec_local_channel_alpha() {
    let version = resolve::resolve_requested_version_spec(
        "test_assets/test_channels.pkg.lua@alpha",
        true,
        true,
    )
    .unwrap();
    assert_eq!(version, Some("1.1.0-alpha".to_string()));
}

#[test]
fn test_resolve_package_defaults_deterministically_without_stable() {
    let (_, version, _, _, _, _) =
        resolve::resolve_package_and_version("test_assets/test_no_stable.pkg.lua", true, true)
            .expect("package should resolve");
    assert_eq!(version, "1.0.0-alpha".to_string());
}

#[test]
fn test_resolve_requested_version_spec_registry_channel_and_exact() {
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home should be created");

    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("ZOI_DB_DIR", root.join("db"));
    }
    sysroot::set_sysroot(root.clone());

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: "testreg".to_string(),
            url: "https://example.invalid/testreg.git".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        repos: vec!["core".to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("config should write");

    let pkg_dir = PathBuf::from(std::env::var("ZOI_DB_DIR").expect("db dir env"))
        .join("testreg")
        .join("core")
        .join("registry-channels");
    fs::create_dir_all(&pkg_dir).expect("pkg dir should be created");
    fs::write(
        pkg_dir.join("registry-channels.pkg.lua"),
        r#"metadata({
  name = "registry-channels",
  repo = "core",
  versions = {
    stable = "4.0.0",
    alpha = "4.1.0-alpha",
  },
  description = "Registry channel test",
  maintainer = { name = "Zoi", email = "zoi@example.com" },
  types = { "source" },
})"#,
    )
    .expect("pkg.lua should write");

    let stable = resolve::resolve_requested_version_spec("registry-channels@stable", true, true)
        .expect("stable should resolve");
    assert_eq!(stable, Some("4.0.0".to_string()));

    let alpha = resolve::resolve_requested_version_spec("registry-channels@alpha", true, true)
        .expect("alpha should resolve");
    assert_eq!(alpha, Some("4.1.0-alpha".to_string()));

    let exact = resolve::resolve_requested_version_spec("registry-channels@4.0.0", true, true)
        .expect("exact should resolve");
    assert_eq!(exact, Some("4.0.0".to_string()));

    unsafe {
        std::env::remove_var("ZOI_DB_DIR");
    }
}
