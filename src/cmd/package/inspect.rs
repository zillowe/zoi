use anyhow::{Result, anyhow};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct InspectCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,

    /// Output the metadata as JSON
    #[arg(long)]
    pub json: bool,

    /// Validate as this target platform (defaults to current platform)
    #[arg(long)]
    pub platform: Option<String>,

    /// Override package version while inspecting
    #[arg(long)]
    pub version_override: Option<String>,
}

pub fn run(args: InspectCommand) -> Result<()> {
    let platform = match args.platform {
        Some(p) => p,
        None => crate::utils::get_platform()?,
    };

    let file_path = args.package_file.to_str().ok_or_else(|| {
        anyhow!(
            "Path contains invalid UTF-8 characters: {:?}",
            args.package_file
        )
    })?;

    let package = crate::pkg::lua::parser::parse_lua_package_for_platform(
        file_path,
        &platform,
        args.version_override.as_deref(),
        true,
    )?;

    if args.json {
        let json = serde_json::to_string_pretty(&package)?;
        println!("{}", json);
    } else {
        println!(
            "{} {} - {}",
            package.name,
            package.version.as_deref().unwrap_or("latest"),
            package.repo
        );
        println!("{}", package.description);
    }

    Ok(())
}
