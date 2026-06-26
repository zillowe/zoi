use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
pub use zoi_core::dependency::Dependency;
use zoi_core::types;
use zoi_core::utils;

include!(concat!(env!("OUT_DIR"), "/generated_managers.rs"));

pub fn parse_dependency_string(dep_str: &str) -> Result<Dependency<'_>> {
    zoi_core::dependency::parse_dependency_string(dep_str, |m| {
        m == "zoi" || m == "native" || MANAGERS.contains_key(m)
    })
}

type ZoiUninstaller = dyn Fn(&str) -> Result<()>;

pub fn uninstall_dependency(dep_str: &str, zoi_uninstaller: &ZoiUninstaller) -> Result<()> {
    let dep = parse_dependency_string(dep_str)?;
    println!(
        "-> Attempting to uninstall dependency: {} via {}",
        dep.package.cyan(),
        dep.manager.yellow()
    );

    if dep.manager == "zoi" {
        return zoi_uninstaller(dep.package);
    }

    if let Some(pm_commands) = MANAGERS.get(dep.manager) {
        let mut uninstall_cmd = pm_commands.uninstall.replace("{package}", dep.package);

        if pm_commands.sudo_uninstall && !utils::is_admin() {
            if utils::command_exists("sudo") {
                uninstall_cmd = format!("sudo {}", uninstall_cmd);
            } else {
                eprintln!(
                    "{}: sudo is required for '{}' but not found. Attempting to run without sudo...",
                    "Warning".yellow(),
                    dep.manager
                );
            }
        }

        println!("Running uninstall command: {}", uninstall_cmd.italic());
        utils::run_shell_command(&uninstall_cmd)
    } else {
        Err(anyhow!(
            "Unknown or unsupported package manager for uninstall: {}",
            dep.manager
        ))
    }
}

pub fn collect_dependencies_for_group(
    group: &types::DependencyGroup,
    sub_package_name: Option<&str>,
    dep_type: Option<&str>,
    yes: bool,
    all_optional: bool,
) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let mut deps = Vec::new();
    let mut chosen_options = Vec::new();
    let mut chosen_optionals = Vec::new();

    match group {
        types::DependencyGroup::Simple(d) => {
            deps.extend(d.clone());
        }
        types::DependencyGroup::Complex(g) => {
            deps.extend(g.required.clone());

            let options = prompt_for_options(&g.options, yes)?;
            chosen_options.extend(options.clone());
            deps.extend(options);

            let optionals = prompt_for_optionals(&g.optional, dep_type, yes, all_optional)?;
            chosen_optionals.extend(optionals.clone());
            deps.extend(optionals);

            if let Some(sub_name) = sub_package_name
                && let Some(sub_deps_map) = &g.sub_packages
                && let Some(sub_dep_group) = sub_deps_map.get(sub_name)
            {
                let (sub_d, sub_co, sub_coo) = collect_dependencies_for_group(
                    sub_dep_group,
                    None,
                    dep_type,
                    yes,
                    all_optional,
                )?;
                deps.extend(sub_d);
                chosen_options.extend(sub_co);
                chosen_optionals.extend(sub_coo);
            }
        }
    }
    Ok((deps, chosen_options, chosen_optionals))
}

pub fn prompt_for_options(
    option_groups: &[types::DependencyOptionGroup],
    yes: bool,
) -> Result<Vec<String>> {
    let mut chosen = Vec::new();
    if option_groups.is_empty() {
        return Ok(chosen);
    }

    for group in option_groups {
        println!(
            "{} There are {} options available for {}:",
            "::".bold().blue(),
            group.depends.len(),
            group.desc.italic()
        );

        let parsed_deps: Vec<_> = group
            .depends
            .iter()
            .map(|d| parse_dependency_string(d))
            .collect::<Result<_>>()?;

        if yes {
            if group.all {
                println!("--yes provided, selecting all options for '{}'", group.name);
                chosen.extend(group.depends.clone());
            } else {
                println!(
                    "--yes provided, selecting first option for '{}'",
                    group.name
                );
                if let Some(dep) = group.depends.first() {
                    chosen.push(dep.clone());
                }
            }
            continue;
        }

        if group.all {
            let items: Vec<_> = parsed_deps
                .iter()
                .map(|d| {
                    format!(
                        "{}:{} - {}",
                        d.manager,
                        d.package,
                        d.description.unwrap_or("No description")
                    )
                })
                .collect();
            let selections = dialoguer::MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install (space to select, enter to confirm)")
                .items(&items)
                .interact()?;

            for i in selections {
                chosen.push(group.depends[i].clone());
            }
        } else {
            let items: Vec<_> = parsed_deps
                .iter()
                .map(|d| {
                    format!(
                        "{}:{} - {}",
                        d.manager,
                        d.package,
                        d.description.unwrap_or("No description")
                    )
                })
                .collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose one to install")
                .items(&items)
                .default(0)
                .interact()?;
            chosen.push(group.depends[selection].clone());
        }
    }
    Ok(chosen)
}

pub fn prompt_for_optionals(
    deps: &[String],
    dep_type: Option<&str>,
    yes: bool,
    all_optional: bool,
) -> Result<Vec<String>> {
    if deps.is_empty() {
        return Ok(Vec::new());
    }

    let type_str = dep_type.map(|s| format!("{} ", s)).unwrap_or_default();

    if all_optional {
        println!(
            "{} Installing all optional {}dependencies...",
            "::".bold().blue(),
            type_str
        );
        return Ok(deps.to_vec());
    }

    if yes {
        println!(
            "{} Skipping optional {}dependencies (--yes provided without --all-optional).",
            "::".bold().yellow(),
            type_str
        );
        return Ok(Vec::new());
    }

    let items: Vec<_> = deps
        .iter()
        .map(|d| {
            parse_dependency_string(d).map(|dep| {
                format!(
                    "{}:{} - {}",
                    dep.manager,
                    dep.package,
                    dep.description.unwrap_or("No description")
                )
            })
        })
        .collect::<Result<_>>()?;

    let selections = dialoguer::MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Select optional {}dependencies to install",
            type_str
        ))
        .items(&items)
        .defaults(&vec![false; deps.len()])
        .interact()?;

    let mut chosen = Vec::new();
    for i in selections {
        chosen.push(deps[i].clone());
    }
    Ok(chosen)
}
