use crate::pkg::types;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub fn get_lib_dir(scope: types::Scope) -> Result<PathBuf, Box<dyn Error>> {
    match scope {
        types::Scope::User => Ok(home::home_dir().unwrap().join(".local/lib")),
        types::Scope::System => Ok(PathBuf::from("/usr/local/lib")),
    }
}

pub fn get_include_dir(scope: types::Scope) -> Result<PathBuf, Box<dyn Error>> {
    match scope {
        types::Scope::User => Ok(home::home_dir().unwrap().join(".local/include")),
        types::Scope::System => Ok(PathBuf::from("/usr/local/include")),
    }
}

pub fn get_pkgconfig_dir(scope: types::Scope) -> Result<PathBuf, Box<dyn Error>> {
    let lib_dir = get_lib_dir(scope)?;
    Ok(lib_dir.join("pkgconfig"))
}

pub fn install_files(source_dir: &Path, pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    let lib_dir = get_lib_dir(pkg.scope)?;
    let include_dir = get_include_dir(pkg.scope)?;
    fs::create_dir_all(&lib_dir)?;
    fs::create_dir_all(&include_dir)?;

    let source_include_dir = source_dir.join("include");
    let search_dir = if source_include_dir.exists() {
        source_include_dir
    } else {
        source_dir.to_path_buf()
    };

    for entry in WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            match extension {
                "so" | "dll" | "a" | "lib" => {
                    fs::copy(path, lib_dir.join(path.file_name().unwrap()))?;
                }
                "h" | "hpp" => {
                    let rel_path = path.strip_prefix(&search_dir).unwrap_or(path);
                    let dest_path = include_dir.join(rel_path);
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(path, dest_path)?;
                }
                _ => {}
            }
        }
    }

    if pkg.scope == types::Scope::System && cfg!(target_os = "linux") {
        println!("Running ldconfig...");
        let status = Command::new("sudo").arg("ldconfig").status()?;
        if !status.success() {
            println!("Warning: ldconfig failed.");
        }
    }

    Ok(())
}

pub fn install_pkg_config_file(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if let Some(pc_info) = &pkg.pkg_config {
        let pc_dir = get_pkgconfig_dir(pkg.scope)?;
        fs::create_dir_all(&pc_dir)?;
        let pc_file_path = pc_dir.join(format!("{}.pc", pkg.name));

        let version = pkg.version.as_deref().unwrap_or("0.0.0");
        let libdir = get_lib_dir(pkg.scope)?;
        let includedir = get_include_dir(pkg.scope)?;

        let prefix = match pkg.scope {
            types::Scope::User => home::home_dir().unwrap().join(".local"),
            types::Scope::System => PathBuf::from("/usr/local"),
        };

        let content = format!(
            "prefix={}\nexec_prefix=${{prefix}}\nlibdir={}\nincludedir={}\n\nName: {}\nDescription: {}\nVersion: {}\nLibs: -L${{libdir}} {}\nCflags: -I${{includedir}} {}\n",
            prefix.display(),
            libdir.display(),
            includedir.display(),
            pkg.name,
            pc_info.description,
            version,
            pc_info.libs,
            pc_info.cflags
        );

        fs::write(pc_file_path, content)?;
    }
    Ok(())
}
