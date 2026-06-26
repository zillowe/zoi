use clap_complete::Shell;
use std::fs;
use tempfile::tempdir;
use zoi::cmd::shell;
use zoi::project::{config, environment};

mod common;

#[test]
fn test_shell_hook_output() {
    assert!(shell::print_hook(Shell::Bash).is_ok());
    assert!(shell::print_hook(Shell::Zsh).is_ok());
    assert!(shell::print_hook(Shell::Fish).is_ok());
}

#[test]
fn test_env_export_shell_logic() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    let yaml = r#"
name: test-exports
config:
  local: true
environments:
  - name: test-env
    cmd: test
    run: ["echo"]
    env:
      FOO: BAR
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();

    let cfg = config::load().unwrap();

    assert!(environment::export_shell(Some("test"), &cfg, Shell::Bash).is_ok());
}

#[test]
fn test_env_export_shell_local_bin_path() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    let bin_dir = root.join(".zoi/pkgs/bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let yaml = r#"
name: test-local-bin
config:
  local: true
"#;
    fs::write(root.join("zoi.yaml"), yaml).unwrap();

    let cfg = config::load().unwrap();

    assert!(environment::export_shell(None, &cfg, Shell::Bash).is_ok());
}
