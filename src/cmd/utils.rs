use crate::pkg::{local, resolve};
use anyhow::Result;

pub fn expand_split_packages(package_names: &[String], action: &str) -> Result<Vec<String>> {
    let mut expanded_names = Vec::new();
    let installed_packages = local::get_installed_packages()?;

    for name in package_names {
        let request = resolve::parse_source_string(name)?;
        let mut was_expanded = false;

        if request.sub_package.is_none()
            && let Ok((pkg, _, _, _, _)) = resolve::resolve_package_and_version(name, true)
            && pkg.sub_packages.is_some()
        {
            let mut installed_subs = Vec::new();
            for manifest in &installed_packages {
                if manifest.name == pkg.name
                    && let Some(sub) = &manifest.sub_package
                {
                    installed_subs.push(sub.clone());
                }
            }

            if !installed_subs.is_empty() {
                println!(
                    "'{}' is a split package. {} all installed sub-packages: {}",
                    name,
                    action,
                    installed_subs.join(", ")
                );
                for sub in installed_subs {
                    expanded_names.push(format!("{}:{}", name, sub));
                }
                was_expanded = true;
            }
        }

        if !was_expanded {
            expanded_names.push(name.clone());
        }
    }

    Ok(expanded_names)
}
