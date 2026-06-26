use crate::pkg::local;
use anyhow::Result;
use colored::*;
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    let absolute_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };

    println!("Querying for file: {}", absolute_path.display());

    let installed_packages = local::get_installed_packages()?;

    for pkg in installed_packages {
        if pkg
            .installed_files
            .iter()
            .any(|f| Path::new(f) == absolute_path)
        {
            println!(
                "{} is owned by {} {}",
                absolute_path.display(),
                pkg.name.cyan(),
                pkg.version.yellow()
            );
            return Ok(());
        }
    }

    println!("No package owns file: {}", absolute_path.display());
    Ok(())
}
