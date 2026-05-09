use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use zoi::pkg::mini_resolve;
use zoi::pkg::types::Scope;

#[derive(Parser)]
#[command(
    name = "zoi-mini",
    author,
    about = "Minimal Zoi package manager",
    version
)]
struct MiniCli {
    #[command(subcommand)]
    command: MiniCommands,

    #[arg(
        short = 'y',
        long,
        help = "Automatically answer yes to all prompts",
        global = true
    )]
    yes: bool,
}

#[derive(Subcommand)]
enum MiniCommands {
    /// Installs a package from Zoidberg registry
    Install {
        /// Package identifier (e.g. hello, @zillowe/hello)
        package: String,
    },
    /// Updates an existing installed package
    Update {
        /// Package name to update
        package: String,
    },
    /// Uninstalls an installed package
    Uninstall {
        /// Package name to uninstall
        package: String,
    },
    /// Lists all installed packages
    List,
}

fn main() -> Result<()> {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    let cli = MiniCli::parse();
    unsafe { std::env::set_var("ZOI_MINI_MODE", "1") };

    match cli.command {
        MiniCommands::Install { package } => install(&package, cli.yes),
        MiniCommands::Update { package } => update(&package, cli.yes),
        MiniCommands::Uninstall { package } => uninstall(&package, cli.yes),
        MiniCommands::List => list(),
    }
}

fn install(package_spec: &str, yes: bool) -> Result<()> {
    println!(
        "{} Resolving {} from Zoidberg...",
        "::".bold().blue(),
        package_spec.cyan()
    );

    let index = mini_resolve::fetch_registry_index()?;

    let request = zoi::pkg::resolve::parse_source_string(package_spec)?;
    let pkg_name = request.name;

    let pkg_index = index
        .packages
        .get(&pkg_name)
        .ok_or_else(|| anyhow!("Package '{}' not found in Zoidberg registry", pkg_name))?;

    if !mini_resolve::check_vulnerabilities(&pkg_name, pkg_index, &pkg_index.version)? {
        return Ok(());
    }

    let lua_url = mini_resolve::get_package_lua_url(&pkg_index.repo, &pkg_name);

    let options = zoi::SourceInstallOptions {
        yes,
        scope_override: Some(Scope::User),
        ..Default::default()
    };

    zoi::install_sources(&[lua_url], &options)
}

fn update(package_name: &str, yes: bool) -> Result<()> {
    println!(
        "{} Checking for updates for {}...",
        "::".bold().blue(),
        package_name.cyan()
    );

    let index = mini_resolve::fetch_registry_index()?;
    let pkg_index = index
        .packages
        .get(package_name)
        .ok_or_else(|| anyhow!("Package '{}' not found in Zoidberg registry", package_name))?;

    let lua_url = mini_resolve::get_package_lua_url(&pkg_index.repo, package_name);

    let options = zoi::SourceInstallOptions {
        yes,
        force: false,
        scope_override: Some(Scope::User),
        ..Default::default()
    };

    zoi::install_sources(&[lua_url], &options)
}

fn uninstall(package_name: &str, _yes: bool) -> Result<()> {
    zoi::uninstall_package(package_name, Some(Scope::User))
}

fn list() -> Result<()> {
    let installed = zoi::pkg::local::get_installed_packages()?;
    if installed.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("{:<20} {:<15} {:<15}", "Package", "Version", "Repo");
    println!("{}", "-".repeat(50));
    for pkg in installed {
        let name = if let Some(sub) = pkg.sub_package {
            format!("{}:{}", pkg.name, sub)
        } else {
            pkg.name
        };
        println!(
            "{:<20} {:<15} {:<15}",
            name.cyan(),
            pkg.version.yellow(),
            pkg.repo.dimmed()
        );
    }
    Ok(())
}
