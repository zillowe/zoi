use zoi::pkg::resolve;

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
