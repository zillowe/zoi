use std::fs;
use tempfile::tempdir;
use zoi::pkg::hooks::global::{GlobalHook, HookWhen};

#[test]
fn test_hook_deserialization() {
    let hook_yaml = r#"
name: test-hook
description: A test hook
platforms: ["linux"]
trigger:
  paths:
    - "usr/share/fonts/**"
  operation: ["install"]
action:
  when: PostTransaction
  exec: echo "triggered"
"#;

    let hook: GlobalHook = serde_yaml::from_str(hook_yaml).unwrap();
    assert_eq!(hook.name, "test-hook");
    assert_eq!(hook.trigger.paths[0], "usr/share/fonts/**");
    assert_eq!(hook.action.when, HookWhen::PostTransaction);
}

#[test]
fn test_hook_loading_dirs() {
    let dir = tempdir().unwrap();
    let hook_path = dir.path().join("test.hook.yaml");

    let content = r#"
name: dynamic-hook
description: Loaded from dir
trigger:
  paths: ["bin/*"]
action:
  when: PostTransaction
  exec: ls
"#;
    fs::write(&hook_path, content).unwrap();

    let loaded: GlobalHook = serde_yaml::from_str(&fs::read_to_string(hook_path).unwrap()).unwrap();
    assert_eq!(loaded.name, "dynamic-hook");
}
