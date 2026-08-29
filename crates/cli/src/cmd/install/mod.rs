/// Command-line arguments for the `install` command.
pub mod args;
/// High-level installation orchestration.
pub mod orchestrator;

use anyhow::Result;
use colored::Colorize;
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
    deps_only: bool,
    build_deps_only: bool,
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
    if plan_json && !dry_run {
        return Err(anyhow::anyhow!("--plan-json requires --dry-run"));
    }
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

    // --build-deps-only resolves the package definitions up front and turns
    // their declared build dependencies into the install sources. This works
    // for any source type (local .pkg.lua, registry package, URL, git).
    let effective_sources: Vec<String> = if build_deps_only {
        if sources.is_empty() && repo.is_none() {
            return Err(anyhow::anyhow!(
                "--build-deps-only requires at least one package source."
            ));
        }
        let platform = zoi_core::utils::get_platform()?;
        let mut all_build_deps: Vec<String> = Vec::new();
        for source in sources {
            let resolved = zoi_resolver::resolve::resolve_source(
                source,
                Some(scope),
                true,
                yes
            )?;
            let deps = crate::pkg::package::build::get_build_dependencies(
                resolved.path.as_path(),
                build_type,
                &platform,
                None,
                false
            )?
            .unwrap_or_default();
            println!(
                "{} Build dependencies for {source}: {}",
                "::".bold().blue(),
                if deps.is_empty() {
                    "none".dimmed().to_string()
                } else {
                    deps.join(", ")
                }
            );
            all_build_deps.extend(deps);
        }
        all_build_deps.sort();
        all_build_deps.dedup();
        if all_build_deps.is_empty() {
            println!(
                "{}",
                "Nothing to install - no build dependencies found.".green()
            );
            return Ok(());
        }
        all_build_deps
    } else {
        sources.to_vec()
    };

    let options = orchestrator::InstallOptions {
        scope,
        force,
        all_optional,
        yes,
        save,
        deps_only,
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
    orchestrator.run(&effective_sources, repo)
}
