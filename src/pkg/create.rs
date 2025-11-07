use crate::pkg::{package, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::fs;
use std::path::Path;
use tar::Archive;
use tempfile::Builder;
use zstd::stream::read::Decoder as ZstdDecoder;

fn install_app_from_archive(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    println!(
        "Extracting app to '{}'...",
        destination_dir.display().to_string().cyan()
    );
    let file = fs::File::open(archive_path)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);

    let temp_extract_dir = Builder::new().prefix("zoi-create-extract-").tempdir()?;

    archive.unpack(temp_extract_dir.path())?;

    let create_pkg_dir = temp_extract_dir.path().join("data/createpkgdir");

    if !create_pkg_dir.exists() {
        return Err(anyhow!(
            "Archive is not a valid app package: missing 'data/createpkgdir' directory."
        ));
    }

    utils::copy_dir_all(&create_pkg_dir, destination_dir)?;

    Ok(())
}

pub fn run(source: &str, app_name: Option<String>, yes: bool) -> Result<()> {
    let (pkg, _, _, pkg_lua_path, _) = resolve::resolve_package_and_version(source, false)?;

    if pkg.package_type != types::PackageType::App {
        return Err(anyhow!(
            "Package '{}' is not of type 'app'. Use 'zoi install' for packages and collections.",
            pkg.name
        ));
    }

    let dest_name = app_name.unwrap_or_else(|| pkg.name.clone());
    let app_dir = Path::new(&dest_name);

    if app_dir.exists() {
        if app_dir.is_dir() {
            if fs::read_dir(app_dir)?.next().is_some() {
                println!(
                    "{}",
                    format!(
                        "Warning: Directory '{}' already exists and is not empty.",
                        &dest_name
                    )
                    .yellow()
                );
                if !utils::ask_for_confirmation("Do you want to continue?", yes) {
                    return Err(anyhow!("Operation aborted by user."));
                }
            }
        } else {
            return Err(anyhow!(
                "A file with the name '{}' already exists.",
                &dest_name
            ));
        }
    }

    println!(
        "Creating app '{}' using template '{}'...",
        dest_name.cyan(),
        pkg.name.green()
    );

    let build_dir = Builder::new().prefix("zoi-create-build-").tempdir()?;

    package::build::run(
        &pkg_lua_path,
        "source",
        &[utils::get_platform()?],
        None,
        Some(build_dir.path()),
        pkg.version.as_deref(),
        None,
        false,
    )?;

    let archive_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        pkg.name,
        pkg.version.as_deref().unwrap_or(""),
        utils::get_platform()?,
    );
    let archive_path = build_dir.path().join(archive_filename);

    if !archive_path.exists() {
        return Err(anyhow!("Build failed to produce an archive."));
    }

    install_app_from_archive(&archive_path, app_dir)?;

    println!("\n{}", "App created successfully.".green());

    Ok(())
}
