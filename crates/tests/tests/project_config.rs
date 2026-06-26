use zoi::project::config;

#[test]
fn test_deserialize_project_config_basic() {
    let yaml = r#"
name: my-test-project
config:
  local: true
pkgs:
  - eza
  - bat
"#;
    let cfg: config::ProjectConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.name, "my-test-project");
    assert!(cfg.config.local);
    assert_eq!(cfg.pkgs.len(), 2);
    assert!(cfg.pkgs.contains(&"eza".to_string()));
}

#[test]
fn test_deserialize_project_config_versioned_pkgs() {
    let yaml = r#"
name: versioned-project
pkgs:
  - fzf: "0.44.1"
  - fd: "8.7.0"
"#;
    let cfg: config::ProjectConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(
        cfg.pkgs.contains(&"fzf@0.44.1".to_string())
            || cfg.pkgs.contains(&"fzf: 0.44.1".to_string())
    );
}
