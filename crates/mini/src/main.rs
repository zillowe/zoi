mod mini_resolve;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use zoi_core::types::Scope;
use zoi_resolver::resolve::parse_source_string;

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
    #[command(alias = "i")]
    Install {
        /// Package identifier (e.g. hello, @zillowe/hello)
        package: String,
    },
    /// Updates an existing installed package
    #[command(alias = "up")]
    Update {
        /// Package name to update
        package: String,
    },
    /// Uninstalls an installed package
    #[command(alias = "un")]
    Uninstall {
        /// Package name to uninstall
        package: String,
    },
    /// Lists all installed packages
    #[command(alias = "ls")]
    List,
}

fn main() -> Result<()> {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    let args: Vec<String> = std::env::args().collect();
    let program_name = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if !program_name.is_empty()
        && program_name != "zoi-mini"
        && !program_name.starts_with("zoi-")
        && !program_name.contains("target")
    {
        if let Err(e) = zoi_cli::pkg::shim::run_shim(program_name, args[1..].to_vec(), None, None) {
            eprintln!("{}: {}", "Shim Error".red().bold(), e);
            std::process::exit(1);
        }
        return Ok(());
    }

    let cli = MiniCli::parse();
    // SAFETY: We are setting this variable at the very start of main, before any
    // background threads are spawned, which is safe on Unix-like systems.
    unsafe { std::env::set_var("ZOI_MINI_MODE", "1") };

    let result = match cli.command {
        MiniCommands::Install { package } => install(&package, cli.yes),
        MiniCommands::Update { package } => update(&package, cli.yes),
        MiniCommands::Uninstall { package } => uninstall(&package, cli.yes),
        MiniCommands::List => list(),
    };

    if let Err(e) = result {
        eprintln!("\n{} {}", "Error:".red().bold(), e);
        eprintln!("\n  If the installation failed, it may require the full Zoi suite.");
        eprintln!(
            "  To install Zoi, please visit the documentation: {}",
            "https://zillowe.qzz.io/docs/zds/zoi".cyan()
        );
        std::process::exit(1);
    }
    Ok(())
}

fn install(package_spec: &str, yes: bool) -> Result<()> {
    println!(
        "{} Resolving {} from Zoidberg...",
        "::".bold().blue(),
        package_spec.cyan()
    );

    let index = mini_resolve::fetch_registry_index()?;
    let repo_config = mini_resolve::fetch_registry_config()?;

    let request = parse_source_string(package_spec)?;
    let pkg_name = request.name;

    let (repo, pkg_info_opt) = if let Some(explicit_repo) = request.repo {
        let match_in_index = index
            .packages
            .get(&pkg_name)
            .filter(|p| p.repo == explicit_repo);
        (explicit_repo, match_in_index)
    } else {
        let pkg_info = index
            .packages
            .get(&pkg_name)
            .ok_or_else(|| anyhow!("Package '{}' not found in Zoidberg registry", pkg_name))?;

        let is_repo_active = repo_config
            .repos
            .iter()
            .any(|r| r.name == pkg_info.repo && r.active);

        if !is_repo_active {
            return Err(anyhow!(
                "Package '{}' is in repository '{}' which is not active by default. Use explicit naming like '@{}/{}'",
                pkg_name,
                pkg_info.repo,
                pkg_info.repo,
                pkg_name
            ));
        }
        (pkg_info.repo.clone(), Some(pkg_info))
    };

    if let Some(pkg_info) = pkg_info_opt
        && !mini_resolve::check_vulnerabilities(&pkg_name, pkg_info, &pkg_info.version)?
    {
        return Ok(());
    }

    let source = zoi_cli::pkg::local::package_source_string("zoidberg", &repo, &pkg_name, None, "");
    let normalized_source = source.trim_end_matches('@');

    let options = zoi_cli::SourceInstallOptions {
        yes,
        scope_override: Some(Scope::User),
        ..Default::default()
    };

    zoi_cli::install_sources(&[normalized_source.to_string()], &options)
}

fn update(package_name: &str, yes: bool) -> Result<()> {
    println!(
        "{} Checking for updates for {}...",
        "::".bold().blue(),
        package_name.cyan()
    );

    let index = mini_resolve::fetch_registry_index()?;
    let repo_config = mini_resolve::fetch_registry_config()?;

    let request = parse_source_string(package_name)?;
    let pkg_name = request.name;

    let (repo, pkg_info_opt) = if let Some(explicit_repo) = request.repo {
        let match_in_index = index
            .packages
            .get(&pkg_name)
            .filter(|p| p.repo == explicit_repo);
        (explicit_repo, match_in_index)
    } else {
        let pkg_info = index
            .packages
            .get(&pkg_name)
            .ok_or_else(|| anyhow!("Package '{}' not found in Zoidberg registry", pkg_name))?;

        let is_repo_active = repo_config
            .repos
            .iter()
            .any(|r| r.name == pkg_info.repo && r.active);

        if !is_repo_active {
            return Err(anyhow!(
                "Package '{}' is in repository '{}' which is not active by default. Use explicit naming like '@{}/{}'",
                pkg_name,
                pkg_info.repo,
                pkg_info.repo,
                pkg_name
            ));
        }
        (pkg_info.repo.clone(), Some(pkg_info))
    };

    if let Some(pkg_info) = pkg_info_opt
        && !mini_resolve::check_vulnerabilities(&pkg_name, pkg_info, &pkg_info.version)?
    {
        return Ok(());
    }

    let source = zoi_cli::pkg::local::package_source_string("zoidberg", &repo, &pkg_name, None, "");
    let normalized_source = source.trim_end_matches('@');

    let options = zoi_cli::SourceInstallOptions {
        yes,
        force: false,
        scope_override: Some(Scope::User),
        ..Default::default()
    };

    zoi_cli::install_sources(&[normalized_source.to_string()], &options)
}

fn uninstall(package_name: &str, _yes: bool) -> Result<()> {
    zoi_cli::uninstall_package(package_name, Some(Scope::User))
}

fn list() -> Result<()> {
    let installed = zoi_cli::pkg::local::get_installed_packages()?;
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
