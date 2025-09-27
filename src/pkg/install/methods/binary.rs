use crate::pkg::{
    install::{
        util::{download_file_with_progress, get_filename_from_url},
        verification::{verify_checksum, verify_signatures},
    },
    library, types,
};
use anyhow::Result;
use colored::*;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

pub fn handle_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let url = &method.url;

    let downloaded_bytes = download_file_with_progress(url)?;

    let file_to_verify = get_filename_from_url(url);
    verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

    if pkg.package_type == types::PackageType::Library {
        let lib_dir = library::get_lib_dir(pkg.scope)?;
        fs::create_dir_all(&lib_dir)?;
        let dest_path = lib_dir.join(get_filename_from_url(url));
        fs::write(&dest_path, downloaded_bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))?;
        }

        if pkg.scope == types::Scope::System && cfg!(target_os = "linux") {
            println!("Running ldconfig...");
            let status = Command::new("sudo").arg("ldconfig").status()?;
            if !status.success() {
                println!("Warning: ldconfig failed.");
            }
        }

        println!("{}", "Library file installed successfully.".green());
        return Ok(());
    }

    let store_dir = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name)
        .join("bin");
    fs::create_dir_all(&store_dir)?;

    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", pkg.name)
    } else {
        pkg.name.clone()
    };
    let bin_path = store_dir.join(&binary_filename);
    let mut dest = File::create(&bin_path)?;
    dest.write_all(&downloaded_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
    }

    #[cfg(unix)]
    {
        let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
        fs::create_dir_all(&symlink_dir)?;
        let symlink_path = symlink_dir.join(&pkg.name);

        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        std::os::unix::fs::symlink(&bin_path, symlink_path)?;
    }

    #[cfg(windows)]
    {
        println!(
            "{}",
            "Binary installed. Please add ~/.zoi/pkgs/bin to your PATH manually.".yellow()
        );
    }

    println!("{}", "Binary installed successfully.".green());
    Ok(())
}
