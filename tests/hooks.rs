use std::fs;
use tempfile::tempdir;
use zoi::pkg::hooks::global::{GlobalHook, HookWhen, load_all_hooks};

mod common;

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

#[test]
fn test_hook_loading_is_deterministic_by_name() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home dir should be created");
    ctx.set_env_var("HOME", &home);
    ctx.set_sysroot(root.clone());

    let user_hooks = home.join(".zoi").join("hooks");
    let system_hooks = root.join("etc").join("zoi").join("hooks");
    fs::create_dir_all(&user_hooks).expect("user hooks dir should be created");
    fs::create_dir_all(&system_hooks).expect("system hooks dir should be created");

    fs::write(
        user_hooks.join("zz-user.hook.yaml"),
        "name: order-test-z\n\
description: z\n\
trigger:\n  paths: [\"*\"]\n\
action:\n  when: PostTransaction\n  exec: echo z\n",
    )
    .expect("user hook should write");
    fs::write(
        system_hooks.join("aa-system.hook.yaml"),
        "name: order-test-a\n\
description: a\n\
trigger:\n  paths: [\"*\"]\n\
action:\n  when: PostTransaction\n  exec: echo a\n",
    )
    .expect("system hook should write");
    fs::write(
        user_hooks.join("mm-user.hook.yaml"),
        "name: order-test-m\n\
description: m\n\
trigger:\n  paths: [\"*\"]\n\
action:\n  when: PostTransaction\n  exec: echo m\n",
    )
    .expect("user hook should write");

    let hooks = load_all_hooks().expect("hooks should load");
    let names: Vec<String> = hooks
        .into_iter()
        .filter(|h| h.name.starts_with("order-test-"))
        .map(|h| h.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "order-test-a".to_string(),
            "order-test-m".to_string(),
            "order-test-z".to_string()
        ]
    );
}
