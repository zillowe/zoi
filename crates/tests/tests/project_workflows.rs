//! Integration tests for project workflow execution and task running.

use std::fs;

use tempfile::tempdir;
use zoi::project::{config, runner};

mod common;

#[test]
fn test_project_run_command() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let lua = r#"
project({ name = "test-workflow" })
tasks({
    { cmd = "build", run = 'echo "hello" > built.txt' },
})
"#;
    fs::write(root.join("zoi.lua"), lua).expect("unwrap failed");

    ctx.set_current_dir(&root);

    let cfg = config::load().expect("unwrap failed");
    runner::run(Some("build"), &[], &cfg).expect("unwrap failed");

    let out_file = root.join("built.txt");
    assert!(out_file.exists());
    let content = fs::read_to_string(out_file).expect("unwrap failed");
    assert_eq!(content.trim(), "hello");
}
