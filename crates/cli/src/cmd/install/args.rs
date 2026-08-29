use clap::Args;
use zoi_common::Runnable;

use crate::cli::InstallScope;
use crate::pkg::plugin::PluginManager;

/// Arguments for the `install` command.
#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    /// The package source identifier(s).
    #[arg(
        value_name = "ALL_SOURCES",
        help = "Package identifier (e.g. @repo/name, #git@repo/name, path, or \
                URL)"
    )]
    pub sources: Vec<String>,

    /// Install from a git repository (e.g. 'zillowe/hello',
    /// 'gh:zillowe/hello')
    #[arg(long, value_name = "REPO", conflicts_with = "sources")]
    pub repo: Option<String>,

    /// Force re-installation even if the package is already installed
    #[arg(long)]
    pub force: bool,

    /// Accept all optional dependencies
    #[arg(long)]
    pub all_optional: bool,

    /// The scope to install the package to
    #[arg(long, value_enum, conflicts_with_all = &["local", "global"])]
    pub scope: Option<InstallScope>,

    /// Install packages to the current project (alias for --scope=project)
    #[arg(long, conflicts_with = "global")]
    pub local: bool,

    /// Install packages globally for the current user (alias for --scope=user)
    #[arg(long)]
    pub global: bool,

    /// Save the package to the project's zoi.yaml
    #[arg(long, conflicts_with = "deps_only")]
    pub save: bool,

    /// Install only the dependencies of the given packages, not the packages
    /// themselves
    #[arg(long, conflicts_with_all = &["save", "build_deps_only"])]
    pub deps_only: bool,

    /// Install only the build dependencies of the given packages
    #[arg(long, conflicts_with_all = &["save", "deps_only"])]
    pub build_deps_only: bool,

    /// The type of package to build if building from source (e.g. 'source',
    /// 'pre-compiled').
    #[arg(long)]
    pub r#type: Option<String>,

    /// Do not actually perform the installation, just show what would be done
    #[arg(long)]
    pub dry_run: bool,

    /// Force building from source even if a pre-compiled archive is available
    /// in the registry
    #[arg(long, short = 'b')]
    pub build: bool,

    /// Enforce zoi.lock exactly (project install only, no lockfile updates)
    #[arg(long)]
    pub frozen: bool,

    /// Explain dependency selection and install decisions
    #[arg(long)]
    pub explain: bool,

    /// Emit machine-readable install plan JSON
    #[arg(long, requires = "dry_run")]
    pub plan_json: bool,

    /// Retry failed downloads this many times (minimum 1)
    #[arg(long, default_value_t = 3)]
    pub retry: u32,

    /// Show additional install details (package origins, preflight info)
    #[arg(long, short)]
    pub verbose: bool,

    /// Use PURL (Package URL) specification for resolving packages
    #[arg(long)]
    pub purl: bool
}

impl Runnable for InstallArgs {
    fn run(&self, yes: bool) -> anyhow::Result<()> {
        let plugin_manager = PluginManager::new()?;

        super::run(
            &self.sources,
            self.repo.clone(),
            self.force,
            self.all_optional,
            yes,
            self.scope,
            self.local,
            self.global,
            self.save,
            self.deps_only,
            self.build_deps_only,
            self.r#type.as_deref(),
            self.dry_run,
            Some(&plugin_manager),
            self.build,
            self.frozen,
            self.explain,
            self.plan_json,
            self.retry,
            self.verbose,
            self.purl,
            None
        )
    }
}
