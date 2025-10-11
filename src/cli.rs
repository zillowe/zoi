use crate::cmd;
use crate::utils;
use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueHint,
    builder::PossibleValue, builder::TypedValueParser, builder::styling,
};
use clap_complete::Shell;
use clap_complete::generate;
use std::io::{self};

// Development, Special, Public or Production
const BRANCH: &str = "Development";
const STATUS: &str = "Release";
const NUMBER: &str = "1.0.0";

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
}

#[derive(Clone, Debug)]
struct PackageValueParser;

impl TypedValueParser for PackageValueParser {
    type Value = String;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        Ok(value.to_string_lossy().into_owned())
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(
            utils::get_all_packages_for_completion()
                .into_iter()
                .map(|pkg| {
                    let help = if pkg.description.is_empty() {
                        pkg.repo
                    } else {
                        format!("[{}] {}", pkg.repo, pkg.description)
                    };
                    PossibleValue::new(Box::leak(pkg.display.into_boxed_str()) as &'static str)
                        .help(Box::leak(help.into_boxed_str()) as &'static str)
                }),
        ))
    }
}

#[derive(Clone, Debug)]
struct PkgOrPathParser;

impl TypedValueParser for PkgOrPathParser {
    type Value = String;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        Ok(value.to_string_lossy().into_owned())
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(
            utils::get_all_packages_for_completion()
                .into_iter()
                .map(|pkg| {
                    let help = if pkg.description.is_empty() {
                        pkg.repo
                    } else {
                        format!("[{}] {}", pkg.repo, pkg.description)
                    };
                    PossibleValue::new(Box::leak(pkg.display.into_boxed_str()) as &'static str)
                        .help(Box::leak(help.into_boxed_str()) as &'static str)
                }),
        ))
    }
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum SetupScope {
    User,
    System,
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

    /// Checks for essential third-party command-line tools
    #[command(
        long_about = "Verifies that all required dependencies (like git) are installed and available in the system's PATH. This is useful for diagnostics."
    )]
    Check,

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
    },

    /// Lists installed or all available packages
    #[command(alias = "ls")]
    List {
        /// List all packages from the database, not just installed ones
        #[arg(short, long)]
        all: bool,
        /// Filter by repository (e.g. 'main', 'extra')
        #[arg(long)]
        repo: Option<String>,
        /// Filter by package type (package, service, config, collection, extension, library)
        #[arg(short = 't', long = "type")]
        package_type: Option<String>,
    },

    /// Shows detailed information about a package
    Show {
        /// The name of the package to show
        #[arg(value_parser = PackageValueParser, hide_possible_values = true)]
        package_name: String,
        /// Display the raw, unformatted package file
        #[arg(long)]
        raw: bool,
    },

    /// Pin a package to a specific version
    Pin {
        /// The name of the package to pin
        #[arg(value_parser = PackageValueParser, hide_possible_values = true)]
        package: String,
        /// The version to pin the package to
        version: String,
    },

    /// Unpin a package, allowing it to be updated
    Unpin {
        /// The name of the package to unpin
        #[arg(value_parser = PackageValueParser, hide_possible_values = true)]
        package: String,
    },

    /// Updates one or more packages to their latest versions
    #[command(alias = "up")]
    Update {
        /// The name(s) of the package(s) to update
        #[arg(value_name = "PACKAGES", value_parser = PackageValueParser, hide_possible_values = true)]
        package_names: Vec<String>,

        /// Update all installed packages
        #[arg(long, conflicts_with = "package_names")]
        all: bool,
    },

    /// Installs one or more packages from a name, local file, URL, or git repository
    #[command(alias = "i")]
    Install {
        /// Package names, local paths, or URLs to .pkg.lua files
        #[arg(value_name = "SOURCES", value_hint = ValueHint::FilePath, value_parser = PkgOrPathParser, hide_possible_values = true)]
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
        /// The scope to install the package to (user or system-wide)
        #[arg(long, value_enum)]
        scope: Option<SetupScope>,
    },

    /// Builds and installs one or more packages from a name, local file, or URL
    #[command(
        long_about = "Builds one or more packages from various sources using the 'source' installation method:\n- A package name from the database (e.g. 'vim')\n- A local .pkg.lua file (e.g. './my-package.pkg.lua')\n- A URL pointing to a raw .pkg.lua file"
    )]
    Build {
        /// Package names, local paths, or URLs to .pkg.lua files
        #[arg(value_name = "SOURCES", required = true, value_hint = ValueHint::FilePath, value_parser = PkgOrPathParser, hide_possible_values = true)]
        sources: Vec<String>,
        /// Force re-installation even if the package is already installed
        #[arg(long)]
        force: bool,
    },

    /// Uninstalls one or more packages previously installed by Zoi
    #[command(
        aliases = ["un", "rm", "remove"],
        long_about = "Removes one or more packages' files from the Zoi store and deletes their symlinks from the bin directory. This command will fail if a package was not installed by Zoi."
    )]
    Uninstall {
        /// One or more packages to uninstall
        #[arg(value_name = "PACKAGES", required = true, value_parser = PackageValueParser, hide_possible_values = true)]
        packages: Vec<String>,
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

    /// Upgrades the Zoi binary to the latest version
    #[command(
        alias = "ug",
        long_about = "Downloads the latest release from GitLab, verifies its checksum, and replaces the current executable."
    )]
    Upgrade {
        /// Force a full download, skipping the patch-based upgrade
        #[arg(long)]
        full: bool,

        /// Force the upgrade even if the version is the same
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
    Autoremove,

    /// Explains why a package is installed
    Why {
        /// The name of the package to inspect
        #[arg(value_parser = PackageValueParser, hide_possible_values = true)]
        package_name: String,
    },

    /// Searches for packages by name or description
    #[command(
        alias = "s",
        long_about = "Searches for a case-insensitive term in the name, description, and tags of all available packages in the database. Filter by repo, type, or tags."
    )]
    Search {
        /// The term to search for (e.g. 'editor', 'cli')
        search_term: String,
        /// Filter by repository (e.g. 'main', 'extra')
        #[arg(long)]
        repo: Option<String>,
        /// Filter by package type (package, service, config, collection, extension, library)
        #[arg(long = "type")]
        package_type: Option<String>,
        /// Filter by tags (any match). Multiple via comma or repeated -t
        #[arg(short = 't', long = "tag", value_delimiter = ',', num_args = 1..)]
        tags: Option<Vec<String>>,
    },

    /// Installs completion scripts for a given shell
    Shell {
        /// The shell to install completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Configures the shell environment for Zoi
    #[command(
        long_about = "Adds the Zoi binary directory to your shell's PATH to make Zoi packages' executables available as commands."
    )]
    Setup {
        /// The scope to apply the setup to (user or system-wide)
        #[arg(long, value_enum, default_value = "user")]
        scope: SetupScope,
    },

    /// Download and execute a binary package without installing it
    #[command(
        alias = "x",
        long_about = "Downloads a binary to a temporary cache and executes it in a shell. All arguments after the package name are passed as arguments to the shell command."
    )]
    Exec {
        /// Package name, local path, or URL to execute
        #[arg(value_name = "SOURCE", value_parser = PkgOrPathParser, value_hint = ValueHint::FilePath, hide_possible_values = true)]
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
    Clean,

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
        /// Package name, @repo/name, local .pkg.lua path, or URL
        source: String,
        /// The application name to substitute into template commands
        app_name: String,
    },

    /// Manage Zoi extensions
    #[command(alias = "ext")]
    Extension(ExtensionCommand),

    /// Rollback a package to the previously installed version
    Rollback {
        /// The name of the package to rollback
        #[arg(value_name = "PACKAGE", value_parser = PackageValueParser, hide_possible_values = true)]
        package: String,
    },

    /// Shows a package's manual
    Man {
        /// The name of the package to show the manual for
        #[arg(value_parser = PackageValueParser, hide_possible_values = true)]
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

#[derive(clap::ValueEnum, Clone)]
enum TelemetryAction {
    Status,
    Enable,
    Disable,
}

pub fn run() {
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
            err.print().unwrap();
            std::process::exit(1);
        }
    };

    utils::check_path();

    if cli.version_flag {
        cmd::version::run(BRANCH, STATUS, NUMBER, commit);
        return;
    }

    if let Some(command) = cli.command {
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
            Commands::Info => {
                cmd::info::run(BRANCH, STATUS, NUMBER, commit);
                Ok(())
            }
            Commands::Check => {
                cmd::check::run();
                Ok(())
            }
            Commands::Sync {
                command,
                verbose,
                fallback,
                no_package_managers,
                no_shell_setup,
            } => {
                if let Some(cmd) = command {
                    match cmd {
                        SyncCommands::Add { url } => cmd::sync::add_registry(&url),
                        SyncCommands::Remove { handle } => cmd::sync::remove_registry(&handle),
                        SyncCommands::List => cmd::sync::list_registries(),
                        SyncCommands::Set { url } => cmd::sync::set_registry(&url),
                    }
                } else {
                    cmd::sync::run(verbose, fallback, no_package_managers, no_shell_setup);
                }
                Ok(())
            }
            Commands::List {
                all,
                repo,
                package_type,
            } => {
                let _ = cmd::list::run(all, repo, package_type);
                Ok(())
            }
            Commands::Show { package_name, raw } => {
                cmd::show::run(&package_name, raw);
                Ok(())
            }
            Commands::Pin { package, version } => {
                cmd::pin::run(&package, &version);
                Ok(())
            }
            Commands::Unpin { package } => {
                cmd::unpin::run(&package);
                Ok(())
            }
            Commands::Update { package_names, all } => {
                if !all && package_names.is_empty() {
                    let mut cmd = Cli::command();
                    if let Some(subcmd) = cmd.find_subcommand_mut("update") {
                        subcmd.print_help().unwrap();
                    }
                } else {
                    cmd::update::run(all, &package_names, cli.yes);
                }
                Ok(())
            }
            Commands::Install {
                sources,
                repo,
                force,
                all_optional,
                scope,
            } => {
                cmd::install::run(&sources, repo, force, all_optional, cli.yes, scope);
                Ok(())
            }
            Commands::Build { sources, force } => {
                cmd::build::run(&sources, force, cli.yes);
                Ok(())
            }
            Commands::Uninstall { packages } => {
                cmd::uninstall::run(&packages);
                Ok(())
            }
            Commands::Run { cmd_alias, args } => {
                cmd::run::run(cmd_alias, args);
                Ok(())
            }
            Commands::Env { env_alias } => {
                cmd::env::run(env_alias);
                Ok(())
            }
            Commands::Upgrade {
                full,
                force,
                tag,
                branch,
            } => {
                cmd::upgrade::run(BRANCH, STATUS, NUMBER, full, force, tag, branch);
                Ok(())
            }
            Commands::Autoremove => {
                cmd::autoremove::run(cli.yes);
                Ok(())
            }
            Commands::Why { package_name } => cmd::why::run(&package_name),
            Commands::Search {
                search_term,
                repo,
                package_type,
                tags,
            } => cmd::search::run(search_term, repo, package_type, tags),
            Commands::Shell { shell } => {
                cmd::shell::run(shell);
                Ok(())
            }
            Commands::Setup { scope } => {
                cmd::setup::run(scope);
                Ok(())
            }
            Commands::Exec {
                source,
                upstream,
                cache,
                local,
                args,
            } => {
                cmd::exec::run(source, args, upstream, cache, local);
                Ok(())
            }
            Commands::Clean => {
                cmd::clean::run();
                Ok(())
            }
            Commands::Repo(args) => {
                cmd::repo::run(args);
                Ok(())
            }
            Commands::Telemetry { action } => {
                use cmd::telemetry::{TelemetryCommand, run};
                let cmd = match action {
                    TelemetryAction::Status => TelemetryCommand::Status,
                    TelemetryAction::Enable => TelemetryCommand::Enable,
                    TelemetryAction::Disable => TelemetryCommand::Disable,
                };
                run(cmd);
                Ok(())
            }
            Commands::Create { source, app_name } => {
                cmd::create::run(cmd::create::CreateCommand { source, app_name }, cli.yes);
                Ok(())
            }
            Commands::Extension(args) => cmd::extension::run(args, cli.yes),
            Commands::Rollback { package } => cmd::rollback::run(&package, cli.yes),
            Commands::Man {
                package_name,
                upstream,
                raw,
            } => cmd::man::run(&package_name, upstream, raw),
            Commands::Package(args) => {
                cmd::package::run(args);
                Ok(())
            }
            Commands::Pgp(args) => cmd::pgp::run(args),
            Commands::Helper(args) => cmd::helper::run(args),
        };

        if let Err(e) = result {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    } else {
        cmd.print_help().unwrap();
    }
}
