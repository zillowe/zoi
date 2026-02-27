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
        let (pkg, _, _, _, registry_handle) = resolve::resolve_package_and_version(name, true)?;

        let (manifest, scope) = if let Some(m) = local::is_package_installed(
            &pkg.name,
            request.sub_package.as_deref(),
            types::Scope::User,
        )? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(
            &pkg.name,
            request.sub_package.as_deref(),
            types::Scope::System,
        )? {
            (m, types::Scope::System)
        } else if let Some(m) = local::is_package_installed(
            &pkg.name,
            request.sub_package.as_deref(),
            types::Scope::Project,
        )? {
            (m, types::Scope::Project)
        } else {
            eprintln!(
                "{}: Package '{}' is not installed.",
                "Error".red().bold(),
                name
            );
            continue;
        };

        local::update_manifest_reason(
            &pkg.name,
            request.sub_package.as_deref(),
            scope,
            new_reason.clone(),
        )?;

        let handle = registry_handle
            .as_deref()
            .unwrap_or(&manifest.registry_handle);
        if let Ok(conn) = db::open_connection(handle) {
            let _ = db::update_package(
                &conn,
                &pkg,
                handle,
                Some(scope),
                request.sub_package.as_deref(),
                Some(&new_reason),
            );
        }
        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                &pkg,
                handle,
                Some(scope),
                request.sub_package.as_deref(),
                Some(&new_reason),
            );
        }

        let _ = recorder::update_package_reason(
            &pkg.name,
            request.sub_package.as_deref(),
            scope,
            new_reason.clone(),
        );

        println!(
            "Successfully marked '{}' as {}.",
            pkg.name.cyan(),
            reason_str
        );
    }

    Ok(())
}
