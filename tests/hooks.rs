use std::fs;
use tempfile::tempdir;
use zoi::pkg::hooks::global::{
    GlobalHook, HookTrigger, HookWhen, load_all_hooks, trigger_matches_modified_files,
};

mod common;

#[test]
fn test_hook_deserialization() {
    let hook_yaml = r#"
name: test-hook
description: A test hook
platforms: ["linux"]
trigger:
  dirs:
    - "usr/share/icons"
  paths:
    - "usr/share/fonts/**"
  operation: ["install"]
action:
  when: PostTransaction
  exec: echo "triggered"
"#;

    let hook: GlobalHook = serde_yaml::from_str(hook_yaml).unwrap();
    assert_eq!(hook.name, "test-hook");
    assert_eq!(hook.trigger.dirs[0], "usr/share/icons");
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
fn test_hook_dir_trigger_matches_descendants_once_per_transaction_input() {
    let trigger = HookTrigger {
        paths: Vec::new(),
        dirs: vec!["usr/share/icons".to_string()],
        operation: Vec::new(),
    };
    let modified_files = vec![
        "usr/share/icons/hicolor/48x48/apps/example.png".to_string(),
        "usr/share/icons/hicolor/index.theme".to_string(),
    ];

    assert!(trigger_matches_modified_files(&trigger, &modified_files));
}

#[test]
fn test_hook_dir_trigger_does_not_match_similar_prefix() {
    let trigger = HookTrigger {
        paths: Vec::new(),
        dirs: vec!["usr/share/icons".to_string()],
        operation: Vec::new(),
    };
    let modified_files = vec!["usr/share/icons-extra/example.png".to_string()];

    assert!(!trigger_matches_modified_files(&trigger, &modified_files));
}

#[test]
fn test_hook_trigger_matches_sysroot_relative_dir() {
    let ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();
    ctx.set_sysroot(root.clone());

    let trigger = HookTrigger {
        paths: Vec::new(),
        dirs: vec!["usr/share/icons".to_string()],
        operation: Vec::new(),
    };
    let modified_files = vec![
        root.join("usr/share/icons/hicolor/index.theme")
            .to_string_lossy()
            .to_string(),
    ];

    assert!(trigger_matches_modified_files(&trigger, &modified_files));
}

#[test]
fn test_hook_path_trigger_still_matches_globs() {
    let trigger = HookTrigger {
        paths: vec!["usr/share/fonts/**".to_string()],
        dirs: Vec::new(),
        operation: Vec::new(),
    };
    let modified_files = vec!["usr/share/fonts/TTF/example.ttf".to_string()];

    assert!(trigger_matches_modified_files(&trigger, &modified_files));
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
