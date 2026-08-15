/// Command-line arguments for the `install` command.
pub mod args;
/// High-level installation orchestration.
pub mod orchestrator;

use anyhow::Result;
use zoi_project as project;

use crate::pkg::types;

/// The primary high-level orchestration for the `zoi install` command.
///
/// # Errors
///
/// Returns an error if the installation process fails.
pub fn run(
    sources: &[String],
    repo: Option<String>,
    force: bool,
    all_optional: bool,
    yes: bool,
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
    save: bool,
    build_type: Option<&str>,
    dry_run: bool,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>,
    build: bool,
    frozen: bool,
    explain: bool,
    plan_json: bool,
    retry: u32,
    verbose: bool,
    purl: bool,
    project_config: Option<project::config::ProjectConfig>
) -> Result<()> {
    let mut resolved_scope = scope.map(|s| match s {
        crate::cli::InstallScope::User => types::Scope::User,
        crate::cli::InstallScope::System => types::Scope::System,
        crate::cli::InstallScope::Project => types::Scope::Project
    });

    if local {
        resolved_scope = Some(types::Scope::Project);
    } else if global {
        resolved_scope = Some(types::Scope::User);
    }

    let scope = resolved_scope
        .unwrap_or_else(crate::pkg::utils::resolve_fallback_scope);

    let options = orchestrator::InstallOptions {
        scope,
        force,
        all_optional,
        yes,
        save,
        build_type,
        dry_run,
        plugin_manager,
        build,
        frozen,
        explain,
        plan_json,
        retry,
        verbose,
        purl,
        project_config
    };

    let orchestrator = orchestrator::Orchestrator::new(options);
    orchestrator.run(sources, repo)
}
