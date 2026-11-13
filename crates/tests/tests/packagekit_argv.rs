//! Regression tests for the argument vectors `zoid`'s `PackageKit` bridge
//! passes to `zoi`, and for related CLI flag definitions.
//!
//! The argv shapes below mirror `zoi_argv` in
//! `crates/daemon/src/packagekit.rs` (asserted there by
//! `zoi_argv_shapes_put_client_values_after_double_dash`). Every shape has to
//! parse with the real CLI definition, otherwise every `PackageKit`
//! transaction of that kind fails before doing anything.

use clap::CommandFactory;
use clap::error::ErrorKind;
use zoi::cli::{Cli, InstallScope, SetupScope};

/// Parses `zoi <args>` with the real CLI definition.
fn parse(args: &[&str]) -> clap::ArgMatches {
    let mut command_line = vec!["zoi"];
    command_line.extend_from_slice(args);
    Cli::command()
        .try_get_matches_from(command_line)
        .unwrap_or_else(|e| panic!("`zoi {}` must parse: {e}", args.join(" ")))
}

/// Returns the matches of the subcommand `name`.
fn sub<'a>(matches: &'a clap::ArgMatches, name: &str) -> &'a clap::ArgMatches {
    let (found, sub) = matches.subcommand().expect("a subcommand");
    assert_eq!(found, name);
    sub
}

/// Collects a positional list argument.
fn strings(matches: &clap::ArgMatches, id: &str) -> Vec<String> {
    matches
        .get_many::<String>(id)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

#[test]
fn packagekit_install_argv_parses_as_system_install() {
    let m = parse(&[
        "install",
        "--yes",
        "--scope",
        "system",
        "--",
        "firefox",
        "@core/vim"
    ]);
    assert!(m.get_flag("yes"));
    let install = sub(&m, "install");
    assert!(matches!(
        install.get_one::<InstallScope>("scope"),
        Some(InstallScope::System)
    ));
    assert_eq!(strings(install, "sources"), ["firefox", "@core/vim"]);
}

#[test]
fn packagekit_uninstall_argv_parses_as_system_uninstall() {
    let m =
        parse(&["uninstall", "--yes", "--scope", "system", "--", "firefox"]);
    let uninstall = sub(&m, "uninstall");
    assert!(matches!(
        uninstall.get_one::<InstallScope>("scope"),
        Some(InstallScope::System)
    ));
    assert_eq!(strings(uninstall, "packages"), ["firefox"]);
}

#[test]
fn packagekit_update_argv_parses_with_scope() {
    let m = parse(&["update", "--yes", "--scope", "system", "--", "firefox"]);
    let update = sub(&m, "update");
    assert!(matches!(
        update.get_one::<InstallScope>("scope"),
        Some(InstallScope::System)
    ));
    assert_eq!(strings(update, "package_names"), ["firefox"]);
    assert!(!update.get_flag("all"));
}

#[test]
fn packagekit_update_all_argv_parses_with_scope() {
    let m = parse(&["update", "--yes", "--scope", "system", "--all"]);
    let update = sub(&m, "update");
    assert!(update.get_flag("all"));
    assert!(matches!(
        update.get_one::<InstallScope>("scope"),
        Some(InstallScope::System)
    ));
}

#[test]
fn packagekit_refresh_argv_parses_as_system_sync() {
    for args in [
        &["sync", "--yes", "--scope", "system"][..],
        &["sync", "--yes", "--scope", "system", "--force"][..]
    ] {
        let m = parse(args);
        let sync = sub(&m, "sync");
        assert!(matches!(
            sync.get_one::<SetupScope>("scope"),
            Some(SetupScope::System)
        ));
        assert_eq!(sync.get_flag("force"), args.contains(&"--force"));
    }
}

#[test]
fn values_after_double_dash_are_never_options() {
    // A hostile package id must arrive as a positional, not as a flag.
    let m = parse(&[
        "install",
        "--yes",
        "--scope",
        "system",
        "--",
        "--repo=evil",
        "-x"
    ]);
    let install = sub(&m, "install");
    assert_eq!(strings(install, "sources"), ["--repo=evil", "-x"]);
    assert!(install.get_one::<String>("repo").is_none());
}

#[test]
fn update_scope_is_optional_and_validated() {
    let m = parse(&["update", "firefox"]);
    assert!(sub(&m, "update").get_one::<InstallScope>("scope").is_none());

    let err = Cli::command()
        .try_get_matches_from(["zoi", "update", "--scope", "galaxy", "x"])
        .expect_err("an unknown scope must be rejected");
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
}

#[test]
fn sync_frozen_requires_local() {
    let err = Cli::command()
        .try_get_matches_from(["zoi", "sync", "--frozen"])
        .expect_err("--frozen without --local must be rejected");
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);

    let m = parse(&["sync", "--local", "--frozen"]);
    let sync = sub(&m, "sync");
    assert!(sync.get_flag("local"));
    assert!(sync.get_flag("frozen"));
}

#[test]
fn main_help_lists_the_helper_command() {
    let help = Cli::command().render_help().to_string();
    assert!(
        help.contains("helper"),
        "the visible `helper` command must be listed in the main help"
    );
}
