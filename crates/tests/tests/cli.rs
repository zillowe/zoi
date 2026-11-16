//! Integration tests for CLI argument parsing and versioning.

use clap::CommandFactory;
use zoi::cli::Cli;

#[test]
fn test_cli_parsing_version() {
    let mut cmd = Cli::command();
    let matches = cmd
        .try_get_matches_from_mut(vec!["zoi", "--version"])
        .expect("Parsing --version failed");
    assert!(matches.get_flag("version_flag"));
}

#[test]
fn test_cli_parsing_help() {
    let mut cmd = Cli::command();
    let err = cmd
        .try_get_matches_from_mut(vec!["zoi", "--help"])
        .expect_err("unwrap_err failed");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    let help_text = err.to_string();
    assert!(help_text.contains("Advanced Package Manager"));
}

#[test]
fn test_cli_parsing_install_flags() {
    let mut cmd = Cli::command();
    let matches = cmd
        .try_get_matches_from_mut(vec![
            "zoi", "install", "--local", "--frozen", "--yes",
        ])
        .expect("Parsing install flags failed");

    let (subcommand, sub_matches) =
        matches.subcommand().expect("unwrap failed");
    assert_eq!(subcommand, "install");

    assert!(sub_matches.get_flag("local"));
    assert!(sub_matches.get_flag("frozen"));

    assert!(matches.get_flag("yes"));
}

#[test]
fn test_cli_parsing_conflicting_flags() {
    let mut cmd = Cli::command();
    let err = cmd
        .try_get_matches_from_mut(vec!["zoi", "install", "--local", "--global"])
        .expect_err("unwrap_err failed");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn bare_zoi_is_a_help_request_not_a_usage_error() {
    let err = Cli::command()
        .try_get_matches_from(vec!["zoi"])
        .expect_err("bare zoi shows help");
    assert!(zoi::cli::is_bare_invocation(&err, 1));

    // A missing subcommand after other arguments is still a usage error.
    let err = Cli::command()
        .try_get_matches_from(vec!["zoi", "system"])
        .expect_err("missing system subcommand");
    assert!(!zoi::cli::is_bare_invocation(&err, 2));
}
