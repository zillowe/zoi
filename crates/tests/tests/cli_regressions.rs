//! Regression tests for CLI fixes: downgrade cache parsing, telemetry
//! minimisation, `--root` with daemon-backed system commands, and `zoi update`
//! failure reporting.

use std::path::Path;

use tempfile::tempdir;
use zoi::cmd::downgrade::version_from_cached_archive;
use zoi::cmd::system::{SystemSubcommands, check_root_supported};
use zoi_telemetry::sanitize_registry_url;

mod common;

#[test]
fn downgrade_parses_versions_from_cached_archive_names() {
    let platform = "linux-amd64";
    let parse = |file: &str, name: &str| {
        version_from_cached_archive(file, name, platform)
    };

    assert_eq!(
        parse("vim-9.1.0-linux-amd64.zpa", "vim").as_deref(),
        Some("9.1.0")
    );
    // Pre-release versions contain dashes and must survive intact.
    assert_eq!(
        parse("vim-9.1.0-rc.1-linux-amd64.zpa", "vim").as_deref(),
        Some("9.1.0-rc.1")
    );
    // Package names containing dashes.
    assert_eq!(
        parse("git-lfs-3.5.1-linux-amd64.zpa", "git-lfs").as_deref(),
        Some("3.5.1")
    );
}

#[test]
fn downgrade_ignores_other_packages_and_platforms() {
    let platform = "linux-amd64";
    let parse = |file: &str, name: &str| {
        version_from_cached_archive(file, name, platform)
    };

    // `git` must not pick up `git-lfs` archives just because of the prefix.
    assert_eq!(parse("git-lfs-3.5.1-linux-amd64.zpa", "git"), None);
    // Another platform's archive is not installable here.
    assert_eq!(parse("vim-9.1.0-macos-arm64.zpa", "vim"), None);
    // Sub-package archives and non-archives are skipped.
    assert_eq!(parse("vim-9.1.0-linux-amd64.doc.zpa", "vim"), None);
    assert_eq!(parse("vim-9.1.0-linux-amd64.zsa", "vim"), None);
    // The old `split('-')` logic returned "linux" style garbage for these.
    assert_eq!(parse("vim-linux-amd64.zpa", "vim"), None);
}

#[test]
fn telemetry_registry_url_keeps_only_scheme_and_host() {
    // Assembled at runtime so secret scanners don't flag the fixture.
    let userinfo = ["user", "not-a-real-secret"].join(":");
    let url = format!(
        "https://{userinfo}@gitlab.example.com:8443/org/db.git?token=abc#x"
    );
    assert_eq!(sanitize_registry_url(&url), "https://gitlab.example.com");
    assert_eq!(
        sanitize_registry_url("https://gitlab.com/zillowe/zoidberg.git"),
        "https://gitlab.com"
    );
    assert_eq!(
        sanitize_registry_url("git@github.com:org/private-registry.git"),
        "ssh://github.com"
    );
    assert_eq!(
        sanitize_registry_url("ssh://deploy@[::1]:22/repo.git"),
        "ssh://[::1]"
    );
    assert_eq!(sanitize_registry_url("/home/alice/registries/db"), "local");
    assert_eq!(sanitize_registry_url("file:///srv/db.git"), "local");
    assert_eq!(sanitize_registry_url("C:\\registries\\db"), "local");
    assert_eq!(sanitize_registry_url(""), "unknown");
    assert_eq!(sanitize_registry_url("unknown"), "unknown");
}

#[test]
fn root_is_refused_for_daemon_backed_system_commands() {
    let root = Some(Path::new("/mnt/target"));

    for command in [
        SystemSubcommands::Apply {
            file: "/etc/zoi/system.lua".to_string()
        },
        SystemSubcommands::List,
        SystemSubcommands::Status,
        SystemSubcommands::Rollback { id: 1 },
        SystemSubcommands::Pin { id: 1 },
        SystemSubcommands::Unpin { id: 1 }
    ] {
        let err = check_root_supported(&command, root)
            .expect_err("--root must be refused for daemon commands");
        assert!(err.to_string().contains("--root /mnt/target"), "{err}");
        // Without --root the same command is allowed through.
        assert!(check_root_supported(&command, None).is_ok());
    }
}

#[test]
fn update_of_failing_packages_exits_with_an_error() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    ctx.set_env_var("XDG_DATA_HOME", root.join(".local/share"));
    ctx.set_env_var("XDG_CONFIG_HOME", root.join(".config"));
    common::TestContextGuard::set_sysroot(root.clone());
    common::TestContextGuard::set_offline(true);

    // Nothing is installed in the sandbox, so the update cannot succeed. It
    // used to print a summary with `failed: 1` and still return Ok, which
    // made `zoi update` (and every PackageKit update) exit 0.
    let result = zoi::cmd::update::run(
        false,
        &["fix-cli-definitely-missing-package".to_string()],
        true,
        false,
        false,
        false,
        false,
        false
    );
    assert!(
        result.is_err(),
        "a failed targeted update must be reported as an error"
    );
}
