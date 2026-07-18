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
        .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    let help_text = err.to_string();
    assert!(help_text.contains("Advanced Package Manager"));
}

#[test]
fn test_cli_parsing_install_flags() {
    let mut cmd = Cli::command();
    let matches = cmd
        .try_get_matches_from_mut(vec!["zoi", "install", "--local", "--frozen", "--yes"])
        .expect("Parsing install flags failed");

    let (subcommand, sub_matches) = matches.subcommand().unwrap();
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
        .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}
