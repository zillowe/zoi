//! Logic for the `upgrade` command.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::pkg;
use crate::pkg::types::{InstallManifest, Scope};

/// Run the upgrade command.
///
/// # Errors
///
/// Returns an error if the upgrade fails.
pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>
) -> Result<()> {
    if let Ok(exe) = std::env::current_exe()
        && let Ok(installed) = pkg::local::get_installed_packages()
        && let Some(managed) = find_managing_package(&exe, &installed)
    {
        if !force {
            print_zoi_managed_guidance(&managed);
            return Err(anyhow!("managed_by_package_manager"));
        }
        println!(
            "{}{}",
            "Warning: ".yellow().bold(),
            format!(
                "Forcing a self-upgrade of a binary managed by the {} package; \
                 Zoi's records for it will no longer match the binary until \
                 it is updated with 'zoi update'.",
                managed.source
            )
            .yellow()
        );
    }

    println!("{} Upgrading Zoi...", "::".bold().blue());

    match pkg::upgrade::run(branch, status, number, force, tag, custom_branch) {
        Ok(()) => {}
        Err(e) if e.to_string() == "already_on_latest" => {}
        Err(e) => return Err(e)
    }
    Ok(())
}

/// An installed Zoi package that owns the running `zoi` binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagingPackage {
    /// Source of the owning package, e.g. `@core/zoi`.
    pub source: String,
    /// Scope the package is installed in.
    pub scope: Scope,
    /// Source of the installed `zoid` package in the same scope and
    /// repository, if any, which should be updated together with Zoi.
    pub daemon_source: Option<String>
}

/// Returns the installed package whose recorded files include `exe`, i.e. a
/// `zoi` binary installed and tracked by Zoi itself (such as `@core/zoi` from
/// Zoidberg).
///
/// Such a binary must be updated with `zoi update`, which keeps the package's
/// manifest, file index and database record in step with the files.
/// `zoi upgrade` replaces the binary in place and would leave them stale.
#[must_use]
pub fn find_managing_package(
    exe: &Path,
    installed: &[InstallManifest]
) -> Option<ManagingPackage> {
    let exe = canonical(exe);
    let owner = installed.iter().find(|manifest| {
        manifest.name == "zoi" && owned_files(manifest).any(|f| f == exe)
    })?;

    let daemon_source = installed
        .iter()
        .find(|m| {
            m.name == "zoid" && m.scope == owner.scope && m.repo == owner.repo
        })
        .map(package_source);

    Some(ManagingPackage {
        source: package_source(owner),
        scope: owner.scope,
        daemon_source
    })
}

/// Returns the absolute paths of the files `manifest` installed.
fn owned_files(manifest: &InstallManifest) -> impl Iterator<Item = PathBuf> {
    let version_dir = pkg::local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )
    .ok();
    let scope = manifest.scope;
    manifest.installed_files.iter().filter_map(move |file| {
        let version_dir = version_dir.as_ref()?;
        pkg::utils::expand_placeholders(file, version_dir, scope)
            .ok()
            .map(|p| canonical(Path::new(&p)))
    })
}

/// Resolves symlinks where possible, so a binary reached through a link
/// still matches the recorded file.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Formats a manifest as an unversioned package source (`@repo/name`), so
/// `zoi update` moves it to the newest version.
fn package_source(manifest: &InstallManifest) -> String {
    format!("@{}/{}", manifest.repo, manifest.name)
}

/// Explains that this Zoi is a Zoi-managed package and how to update it.
fn print_zoi_managed_guidance(managed: &ManagingPackage) {
    let mut sources = managed.source.clone();
    if let Some(daemon) = &managed.daemon_source {
        sources.push(' ');
        sources.push_str(daemon);
    }
    let sudo = if managed.scope == Scope::System {
        "sudo "
    } else {
        ""
    };
    eprintln!(
        "{}{}",
        "Warning: ".yellow().bold(),
        format!(
            "This Zoi binary is managed by Zoi itself (package {}).",
            managed.source
        )
        .yellow()
    );
    eprintln!(
        "Update it with '{}', which keeps Zoi's records of the package in \
         step with the files.",
        format!("{sudo}zoi update {sources}").cyan()
    );
    eprintln!(
        "To self-upgrade the binary anyway, run with the '{}' flag.",
        "--force".cyan()
    );
}
