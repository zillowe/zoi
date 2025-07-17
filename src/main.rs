use clap::{CommandFactory, Parser, Subcommand};

mod cmd;
mod pkg;
mod project;
mod utils;

const BRANCH: &str = "Development";
const STATUS: &str = "Beta";
const NUMBER: &str = "2.0.22";

/// Zoi - The Universal Package Manager & Environment Setup Tool.
///
/// Part of the Zillowe Development Suite (ZDS), Zoi is designed to streamline
/// your development workflow by managing tools and project environments.
#[derive(Parser)]
#[command(author, about, long_about, disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short = 'v',
        long = "version",
        help = "Print detailed version information"
    )]
    version_flag: bool,
}

#[derive(Subcommand)]
enum Commands {
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
    Sync,

    /// Lists installed or all available packages
    List {
        /// Use 'all' to list all packages from the database
        #[arg(value_name = "all")]
        all: Option<String>,
    },

    /// Displays detailed information about a package
    Show {
        package_name: String,
        /// Display the raw package file content
        #[arg(long)]
        raw: bool,
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
}

fn main() {
    let commit: &str = option_env!("ZOI_COMMIT_HASH").unwrap_or("dev");
    let cli = Cli::parse();

    if cli.version_flag {
        cmd::version::run(BRANCH, STATUS, NUMBER, commit);
        return;
    }

    if let Some(command) = cli.command {
        match command {
            Commands::Version => cmd::version::run(BRANCH, STATUS, NUMBER, commit),
            Commands::About => cmd::about::run(BRANCH, STATUS, NUMBER, commit),
            Commands::Info => cmd::info::run(),
            Commands::Check => cmd::check::run(),
            Commands::Sync => cmd::sync::run(),
            Commands::List { all } => cmd::list::run(all.is_some()),
            Commands::Show { package_name, raw } => cmd::show::run(&package_name, raw),
            Commands::Update { package_name } => cmd::update::run(&package_name),
            Commands::Install { source, force } => cmd::install::run(&source, force),
            Commands::Build { source } => cmd::build::run(&source),
            Commands::Uninstall { package_name } => cmd::uninstall::run(&package_name),
            Commands::Run { cmd_alias } => cmd::run::run(cmd_alias),
            Commands::Env { env_alias } => cmd::env::run(env_alias),
            Commands::Clone {
                source,
                target_directory,
            } => cmd::clone::run(source, target_directory),
        }
    } else {
        Cli::command().print_help().unwrap();
    }
}
