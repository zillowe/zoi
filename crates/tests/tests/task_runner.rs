use std::fs;
use tempfile::tempdir;
use zoi::project::{config, runner};

mod common;

#[test]
fn test_task_runner_sequential_dependencies() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    let yaml = r#"
name: test-task-deps
commands:
  - cmd: first
    run: echo "1" > first.txt
  - cmd: second
    run: echo "2" > second.txt
    depends_on: ["first"]
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();

    let cfg = config::load().unwrap();
    runner::run(Some("second"), &[], &cfg).unwrap();

    assert!(
        root.join("first.txt").exists(),
        "First task should have run"
    );
    assert!(
        root.join("second.txt").exists(),
        "Second task should have run"
    );
}

#[test]
fn test_task_runner_caching() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    let yaml = r#"
name: test-task-caching
commands:
  - cmd: cached-task
    run: echo "running" >> output.txt
    cache_files: ["input.txt"]
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();
    fs::write(root.join("input.txt"), "input-v1").unwrap();

    let cfg = config::load().unwrap();

    runner::run(Some("cached-task"), &[], &cfg).unwrap();
    let count1 = fs::read_to_string(root.join("output.txt"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(count1, 1);

    runner::run(Some("cached-task"), &[], &cfg).unwrap();
    let count2 = fs::read_to_string(root.join("output.txt"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(count2, 1, "Should have been skipped due to caching");

    fs::write(root.join("input.txt"), "input-v2").unwrap();
    runner::run(Some("cached-task"), &[], &cfg).unwrap();
    let count3 = fs::read_to_string(root.join("output.txt"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(count3, 2, "Should have run again after input changed");
}

#[test]
fn test_task_runner_circular_dependency_detection() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    let yaml = r#"
name: test-circular
commands:
  - cmd: a
    run: echo a
    depends_on: ["b"]
  - cmd: b
    run: echo b
    depends_on: ["a"]
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();

    let cfg = config::load().unwrap();
    let res = runner::run(Some("a"), &[], &cfg);

    assert!(res.is_err(), "Should detect circular dependency");
    assert!(res.unwrap_err().to_string().contains("Circular dependency"));
}
