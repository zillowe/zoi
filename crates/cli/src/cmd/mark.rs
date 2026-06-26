use crate::pkg::{db, local, recorder, resolve, types};
use anyhow::{Result, anyhow};
use colored::*;

pub fn run(package_names: &[String], as_dependency: bool, as_explicit: bool) -> Result<()> {
    let new_reason = if as_dependency {
        types::InstallReason::Dependency {
            parent: "manual".to_string(),
        }
    } else if as_explicit {
        types::InstallReason::Direct
    } else {
        return Err(anyhow!(
            "Either --as-dependency or --as-explicit must be provided."
        ));
    };

    let reason_str = if as_dependency {
        "dependency".cyan()
    } else {
        "explicit".green()
    };

    for name in package_names {
        println!("Marking '{}' as {}...", name.blue().bold(), reason_str);

        let request = resolve::parse_source_string(name)?;
        let (pkg, _, _, _, registry_handle, _) =
            resolve::resolve_package_and_version(name, true, false)?;
        let installed_source = if let Some(sub) = request.sub_package.as_deref() {
            format!(
                "#{}@{}/{}:{}",
                registry_handle.as_deref().unwrap_or("local"),
                pkg.repo,
                pkg.name,
                sub
            )
        } else {
            format!(
                "#{}@{}/{}",
                registry_handle.as_deref().unwrap_or("local"),
                pkg.repo,
                pkg.name
            )
        };
        let installed_request = resolve::parse_source_string(&installed_source)?;
        let mut candidates = Vec::new();
        for scope in [
            types::Scope::User,
            types::Scope::System,
            types::Scope::Project,
        ] {
            candidates.extend(local::find_installed_manifests_matching(
                &installed_request,
                scope,
            )?);
        }

        let manifest =
            match crate::cmd::installed_select::choose_installed_manifest(name, &candidates, false)
            {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    continue;
                }
            };
        let scope = manifest.scope;

        local::update_manifest_reason(&manifest, new_reason.clone())?;

        let handle = registry_handle
            .as_deref()
            .unwrap_or(&manifest.registry_handle);
        let mut db_pkg = pkg.clone();
        db_pkg.repo = manifest.repo.clone();
        db_pkg.scope = manifest.scope;
        db_pkg.sub_package = manifest.sub_package.clone();
        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                &db_pkg,
                handle,
                Some(scope),
                manifest.sub_package.as_deref(),
                Some(&new_reason),
            );
        }

        let _ = recorder::update_package_reason(&manifest, new_reason.clone());

        println!(
            "Successfully marked '{}' as {}.",
            pkg.name.cyan(),
            reason_str
        );
    }

    Ok(())
}
