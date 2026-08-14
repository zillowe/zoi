use clap::Args;
use zoi_common::Runnable;

use crate::cli::InstallScope;
use crate::pkg::plugin::PluginManager;

/// Arguments for the `uninstall` command.
#[derive(Args, Debug, Clone)]
pub struct UninstallArgs {
    /// The package identifier(s).
    #[arg(
        value_name = "INST_PACKAGES",
        required = true,
        help = "Package identifier (e.g. @repo/name, path, or URL)"
    )]
    pub packages: Vec<String>,

    /// The scope to uninstall the package from
    #[arg(long, value_enum, conflicts_with_all = &["local", "global"])]
    pub scope: Option<InstallScope>,

    /// Uninstall packages from the current project (alias for --scope=project)
    #[arg(long, conflicts_with = "global")]
    pub local: bool,

    /// Uninstall packages globally for the current user (alias for
    /// --scope=user)
    #[arg(long)]
    pub global: bool,

    /// Remove the package from the project's zoi.yaml
    #[arg(long)]
    pub save: bool,

    /// Recursively remove dependencies that are no longer needed
    #[arg(short, long)]
    pub recursive: bool,

    /// Do not actually uninstall, just show what would be done
    #[arg(long)]
    pub dry_run: bool,

    /// Explain uninstall decisions (dependency impact and safety blocks)
    #[arg(long)]
    pub explain: bool,

    /// Emit machine-readable uninstall plan JSON
    #[arg(long)]
    pub plan_json: bool
}

impl Runnable for UninstallArgs {
    fn run(&self, yes: bool) -> anyhow::Result<()> {
        let plugin_manager = PluginManager::new()?;
        super::run(
            &self.packages,
            self.scope,
            self.local,
            self.global,
            self.save,
            yes,
            self.recursive,
            Some(&plugin_manager),
            self.explain,
            self.plan_json,
            self.dry_run
        )
    }
}
