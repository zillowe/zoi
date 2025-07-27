use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::generate;
use clap_complete::Shell;
use std::io;
mod cmd;
mod pkg;
mod project;
mod utils;

// Production or Development
const BRANCH: &str = "Development";
const STATUS: &str = "Beta";
const NUMBER: &str = "3.0.0";

/// Zoi - The Universal Package Manager & Environment Setup Tool.
///
/// Part of the Zillowe Development Suite (ZDS), Zoi is designed to streamline
/// your development workflow by managing tools and project environments.
#[derive(Parser)]
#[command(author, about, long_about = None, disable_version_flag = true,
    trailing_var_arg = true,
)]
struct Cli {
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

#[derive(Subcommand)]
enum Commands {
    /// Generates shell completion scripts
    #[command(hide = true)]
    GenerateCompletions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

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
        long_about = "Clones the official package database from GitLab to your local machine (~/.zoi/pkgs/db). If the database already exists, it verifies the remote URL and pulls the latest changes."
    )]
    Sync {
        /// Show the full git output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Lists installed or all available packages
    List {
        /// Use 'all' to list all packages from the database and/or filter by repo
        #[arg()]
        args: Vec<String>,
    },

    /// Displays detailed information about a package
    Show {
        package_name: String,
        /// Display the raw package file content
        #[arg(long)]
        raw: bool,
    },

    /// Pins a package to a specific version
    Pin {
        /// The package to pin, e.g. "vim@1.8.0"
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Unpins a package
    Unpin {
        /// The package to unpin
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Updates an installed package to the latest version
    Update { package_name: String },

    /// Installs a package from a name, local file, or URL
    #[command(
        long_about = "Installs a package from various sources:\n- A package name from the database (e.g. 'vim')\n- A local .pkg.yaml file (e.g. './my-package.pkg.yaml')\n- A URL pointing to a raw .pkg.yaml file"
    )]
    Install {
        /// Package name, local path, or URL to a .pkg.yaml file
        #[arg(value_name = "SOURCE")]
        source: String,
        /// Reinstall the package even if it's already present
        #[arg(long)]
        force: bool,
        /// Interactive mode to select installation method
        #[arg(short, long)]
        interactive: bool,
    },

    /// Builds and installs a package from a name, local file, or URL
    #[command(
        long_about = "Builds a package from various sources using the 'source' installation method:\n- A package name from the database (e.g. 'vim')\n- A local .pkg.yaml file (e.g. './my-package.pkg.yaml')\n- A URL pointing to a raw .pkg.yaml file"
    )]
    Build {
        /// Package name, local path, or URL to a .pkg.yaml file
        #[arg(value_name = "SOURCE")]
        source: String,
    },

    /// Uninstalls a package previously installed by Zoi
    #[command(
        long_about = "Removes a package's files from the Zoi store and deletes its symlink from the bin directory. This command will fail if the package was not installed by Zoi."
    )]
    Uninstall {
        /// The name of the package to uninstall
        package_name: String,
    },

    /// Execute a command defined in a local zoi.yaml file
    #[command(
        long_about = "Execute a command from zoi.yaml. If no command is specified, it will launch an interactive prompt to choose one."
    )]
    Run {
        /// The alias of the command to execute
        cmd_alias: Option<String>,
    },

    /// Manage and set up project environments from a local zoi.yaml file
    #[command(
        long_about = "Checks for required packages and runs setup commands for a defined environment. If no environment is specified, it launches an interactive prompt."
    )]
    Env {
        /// The alias of the environment to set up
        env_alias: Option<String>,
    },

    /// Clones the source code repository of a package
    #[command(
        long_about = "Finds a package's definition, extracts its git repository URL, and clones it into a new directory."
    )]
    Clone {
        /// Package name, local path, or URL to resolve the git repo from
        #[arg(value_name = "SOURCE")]
        source: String,

        /// Optional directory to clone into. Defaults to the package name.
        #[arg(value_name = "TARGET_DIRECTORY")]
        target_directory: Option<String>,
    },

    /// Upgrades the Zoi binary to the latest version
    #[command(
        long_about = "Downloads the latest release from GitLab, verifies its checksum, and replaces the current executable."
    )]
    Upgrade,

    /// Removes packages that were installed as dependencies but are no longer needed
    Autoremove,

    /// Searches for packages by name or description
    #[command(
        long_about = "Searches for a case-insensitive term in the name and description of all available packages in the database."
    )]
    Search {
        /// The term to search for (e.g. 'editor', 'cli') and an optional repo to search in (e.g. '@main')
        #[arg()]
        args: Vec<String>,
    },

    /// Download and execute a binary package without installing it
    #[command(
        long_about = "Downloads a binary to a temporary cache and runs it. All arguments after the package name are passed directly to the executed command."
    )]
    Exec {
        /// Package name, local path, or URL to execute
        #[arg(value_name = "SOURCE")]
        source: String,

        /// Arguments to pass to the executed command
        #[arg(value_name = "ARGS")]
        args: Vec<String>,
    },

    /// Manage package repositories
    #[command(
        long_about = "Manages the list of package repositories that Zoi uses to find and install packages. By default, Zoi is configured with 'main' and 'extra' repositories.\n\nCommands:\n- add: Adds a new repository from the available sources. Can be interactive.\n- remove: Deletes a repository from the active list.\n- list: Shows all currently active repositories.\n- list all: Displays all available repositories and their status (active/inactive)."
    )]
    Repo(cmd::repo::RepoCommand),
}

fn main() {
    let commit: &str = option_env!("ZOI_COMMIT_HASH").unwrap_or("dev");
    let cli = Cli::parse();

    utils::check_path();

    if cli.version_flag {
        cmd::version::run(BRANCH, STATUS, NUMBER, commit);
        return;
    }

    if let Some(command) = cli.command {
        match command {
            Commands::GenerateCompletions { shell } => {
                let mut cmd = Cli::command();
                let bin_name = cmd.get_name().to_string();
                generate(shell, &mut cmd, bin_name, &mut io::stdout());
            }
            Commands::Version => cmd::version::run(BRANCH, STATUS, NUMBER, commit),
            Commands::About => cmd::about::run(BRANCH, STATUS, NUMBER, commit),
            Commands::Info => cmd::info::run(BRANCH, STATUS, NUMBER, commit),
            Commands::Check => cmd::check::run(),
            Commands::Sync { verbose } => cmd::sync::run(verbose),
            Commands::List { args } => cmd::list::run(args),
            Commands::Show { package_name, raw } => cmd::show::run(&package_name, raw),
            Commands::Pin { package } => cmd::pin::run(&package),
            Commands::Unpin { package } => cmd::unpin::run(&package),
            Commands::Update { package_name } => cmd::update::run(&package_name, cli.yes),
            Commands::Install {
                source,
                force,
                interactive,
            } => cmd::install::run(&source, force, interactive, cli.yes),
            Commands::Build { source } => cmd::build::run(&source, cli.yes),
            Commands::Uninstall { package_name } => cmd::uninstall::run(&package_name),
            Commands::Run { cmd_alias } => cmd::run::run(cmd_alias),
            Commands::Env { env_alias } => cmd::env::run(env_alias),
            Commands::Clone {
                source,
                target_directory,
            } => cmd::clone::run(source, target_directory, cli.yes),
            Commands::Upgrade => cmd::upgrade::run(),
            Commands::Autoremove => cmd::autoremove::run(cli.yes),
            Commands::Search { args } => cmd::search::run(args),
            Commands::Exec { source, args } => cmd::exec::run(source, args),
            Commands::Repo(args) => cmd::repo::run(args),
        }
    } else {
        Cli::command().print_help().unwrap();
    }
}
