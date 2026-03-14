use std::fs;
use tempfile::tempdir;
use zoi::project::{config, runner};

#[test]
fn test_project_run_command() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let yaml = r#"
name: test-workflow
commands:
  - cmd: build
    run: echo "hello" > built.txt
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&root).unwrap();

    let cfg = config::load().unwrap();
    runner::run(Some("build"), &[], &cfg).unwrap();

    let out_file = root.join("built.txt");
    assert!(out_file.exists());
    let content = fs::read_to_string(out_file).unwrap();
    assert_eq!(content.trim(), "hello");

    std::env::set_current_dir(old_dir).unwrap();
}
