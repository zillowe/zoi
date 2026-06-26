pub mod cli;
pub mod cmd;
pub mod pkg;
pub use zoi_project as project;
pub mod utils;

use anyhow::Result;

pub use zoi_core::cache;
pub use zoi_core::config;
pub use zoi_core::hash;
pub use zoi_core::lock;
pub use zoi_core::offline;
pub use zoi_core::pgp;
pub use zoi_core::pin;
pub use zoi_core::pkgdir;
pub use zoi_core::recorder;
pub use zoi_core::sysroot;
pub use zoi_core::types::{self, Scope};
pub use zoi_core::upgrade;
pub use zoi_hooks as hooks;
pub use zoi_lua as lua;
pub use zoi_purl as purl;
pub use zoi_resolver as resolve;
#[cfg(target_os = "linux")]
pub use zoi_sandbox as sandbox;
pub use zoi_telemetry as telemetry;

pub use pkg::local;
pub use pkg::mini_resolve;

#[derive(Debug, Clone, Default)]
pub struct SourceInstallOptions {
    pub repo: Option<String>,
    pub force: bool,
    pub all_optional: bool,
    pub yes: bool,
    pub scope_override: Option<Scope>,
    pub save: bool,
    pub build_type: Option<String>,
    pub dry_run: bool,
    pub build: bool,
    pub frozen_lockfile: bool,
}

fn to_install_scope(scope: Scope) -> cli::InstallScope {
    match scope {
        Scope::User => cli::InstallScope::User,
        Scope::System => cli::InstallScope::System,
        Scope::Project => cli::InstallScope::Project,
    }
}

pub fn install_sources(sources: &[String], options: &SourceInstallOptions) -> Result<()> {
    let plugin_manager = if crate::pkg::utils::is_mini_mode() {
        None
    } else {
        let pm = pkg::plugin::PluginManager::new()?;
        let _ = pm.load_all(options.yes);
        Some(pm)
    };

    let pm_ptr = plugin_manager.as_ref();

    cmd::install::run(
        sources,
        options.repo.clone(),
        options.force,
        options.all_optional,
        options.yes,
        options.scope_override.map(to_install_scope),
        false,
        false,
        options.save,
        options.build_type.clone(),
        options.dry_run,
        pm_ptr,
        options.build,
        options.frozen_lockfile,
        false,
        false,
        3,
        false,
        false,
    )
}

pub fn uninstall_package(package_name: &str, scope_override: Option<Scope>) -> Result<()> {
    zoi_uninstall::run(package_name, scope_override, false, false).map(|_| ())
}
