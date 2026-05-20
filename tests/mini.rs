use std::fs;
use zoi::pkg::mini_resolve::{MiniPackageIndex, MiniRegistryIndex, check_vulnerabilities};

#[test]
fn test_parse_mini_registry_index() {
    let path = "tests/assets/packages.json";
    let content = fs::read_to_string(path).unwrap();
    let index: MiniRegistryIndex = serde_json::from_str(&content).unwrap();

    assert!(index.packages.contains_key("hello"));
    let hello = &index.packages["hello"];
    assert_eq!(hello.repo, "zillowe");
    assert_eq!(hello.repo_type, "official");
    assert_eq!(hello.version, "4.0.0");
    assert_eq!(hello.vuln.as_ref().unwrap().len(), 1);
    assert_eq!(hello.vuln.as_ref().unwrap()[0].id, "ZSA-2026-D0042");

    assert!(index.packages.contains_key("collision"));
    assert_eq!(index.packages["collision"].repo, "extra");
}

#[test]
fn test_parse_mini_registry_config() {
    let path = "tests/assets/repo.yaml";
    let content = fs::read_to_string(path).unwrap();
    let config: zoi::pkg::types::RepoConfig = serde_yaml::from_str(&content).unwrap();

    assert_eq!(config.name, "Zoidberg");
    assert!(config.repos.iter().any(|r| r.name == "core" && r.active));
    assert!(
        config
            .repos
            .iter()
            .any(|r| r.name == "zillowe" && !r.active)
    );
}

#[test]
fn test_mini_vulnerability_check() {
    let pkg_info = MiniPackageIndex {
        repo: "zillowe".to_string(),
        repo_type: "official".to_string(),
        version: "4.0.0".to_string(),
        description: "test".to_string(),
        sub_packages: None,
        vuln: Some(vec![zoi::pkg::mini_resolve::MiniVulnerability {
            id: "VULN-1".to_string(),
            severity: "high".to_string(),
            affected_range: ">=1.0.0, <2.0.0".to_string(),
            fixed_in: Some("2.0.0".to_string()),
            summary: "test vuln".to_string(),
        }]),
    };

    assert!(check_vulnerabilities("test", &pkg_info, "2.0.0").unwrap());
}

#[test]
fn test_is_mini_mode() {
    unsafe { std::env::set_var("ZOI_MINI_MODE", "1") };
    assert!(zoi::utils::is_mini_mode());
    unsafe { std::env::set_var("ZOI_MINI_MODE", "0") };
    assert!(!zoi::utils::is_mini_mode());
    unsafe { std::env::remove_var("ZOI_MINI_MODE") };
}

#[test]
fn test_get_package_lua_url() {
    let url = zoi::pkg::mini_resolve::get_package_lua_url("core", "hello");
    assert!(url.contains("core/hello/hello.pkg.lua"));
    assert!(url.starts_with("https://gitlab.com"));
}
