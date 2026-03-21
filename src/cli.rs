use crate::cmd;
use crate::pkg::lock;
use crate::utils;
use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueHint, builder::styling,
};
use clap_complete::Shell;
use clap_complete::generate;
use colored::Colorize;
use std::io::{self};

// Development, Special, Public or Production
const BRANCH: &str = "Production";
const STATUS: &str = "Release";
const NUMBER: &str = "1.9.1";
const PKG_SOURCE_HELP: &str = "Package identifier (e.g. @repo/name, path, or URL)";

/// Zoi - The Universal Package Manager & Environment Setup Tool.
///
/// Part of the Zillowe Development Suite (ZDS), Zoi is designed to streamline
/// your development workflow by managing tools and project environments.
#[derive(Parser)]
#[command(name = "zoi", author, about, long_about = None, disable_version_flag = true,
    trailing_var_arg = true,
    color = ColorChoice::Auto,
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short = 'v',
        long = "version",
        help = "Print detailed version information"
    )]
    version_flag: bool,

    #[arg(
        short = 'y',
        long,
        help = "Automatically answer yes to all prompts",
        global = true
    )]
    yes: bool,

    #[arg(
        long = "root",
        help = "Operate on a different root directory",
        global = true,
        value_hint = ValueHint::DirPath
    )]
    pub root: Option<std::path::PathBuf>,

    #[arg(
        long = "offline",
        help = "Do not attempt to connect to the network",
        global = true
    )]
    pub offline: bool,

    #[arg(
        long = "pkg-dir",
        help = "Additional directory to search for .pkg.tar.zst archives",
        global = true,
        value_hint = ValueHint::DirPath
    )]
    pub pkg_dirs: Vec<std::path::PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Debug, Copy, PartialEq, Eq)]
pub enum SetupScope {
    User,
    System,
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum InstallScope {
    User,
    System,
    Project,
}

#[derive(Subcommand)]
enum Commands {
    /// Generates shell completion scripts
    #[command(hide = true)]
    GenerateCompletions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Generates man pages for zoi
    #[command(hide = true)]
    GenerateManual,

    /// Prints concise version and build information
    #[command(
        alias = "v",
        long_about = "Displays the version number, build status, branch, and commit hash. This is the same output provided by the -v and --version flags."
    )]
    Version,

    /// Shows detailed application information and credits
    #[command(
        long_about = "Displays the full application name, description, author, license, and homepage information."
    )]
    About,

    /// Displays detected operating system and architecture information
    #[command(
        long_about = "Detects and displays key system details, including the OS, CPU architecture, Linux distribution (if applicable), and available package managers."
    )]
    Info,

    /// Downloads or updates the package database from the remote repository
    #[command(
        alias = "sy",
        long_about = "Clones the official package database from GitLab to your local machine (~/.zoi/pkgs/db). If the database already exists, it verifies the remote URL and pulls the latest changes."
    )]
    Sync {
        #[command(subcommand)]
        command: Option<SyncCommands>,

        /// Show the full git output
        #[arg(short, long)]
        verbose: bool,

        /// Fallback to other mirrors if the default one fails
        #[arg(long)]
        fallback: bool,

        /// Do not check for installed package managers
        #[arg(long = "no-pm")]
        no_package_managers: bool,

        /// Do not attempt to set up shell completions after syncing
        #[arg(long)]
        no_shell_setup: bool,

        /// Download and index file lists for global search
        #[arg(short, long)]
        files: bool,
    },

    /// Lists installed or all available packages
    #[command(alias = "ls")]
    List {
        /// List all packages from the database, not just installed ones
        #[arg(short, long)]
        all: bool,
        /// Filter by registry handle (e.g. 'zoidberg')
        #[arg(long)]
        registry: Option<String>,
        /// Filter by repository (e.g. 'main', 'extra')
        #[arg(long)]
        repo: Option<String>,
        /// Filter by package type (package, app, collection, extension)
        #[arg(short = 't', long = "type")]
        package_type: Option<String>,
        /// List packages not found in any configured registry
        #[arg(short = 'm', long)]
        foreign: bool,
        /// List only package names (internal use for completions)
        #[arg(long, hide = true)]
        names: bool,
        /// List packages with descriptions for completion
        #[arg(long, hide = true)]
        completion: bool,
    },

    /// Shows detailed information about a package
    Show {
        #[arg(help = PKG_SOURCE_HELP)]
        package_name: String,
        /// Display the raw, unformatted package file
        #[arg(long)]
        raw: bool,
    },

    /// Pin a package to a specific version
    Pin {
        #[arg(help = PKG_SOURCE_HELP)]
        package: String,
        /// The version to pin the package to
        version: String,
    },

    /// Find which package provides a specific command or file
    Provides {
        /// The command or file path to search for
        term: String,
    },

    /// Visualize the dependency tree of a package
    Tree {
        #[arg(value_name = "PACKAGES", required = true, help = PKG_SOURCE_HELP)]
        packages: Vec<String>,
    },

    /// Unpin a package, allowing it to be updated
    Unpin {
        #[arg(help = PKG_SOURCE_HELP)]
        package: String,
    },

    /// Modify the installation reason of a package
    #[command(
        alias = "m",
        long_about = "Changes whether a package is considered explicitly installed or a dependency. Explicit packages are not removed by 'autoremove', while dependencies are if no other package requires them."
    )]
    Mark {
        #[arg(value_name = "PACKAGES", required = true, help = PKG_SOURCE_HELP)]
        packages: Vec<String>,

        /// Mark packages as dependencies
        #[arg(long, aliases = ["asdeps"])]
        as_dependency: bool,

        /// Mark packages as explicitly installed
        #[arg(long, aliases = ["asexpl"], conflicts_with = "as_dependency")]
        as_explicit: bool,
    },

    /// Updates one or more packages to their latest versions
    #[command(alias = "up")]
    Update {
        #[arg(value_name = "PACKAGES", help = PKG_SOURCE_HELP)]
        package_names: Vec<String>,

        /// Update all installed packages
        #[arg(long, conflicts_with = "package_names")]
        all: bool,

        /// Do not actually perform the update, just show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// Installs one or more packages from a name, local file, URL, or git repository
    #[command(alias = "i")]
    Install {
        #[arg(value_name = "SOURCES", value_hint = ValueHint::FilePath, help = PKG_SOURCE_HELP)]
        sources: Vec<String>,
        /// Install from a git repository (e.g. 'Zillowe/Hello', 'gl:Zillowe/Hello')
        #[arg(long, value_name = "REPO", conflicts_with = "sources")]
        repo: Option<String>,
        /// Force re-installation even if the package is already installed
        #[arg(long)]
        force: bool,
        /// Accept all optional dependencies
        #[arg(long)]
        all_optional: bool,
        /// The scope to install the package to
        #[arg(long, value_enum, conflicts_with_all = &["local", "global"])]
        scope: Option<InstallScope>,
        /// Install packages to the current project (alias for --scope=project)
        #[arg(long, conflicts_with = "global")]
        local: bool,
        /// Install packages globally for the current user (alias for --scope=user)
        #[arg(long)]
        global: bool,
        /// Save the package to the project's zoi.yaml
        #[arg(long)]
        save: bool,
        /// The type of package to build if building from source (e.g. 'source', 'pre-compiled').
        #[arg(long)]
        r#type: Option<String>,
        /// Do not actually perform the installation, just show what would be done
        #[arg(long)]
        dry_run: bool,

        /// Force building from source even if a pre-compiled archive is available in the registry
        #[arg(long, short = 'b')]
        build: bool,
    },

    /// Uninstalls one or more packages previously installed by Zoi
    #[command(
        aliases = ["un", "rm", "remove"],
        long_about = "Removes one or more packages' files from the Zoi store and deletes their symlinks from the bin directory. This command will fail if a package was not installed by Zoi."
    )]
    Uninstall {
        #[arg(value_name = "PACKAGES", required = true, help = PKG_SOURCE_HELP)]
        packages: Vec<String>,
        /// The scope to uninstall the package from
        #[arg(long, value_enum, conflicts_with_all = &["local", "global"])]
        scope: Option<InstallScope>,
        /// Uninstall packages from the current project (alias for --scope=project)
        #[arg(long, conflicts_with = "global")]
        local: bool,
        /// Uninstall packages globally for the current user (alias for --scope=user)
        #[arg(long)]
        global: bool,
        /// Remove the package from the project's zoi.yaml
        #[arg(long)]
        save: bool,
        /// Recursively remove dependencies that are no longer needed
        #[arg(short, long)]
        recursive: bool,
    },

    /// Execute a command defined in a local zoi.yaml file
    #[command(
        long_about = "Execute a command from zoi.yaml. If no command is specified, it will launch an interactive prompt to choose one."
    )]
    Run {
        /// The alias of the command to execute
        cmd_alias: Option<String>,
        /// Arguments to pass to the command
        args: Vec<String>,
    },

    /// Manage and set up project environments from a local zoi.yaml file
    #[command(
        long_about = "Checks for required packages and runs setup commands for a defined environment. If no environment is specified, it launches an interactive prompt."
    )]
    Env {
        /// The alias of the environment to set up
        env_alias: Option<String>,
    },

    /// Enter a development shell for the current project
    #[command(
        alias = "develop",
        long_about = "Loads the project configuration from zoi.yaml, ensures all required packages are installed locally, sets up environment variables (PATH, LD_LIBRARY_PATH, etc.), and drops you into a subshell."
    )]
    Dev {
        /// Command to run in the dev shell instead of an interactive shell
        #[arg(short, long)]
        run: Option<String>,
    },

    /// Upgrades the Zoi binary to the latest version
    #[command(
        alias = "ug",
        long_about = "Downloads the latest release from GitLab, verifies its checksum, and replaces the current executable."
    )]
    Upgrade {
        /// Force a full download
        #[arg(long)]
        force: bool,

        /// Upgrade to a specific git tag
        #[arg(long)]
        tag: Option<String>,

        /// Upgrade to the latest release of a specific branch (e.g. Prod, Pub)
        #[arg(long)]
        branch: Option<String>,
    },

    /// Removes packages that were installed as dependencies but are no longer needed
    Autoremove {
        /// Do not actually remove packages, just show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// Explains why a package is installed
    Why {
        #[arg(help = PKG_SOURCE_HELP)]
        package_name: String,
    },

    /// Find which package owns a file
    #[command(alias = "owns")]
    Owner {
        /// Path to the file
        #[arg(value_hint = ValueHint::FilePath)]
        path: std::path::PathBuf,
    },

    /// List all files owned by a package
    Files {
        #[arg(help = PKG_SOURCE_HELP)]
        package: String,
    },

    /// Shows the history of package operations
    History,

    /// Searches for packages by name or description
    #[command(
        alias = "s",
        long_about = "Searches for a case-insensitive term in the name, description, and tags of all available packages in the database. Filter by repo, type, or tags."
    )]
    Search {
        /// The term to search for (e.g. 'editor', 'cli')
        search_term: String,
        /// Filter by registry handle (e.g. 'zoidberg')
        #[arg(long)]
        registry: Option<String>,
        /// Filter by repository (e.g. 'main', 'extra')
        #[arg(long)]
        repo: Option<String>,
        /// Filter by package type (package, app, collection, extension)
        #[arg(long = "type")]
        package_type: Option<String>,
        /// Filter by tags (any match). Multiple via comma or repeated -t
        #[arg(short = 't', long = "tag", value_delimiter = ',', num_args = 1..)]
        tags: Option<Vec<String>>,
        /// Sort results by field (name, repo, type)
        #[arg(long, default_value = "name")]
        sort: String,
        /// Search for files provided by packages instead of package names
        #[arg(short, long)]
        files: bool,
        /// Open results in an interactive TUI
        #[arg(short = 'i', long)]
        interactive: bool,
    },

    /// Manage background services for installed packages
    #[command(alias = "svc")]
    Service(cmd::service::ServiceCommand),

    /// Set up shell completions or enter an ephemeral environment with specific packages
    #[command(
        long_about = "If a shell is provided, it installs completion scripts. If packages are provided via --package/-p, it enters a temporary subshell with those packages available in PATH."
    )]
    Shell {
        /// The shell to set up completions for
        #[arg(value_enum)]
        shell: Option<Shell>,
        /// The scope to apply the setup to (user or system-wide)
        #[arg(long, value_enum, default_value = "user")]
        scope: SetupScope,
        /// Packages to include in the ephemeral environment
        #[arg(short, long = "package", value_name = "PACKAGE")]
        packages: Vec<String>,
        /// Command to run in the ephemeral environment instead of an interactive shell
        #[arg(short, long)]
        run: Option<String>,
    },

    /// Download and execute a binary package without installing it
    #[command(
        alias = "x",
        long_about = "Downloads a binary to a temporary cache and executes it in a shell. All arguments after the package name are passed as arguments to the shell command."
    )]
    Exec {
        #[arg(value_name = "SOURCE", value_hint = ValueHint::FilePath, help = PKG_SOURCE_HELP)]
        source: String,

        /// Force execution from a fresh download, bypassing any cache.
        #[arg(long)]
        upstream: bool,

        /// Force execution from the cache, failing if the package is not cached.
        #[arg(long)]
        cache: bool,

        /// Force execution from the local project installation.
        #[arg(long)]
        local: bool,

        /// Arguments to pass to the executed command
        #[arg(value_name = "ARGS")]
        args: Vec<String>,
    },

    /// Clears the cache of downloaded package binaries
    Clean {
        /// Do not actually clear the cache, just show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// Manage Zoi's local cache
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },

    /// Manage package repositories
    #[command(
        aliases = ["repositories"],
        long_about = "Manages the list of package repositories used by Zoi.\n\nCommands:\n- add (alias: a): Add an official repo by name or clone from a git URL.\n- remove|rm: Remove a repo from active list (repo rm <name>).\n- list|ls: Show active repositories by default; use 'list all' to show all available repositories.\n- git: Manage cloned git repositories (git ls, git rm <repo-name>)."
    )]
    Repo(cmd::repo::RepoCommand),

    /// Manage telemetry settings (opt-in analytics)
    #[command(
        long_about = "Manage opt-in anonymous telemetry used to understand package popularity. Default is disabled."
    )]
    Telemetry {
        #[arg(value_enum)]
        action: TelemetryAction,
    },

    /// Create an application using a package template
    Create {
        #[arg(help = PKG_SOURCE_HELP)]
        source: String,
        /// The application name to substitute into template commands
        app_name: Option<String>,
    },

    /// Downgrade a package to a specific version from local cache or store
    #[command(
        alias = "dg",
        long_about = "Interactively choose and install an older version of a package from the local store or archive cache. This is useful if a recent update has introduced bugs or compatibility issues."
    )]
    Downgrade {
        #[arg(help = PKG_SOURCE_HELP)]
        package: String,
    },

    /// Manage Zoi extensions
    #[command(alias = "ext")]
    Extension(ExtensionCommand),

    /// Manage declarative system configuration (Arch Linux only)
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },

    /// Rollback a package to the previously installed version
    Rollback {
        #[arg(value_name = "PACKAGE", required_unless_present = "last_transaction", help = PKG_SOURCE_HELP)]
        package: Option<String>,

        /// Rollback the last transaction
        #[arg(long, conflicts_with = "package")]
        last_transaction: bool,
    },

    /// Shows a package's manual
    Man {
        #[arg(help = PKG_SOURCE_HELP)]
        package_name: String,
        /// Always look at the upstream manual even if it's downloaded
        #[arg(long)]
        upstream: bool,
        /// Print the manual to the terminal raw
        #[arg(long)]
        raw: bool,
    },

    /// Build, create, and manage Zoi packages
    #[command(alias = "pkg")]
    Package(cmd::package::PackageCommand),

    /// Manage PGP keys for package signature verification
    Pgp(cmd::pgp::PgpCommand),

    /// Helper commands for various tasks
    Helper(cmd::helper::HelperCommand),

    /// Checks for common issues and provides actionable suggestions
    Doctor,

    /// Audit installed or all packages for security vulnerabilities
    Audit {
        /// Show all vulnerabilities from the database, not just for installed packages
        #[arg(short, long)]
        all: bool,
        /// Filter by registry handle
        #[arg(long)]
        registry: Option<String>,
        /// Filter by repository
        #[arg(long)]
        repo: Option<String>,
    },

    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(clap::Parser, Debug)]
pub struct ExtensionCommand {
    #[command(subcommand)]
    pub command: ExtensionCommands,
}

#[derive(clap::Subcommand, Debug)]
pub enum ExtensionCommands {
    /// Add an extension
    Add {
        /// The name of the extension to add
        #[arg(required = true)]
        name: String,
    },
    /// Remove an extension
    Remove {
        /// The name of the extension to remove
        #[arg(required = true)]
        name: String,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum SystemCommands {
    /// Apply the declarative system configuration from /etc/zoi/zoi.lua
    Apply,
    /// Encrypt a phrase (e.g. password) for use in zoi.lua
    Encrypt {
        /// The phrase to encrypt
        phrase: String,
    },
}

#[derive(clap::Subcommand, Clone)]
pub enum SyncCommands {
    /// Add a new registry
    Add {
        /// URL of the registry to add
        url: String,
    },
    /// Remove a configured registry by its handle
    Remove {
        /// Handle of the registry to remove
        handle: String,
    },
    /// List configured registries
    #[command(alias = "ls")]
    List,
    /// Set the default registry URL
    Set {
        /// URL or keyword (default, github, gitlab, codeberg)
        url: String,
    },
}

#[derive(clap::Subcommand)]
pub enum CacheCommands {
    /// Add package archive(s) to the local cache
    Add {
        /// Path to the .pkg.tar.zst archive(s)
        #[arg(required = true)]
        files: Vec<std::path::PathBuf>,
    },
    /// Clear the local cache
    #[command(alias = "clean")]
    Clear {
        /// Do not actually clear the cache, just show what would be done
        #[arg(long)]
        dry_run: bool,
    },
    /// List all archives currently in the cache
    #[command(alias = "ls")]
    List,
}

#[derive(clap::ValueEnum, Clone)]
enum TelemetryAction {
    Status,
    Enable,
    Disable,
}

pub fn run() -> anyhow::Result<()> {
    let styles = styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default() | styling::Effects::BOLD)
        .usage(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::Green.on_default())
        .placeholder(styling::AnsiColor::Cyan.on_default());

    let commit: &str = option_env!("ZOI_COMMIT_HASH").unwrap_or("dev");
    let mut cmd = Cli::command().styles(styles);
    let matches = cmd.clone().get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(err) => {
            err.print()?;
            return Err(anyhow::anyhow!("Failed to parse arguments"));
        }
    };

    if let Some(root) = cli.root {
        crate::pkg::sysroot::set_sysroot(root);
    }

    let config = crate::pkg::config::read_config().unwrap_or_default();

    let is_offline = cli.offline || config.offline_mode;
    crate::pkg::offline::set_offline(is_offline);

    let mut all_pkg_dirs = cli.pkg_dirs;
    for dir in config.pkg_dirs {
        let path = std::path::PathBuf::from(dir);
        if !all_pkg_dirs.contains(&path) {
            all_pkg_dirs.push(path);
        }
    }
    crate::pkg::pkgdir::set_pkg_dirs(all_pkg_dirs);

    utils::check_path();

    if let Err(e) = crate::pkg::pgp::ensure_builtin_keys() {
        eprintln!(
            "{}: Failed to ensure builtin PGP keys: {}",
            "Warning".yellow(),
            e
        );
    }

    let plugin_manager = crate::pkg::plugin::PluginManager::new()?;
    if let Err(e) = plugin_manager.load_all() {
        eprintln!("{}: Failed to load plugins: {}", "Warning".yellow(), e);
    }

    if cli.version_flag {
        cmd::version::run(BRANCH, STATUS, NUMBER, commit);
        return Ok(());
    }

    if let Some(command) = cli.command {
        let needs_lock = matches!(
            command,
            Commands::Install { .. }
                | Commands::Uninstall { .. }
                | Commands::Update { .. }
                | Commands::Autoremove { .. }
                | Commands::Rollback { .. }
                | Commands::Package(_)
        );

        let _lock_guard = if needs_lock {
            Some(lock::acquire_lock()?)
        } else {
            None
        };

        let result = match command {
            Commands::GenerateCompletions { shell } => {
                let mut cmd = Cli::command();
                let bin_name = cmd.get_name().to_string();
                generate(shell, &mut cmd, bin_name, &mut io::stdout());
                Ok(())
            }
            Commands::GenerateManual => cmd::gen_man::run().map_err(Into::into),
            Commands::Version => {
                cmd::version::run(BRANCH, STATUS, NUMBER, commit);
                Ok(())
            }
            Commands::About => {
                cmd::about::run(BRANCH, STATUS, NUMBER, commit);
                Ok(())
            }
            Commands::Info => cmd::info::run(BRANCH, STATUS, NUMBER, commit),
            Commands::Sync {
                command,
                verbose,
                fallback,
                no_package_managers,
                no_shell_setup,
                files,
            } => {
                if let Some(cmd) = command {
                    match cmd {
                        SyncCommands::Add { url } => cmd::sync::add_registry(&url),
                        SyncCommands::Remove { handle } => cmd::sync::remove_registry(&handle),
                        SyncCommands::List => cmd::sync::list_registries(),
                        SyncCommands::Set { url } => cmd::sync::set_registry(&url),
                    }
                } else {
                    plugin_manager.trigger_hook("on_pre_sync", None)?;
                    let res = cmd::sync::run(
                        verbose,
                        fallback,
                        no_package_managers,
                        no_shell_setup,
                        files,
                    );
                    plugin_manager.trigger_hook("on_post_sync", None)?;
                    res
                }
            }
            Commands::List {
                all,
                registry,
                repo,
                package_type,
                foreign,
                names,
                completion,
            } => cmd::list::run(
                all,
                registry,
                repo,
                package_type,
                foreign,
                names,
                completion,
            ),
            Commands::Show { package_name, raw } => cmd::show::run(&package_name, raw),
            Commands::Pin { package, version } => cmd::pin::run(&package, &version),
            Commands::Provides { term } => cmd::provides::run(&term),
            Commands::Tree { packages } => cmd::tree::run(&packages),
            Commands::Unpin { package } => cmd::unpin::run(&package),
            Commands::Mark {
                packages,
                as_dependency,
                as_explicit,
            } => {
                if !as_dependency && !as_explicit {
                    let mut cmd = Cli::command();
                    if let Some(subcmd) = cmd.find_subcommand_mut("mark") {
                        subcmd.print_help()?;
                    }
                    Ok(())
                } else {
                    cmd::mark::run(&packages, as_dependency, as_explicit)
                }
            }
            Commands::Update {
                package_names,
                all,
                dry_run,
            } => {
                if !all && package_names.is_empty() {
                    let mut cmd = Cli::command();
                    if let Some(subcmd) = cmd.find_subcommand_mut("update") {
                        subcmd.print_help()?;
                    }
                    Ok(())
                } else {
                    cmd::update::run(all, &package_names, cli.yes, dry_run)
                }
            }
            Commands::Install {
                sources,
                repo,
                force,
                all_optional,
                scope,
                local,
                global,
                save,
                r#type,
                dry_run,
                build,
            } => cmd::install::run(
                &sources,
                repo,
                force,
                all_optional,
                cli.yes,
                scope,
                local,
                global,
                save,
                r#type,
                dry_run,
                &plugin_manager,
                build,
            ),
            Commands::Uninstall {
                packages,
                scope,
                local,
                global,
                save,
                recursive,
            } => cmd::uninstall::run(
                &packages,
                scope,
                local,
                global,
                save,
                cli.yes,
                recursive,
                &plugin_manager,
            ),
            Commands::Run { cmd_alias, args } => cmd::run::run(cmd_alias, args),
            Commands::Env { env_alias } => cmd::env::run(env_alias),
            Commands::Dev { run } => cmd::dev::run(run),
            Commands::Upgrade { force, tag, branch } => {
                match cmd::upgrade::run(BRANCH, STATUS, NUMBER, force, tag, branch) {
                    Ok(()) => {
                        println!(
                            "\n{}",
                            "Zoi upgraded successfully! Please restart your shell for changes to take effect."
                                .green()
                        );
                        println!(
                            "\n{}: https://github.com/Zillowe/Zoi/blob/main/CHANGELOG.md",
                            "Changelog".cyan().bold()
                        );
                        println!(
                            "\n{}: To update shell completions, run 'zoi shell <your-shell>'.",
                            "Hint".cyan().bold()
                        );
                    }
                    Err(e) if e.to_string() == "already_on_latest" => {}
                    Err(e) if e.to_string() == "managed_by_package_manager" => {}
                    Err(e) => return Err(e),
                }
                Ok(())
            }
            Commands::Autoremove { dry_run } => cmd::autoremove::run(cli.yes, dry_run),
            Commands::Why { package_name } => cmd::why::run(&package_name),
            Commands::Owner { path } => cmd::owner::run(&path),
            Commands::Files { package } => cmd::files::run(&package),
            Commands::History => cmd::history::run(),
            Commands::Search {
                search_term,
                registry,
                repo,
                package_type,
                tags,
                sort,
                files,
                interactive,
            } => cmd::search::run(
                search_term,
                registry,
                repo,
                package_type,
                tags,
                sort,
                files,
                interactive,
            ),
            Commands::Service(args) => cmd::service::run(args),
            Commands::Shell {
                shell,
                scope,
                packages,
                run,
            } => {
                if !packages.is_empty() {
                    cmd::shell::enter_ephemeral_shell(&packages, run, &plugin_manager)
                } else if let Some(s) = shell {
                    cmd::shell::run(s, scope)
                } else {
                    let mut cmd = Cli::command();
                    if let Some(subcmd) = cmd.find_subcommand_mut("shell") {
                        subcmd.print_help()?;
                    }
                    Ok(())
                }
            }
            Commands::Exec {
                source,
                upstream,
                cache,
                local,
                args,
            } => match cmd::exec::run(source, args, upstream, cache, local) {
                Ok(0) => Ok(()),
                Ok(exit_code) => Err(anyhow::anyhow!("process exited with code {}", exit_code)),
                Err(e) => Err(e),
            },
            Commands::Clean { dry_run } => cmd::clean::run(dry_run),
            Commands::Cache { command } => match command {
                CacheCommands::Add { files } => cmd::cache::add(&files),
                CacheCommands::Clear { dry_run } => cmd::cache::clear(dry_run),
                CacheCommands::List => cmd::cache::list(),
            },
            Commands::Repo(args) => cmd::repo::run(args),
            Commands::Telemetry { action } => {
                use cmd::telemetry::{TelemetryCommand, run};
                let cmd = match action {
                    TelemetryAction::Status => TelemetryCommand::Status,
                    TelemetryAction::Enable => TelemetryCommand::Enable,
                    TelemetryAction::Disable => TelemetryCommand::Disable,
                };
                run(cmd)
            }
            Commands::Create { source, app_name } => cmd::create::run(
                cmd::create::CreateCommand { source, app_name },
                cli.yes,
                &plugin_manager,
            ),
            Commands::Downgrade { package } => {
                cmd::downgrade::run(&package, cli.yes, &plugin_manager)
            }
            Commands::Extension(args) => cmd::extension::run(args, cli.yes, &plugin_manager),
            Commands::System { command } => match command {
                SystemCommands::Apply => crate::pkg::system::apply(cli.yes, &plugin_manager),
                SystemCommands::Encrypt { phrase } => {
                    match crate::pkg::system::encrypt_password(&phrase) {
                        Ok(enc) => {
                            println!("Encrypted value (safe for zoi.lua):\n{}", enc);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
            },
            Commands::Rollback {
                package,
                last_transaction,
            } => {
                if last_transaction {
                    cmd::rollback::run_transaction_rollback(cli.yes, &plugin_manager)
                } else if let Some(pkg) = package {
                    cmd::rollback::run(&pkg, cli.yes, &plugin_manager)
                } else {
                    Ok(())
                }
            }
            Commands::Man {
                package_name,
                upstream,
                raw,
            } => cmd::man::run(&package_name, upstream, raw),
            Commands::Package(args) => cmd::package::run(args),
            Commands::Pgp(args) => cmd::pgp::run(args),
            Commands::Helper(args) => cmd::helper::run(args),
            Commands::Doctor => cmd::doctor::run(),
            Commands::Audit {
                all,
                registry,
                repo,
            } => cmd::audit::run(all, registry, repo),
            Commands::External(args) => {
                let (cmd_name, cmd_args) = if args.is_empty() {
                    return Err(anyhow::anyhow!("No command specified"));
                } else {
                    (&args[0], args[1..].to_vec())
                };

                match plugin_manager.run_command(cmd_name, cmd_args) {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        let mut cmd = Cli::command();
                        cmd.print_help()?;
                        println!(
                            "\n{}: '{}' is not a Zoi command.",
                            "Error".red().bold(),
                            cmd_name
                        );

                        let plugin_cmds = plugin_manager.list_commands()?;
                        if !plugin_cmds.is_empty() {
                            println!("\n{}:", "Available Plugin Commands".cyan().bold());
                            for (pcmd, pdesc) in plugin_cmds {
                                if pdesc.is_empty() {
                                    println!("  {}", pcmd);
                                } else {
                                    println!("  {:<12} {}", pcmd, pdesc.dimmed());
                                }
                            }
                        }

                        std::process::exit(1);
                    }
                    Err(e) => Err(e),
                }
            }
        };

        if let Err(e) = result {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    } else {
        cmd.print_help()?;
    }
    Ok(())
}
