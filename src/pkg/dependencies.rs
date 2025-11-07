use crate::pkg::{install, local, pm, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use indicatif::MultiProgress;
use semver::VersionReq;
use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Dependency<'a> {
    pub manager: &'a str,
    pub package: &'a str,
    pub req: Option<VersionReq>,
    pub version_str: Option<String>,
    pub description: Option<&'a str>,
}

pub fn parse_dependency_string(dep_str: &str) -> Result<Dependency<'_>> {
    (|| {
        let (manager, rest) = match dep_str.split_once(':') {
            Some((m, r)) if !m.is_empty() => (m, r),
            _ => ("zoi", dep_str),
        };

        if rest.is_empty() {
            return Err(anyhow!("Invalid dependency string: {}", dep_str));
        }

        if !rest.contains(['@', '=', '>', '<', '~', '^']) && !rest.contains(':') {
            return Ok(Dependency {
                manager,
                package: rest,
                req: None,
                version_str: None,
                description: None,
            });
        }

        let (package_and_version, description) = if manager != "go" {
            if let Some((main, desc)) = rest.rsplit_once(':') {
                if main.is_empty() || desc.contains(['=', '>', '<', '~', '^', '@']) {
                    (rest, None)
                } else {
                    (main, Some(desc))
                }
            } else {
                (rest, None)
            }
        } else {
            (rest, None)
        };

        let (package, version_str) = if let Some(at_pos) = package_and_version.rfind('@') {
            if at_pos > 0 {
                let (p, v) = package_and_version.split_at(at_pos);
                (p, Some(v[1..].to_string()))
            } else {
                (package_and_version, None)
            }
        } else if let Some(idx) = package_and_version.find(['=', '>', '<', '~', '^']) {
            let (p, v) = package_and_version.split_at(idx);
            (p, Some(v.to_string()))
        } else {
            (package_and_version, None)
        };

        let req = if let Some(v_str) = &version_str {
            let req_parse_str = if v_str
                .chars()
                .next()
                .ok_or_else(|| anyhow!("Empty version string"))?
                .is_ascii_digit()
            {
                format!("={}", v_str)
            } else {
                v_str.to_string()
            };
            Some(VersionReq::parse(&req_parse_str)?)
        } else {
            None
        };

        Ok(Dependency {
            manager,
            package,
            req,
            version_str,
            description,
        })
    })()
}

pub fn install_dependency(
    dep: &Dependency,
    parent_id: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    installed_deps: &mut Vec<String>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let dep_id = format!("{}:{}", dep.manager, dep.package);
    if !processed_deps.lock().unwrap().insert(dep_id.clone()) {
        return Ok(());
    }

    let version_info = dep
        .version_str
        .as_ref()
        .map_or("any".to_string(), |r| r.to_string());
    println!(
        "-> Checking dependency: {} (version: {}) via {}",
        dep.package.cyan(),
        version_info.yellow(),
        dep.manager.yellow()
    );

    installed_deps.push(dep_id.clone());

    if dep.manager == "zoi" {
        return install_zoi_dependency(dep, parent_id, scope, yes, all_optional, processed_deps, m);
    } else if dep.manager == "native" {
        let pm = utils::get_native_package_manager()
            .ok_or_else(|| anyhow!("Native package manager not found for this OS"))?;
        println!("-> Using native package manager: {}", pm.cyan());
        let native_dep_str = format!("{}:{}", pm, dep.package);
        let native_dep = parse_dependency_string(&native_dep_str)?;
        install_dependency(
            &native_dep,
            parent_id,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
            m,
        )?;
        return Ok(());
    }

    if let Some(pm_commands) = pm::MANAGERS.get(dep.manager) {
        if let Some(check_cmd_template) = pm_commands.is_installed {
            let check_cmd = check_cmd_template.replace("{package}", dep.package);
            if utils::run_shell_command(&check_cmd).is_ok() {
                println!("Already installed. Skipping.");
                return Ok(());
            }
        }

        let package_with_version = if let Some(v) = &dep.version_str {
            match dep.manager {
                "apt" | "apt-get" | "zypper" => format!("{}={}", dep.package, v),
                "dnf" | "yum" => format!("{}-{}", dep.package, v),
                "pip" | "pipx" => format!("{}=={}", dep.package, v),
                _ => format!("{}@{}", dep.package, v),
            }
        } else {
            dep.package.to_string()
        };

        let install_cmd = pm_commands
            .install
            .replace("{package}", dep.package)
            .replace("{package_with_version}", &package_with_version);

        println!("Running install command: {}", install_cmd.italic());
        utils::run_shell_command(&install_cmd)
    } else {
        Err(anyhow!(
            "Unknown or unsupported package manager in dependency: {}",
            dep.manager
        ))
    }
}

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

    if let Some(pm_commands) = pm::MANAGERS.get(dep.manager) {
        let uninstall_cmd = pm_commands.uninstall.replace("{package}", dep.package);
        println!("Running uninstall command: {}", uninstall_cmd.italic());
        utils::run_shell_command(&uninstall_cmd)
    } else {
        Err(anyhow!(
            "Unknown or unsupported package manager for uninstall: {}",
            dep.manager
        ))
    }
}

fn install_zoi_dependency(
    dep: &Dependency,
    parent_id: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    _processed_deps: &Mutex<HashSet<String>>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let zoi_dep_name = if let Some(v) = &dep.version_str {
        format!("{}@{}", dep.package, v)
    } else {
        dep.package.to_string()
    };
    let req = crate::pkg::resolve::parse_source_string(&zoi_dep_name)?;
    if let Some(manifest) =
        local::is_package_installed(&req.name.to_lowercase(), req.sub_package.as_deref(), scope)?
    {
        println!(
            "Zoi package '{}' is already installed (version {}). Skipping.",
            dep.package, manifest.version
        );
        let package_dir = local::get_package_dir(
            scope,
            &manifest.registry_handle,
            &manifest.repo,
            &manifest.name,
        )?;
        local::add_dependent(&package_dir, parent_id)?;
        return Ok(());
    }

    println!("Not installed. Proceeding with zoi installation...");

    let (graph, _) = match install::resolver::resolve_dependency_graph(
        &[zoi_dep_name.to_string()],
        Some(scope),
        false,
        yes,
        all_optional,
        None,
        true,
    ) {
        Ok(res) => res,
        Err(e) => {
            return Err(anyhow!(
                "Failed to resolve dependency graph for '{}': {}",
                zoi_dep_name,
                e
            ));
        }
    };

    if graph.nodes.is_empty() {
        return Ok(());
    }

    let install_plan = match install::plan::create_install_plan(&graph.nodes) {
        Ok(plan) => plan,
        Err(e) => {
            return Err(anyhow!(
                "Failed to create install plan for '{}': {}",
                zoi_dep_name,
                e
            ));
        }
    };

    for (id, node) in &graph.nodes {
        let action = install_plan
            .get(id)
            .ok_or_else(|| anyhow!("Could not find install action for {}", id))?;
        install::installer::install_node(node, action, m, None, yes)?;
    }

    Ok(())
}

type ZoiUninstaller = dyn Fn(&str) -> Result<()>;

pub fn resolve_and_install_required(
    deps: &[String],
    parent_id: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    installed_deps: &mut Vec<String>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    if deps.is_empty() {
        return Ok(());
    }

    println!("{}", "Resolving required dependencies...".bold());
    for dep_str in deps {
        let dependency = parse_dependency_string(dep_str)?;
        install_dependency(
            &dependency,
            parent_id,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
            m,
        )?;
    }
    println!("{}", "All required dependencies resolved.".green());
    Ok(())
}

pub fn resolve_and_install_required_options(
    option_groups: &[types::DependencyOptionGroup],
    parent_id: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    installed_deps: &mut Vec<String>,
    chosen_options: &mut Vec<String>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    if option_groups.is_empty() {
        return Ok(());
    }

    for group in option_groups {
        println!(
            "[{}] There are {} options available for {}",
            group.name.bold(),
            group.depends.len(),
            group.desc.italic()
        );

        let mut parsed_deps = Vec::new();
        for (i, dep_str) in group.depends.iter().enumerate() {
            let dep = parse_dependency_string(dep_str)?;
            let desc = dep.description.unwrap_or("No description");
            let mut dep_display = format!("{}:{}", dep.manager, dep.package);
            if let Some(req) = &dep.req {
                dep_display.push_str(&req.to_string());
            }
            println!(
                "  {}. {} - {}",
                (i + 1).to_string().cyan(),
                dep_display.bold(),
                desc.italic()
            );
            parsed_deps.push(dep);
        }

        if yes {
            if group.all {
                println!(
                    "--yes provided, installing all options for '{}'",
                    group.name
                );
                for dep in parsed_deps.iter() {
                    chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                    install_dependency(
                        dep,
                        parent_id,
                        scope,
                        yes,
                        all_optional,
                        processed_deps,
                        installed_deps,
                        m,
                    )?;
                }
            } else {
                println!(
                    "--yes provided, installing the first option for '{}'",
                    group.name
                );
                if let Some(dep) = parsed_deps.first() {
                    chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                    install_dependency(
                        dep,
                        parent_id,
                        scope,
                        yes,
                        all_optional,
                        processed_deps,
                        installed_deps,
                        m,
                    )?;
                }
            }
            continue;
        }

        if group.all {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install (e.g. '1,3', 'all')")
                .default("1".into())
                .interact_text()?;

            let trimmed_input = input.trim().to_lowercase();
            let mut to_install = Vec::new();

            if trimmed_input == "all" {
                to_install.extend(0..parsed_deps.len());
            } else {
                for part in trimmed_input.split([',', ' ']).filter(|s| !s.is_empty()) {
                    if let Ok(num) = part.parse::<usize>() {
                        if num > 0 && num <= parsed_deps.len() {
                            to_install.push(num - 1);
                        } else {
                            return Err(anyhow::anyhow!("Invalid number: {}", num));
                        }
                    } else {
                        return Err(anyhow::anyhow!("Invalid input: {}", part));
                    }
                }
            }
            to_install.sort();
            to_install.dedup();

            for index in to_install {
                let dep = &parsed_deps[index];
                chosen_options.push(format!("{}:{}", dep.manager, dep.package));
                install_dependency(
                    dep,
                    parent_id,
                    scope,
                    yes,
                    all_optional,
                    processed_deps,
                    installed_deps,
                    m,
                )?;
            }
        } else {
            let items: Vec<_> = parsed_deps
                .iter()
                .map(|d| {
                    let mut dep_display = format!("{}:{}", d.manager, d.package);
                    if let Some(req) = &d.req {
                        dep_display.push_str(&req.to_string());
                    }
                    dep_display
                })
                .collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose which to install")
                .items(&items)
                .default(0)
                .interact()?;

            let dep = &parsed_deps[selection];
            chosen_options.push(format!("{}:{}", dep.manager, dep.package));
            install_dependency(
                dep,
                parent_id,
                scope,
                yes,
                all_optional,
                processed_deps,
                installed_deps,
                m,
            )?;
        }
    }
    Ok(())
}

pub fn resolve_and_install_optional(
    deps: &[String],
    parent_id: &str,
    _parent_version: &str,
    scope: types::Scope,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    installed_deps: &mut Vec<String>,
    chosen_optionals: &mut Vec<String>,
    dep_type: Option<&str>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    if deps.is_empty() {
        return Ok(());
    }

    let type_str = dep_type.map(|s| format!("{} ", s)).unwrap_or_default();

    if yes || all_optional {
        println!(
            "{}",
            format!("Installing all optional {} dependencies...", type_str).bold()
        );
        for dep_str in deps {
            chosen_optionals.push(dep_str.clone());
            let dependency = parse_dependency_string(dep_str)?;
            install_dependency(
                &dependency,
                parent_id,
                scope,
                true,
                all_optional,
                processed_deps,
                installed_deps,
                m,
            )?;
        }
        return Ok(());
    }

    println!(
        "{}",
        format!("This package has optional {} dependencies:", type_str).bold()
    );
    let mut parsed_deps = Vec::new();
    for (i, dep_str) in deps.iter().enumerate() {
        let dep = parse_dependency_string(dep_str)?;
        let desc = dep.description.unwrap_or("No description");
        let mut dep_display = format!("{}:{}", dep.manager, dep.package);
        if let Some(req) = &dep.req {
            dep_display.push_str(&req.to_string());
        }
        println!(
            "  {}. {} - {}",
            (i + 1).to_string().cyan(),
            dep_display.bold(),
            desc.italic()
        );
        parsed_deps.push(dep);
    }

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose which to install (e.g. '1,3', 'all', 'none')")
        .default("none".into())
        .interact_text()?;

    let trimmed_input = input.trim().to_lowercase();

    if trimmed_input == "none" || trimmed_input == "n" || trimmed_input == "0" {
        println!("Skipping optional dependencies.");
        return Ok(());
    }

    let mut to_install = Vec::new();
    if trimmed_input == "all" || trimmed_input == "y" {
        to_install.extend(0..parsed_deps.len());
    } else {
        for part in trimmed_input.split([',', ' ']).filter(|s| !s.is_empty()) {
            if let Ok(num) = part.parse::<usize>() {
                if num > 0 && num <= parsed_deps.len() {
                    to_install.push(num - 1);
                } else {
                    return Err(anyhow::anyhow!("Invalid number: {}", num));
                }
            } else {
                return Err(anyhow::anyhow!("Invalid input: {}", part));
            }
        }
    }

    to_install.sort();
    to_install.dedup();

    for index in to_install {
        let dep = &parsed_deps[index];
        chosen_optionals.push(format!("{}:{}", dep.manager, dep.package));
        install_dependency(
            dep,
            parent_id,
            scope,
            yes,
            all_optional,
            processed_deps,
            installed_deps,
            m,
        )?;
    }

    Ok(())
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
            "[{}] There are {} options available for {}",
            group.name.bold(),
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

    if yes || all_optional {
        println!("Installing all optional {}dependencies...", type_str);
        return Ok(deps.to_vec());
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
