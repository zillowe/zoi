use crate::pkg::{install, local, pm, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::MultiProgress;
use regex::Regex;
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
    let (manager, rest) = match dep_str.split_once(':') {
        Some((m, r))
            if !m.is_empty() && (pm::MANAGERS.contains_key(m) || m == "zoi" || m == "native") =>
        {
            (m, r)
        }
        _ => ("zoi", dep_str),
    };

    if rest.is_empty() {
        return Err(anyhow!("Invalid dependency string: {}", dep_str));
    }

    let re = Regex::new(r"^(?P<pkg_and_ver>.+?)(?::(?P<desc>[^:].*))?$").unwrap();
    let caps = re
        .captures(rest)
        .ok_or_else(|| anyhow!("Failed to parse dependency string: {}", rest))?;

    let package_and_version = caps.name("pkg_and_ver").unwrap().as_str();
    let description = caps.name("desc").map(|m| m.as_str());

    let ver_re = Regex::new(r"^(?P<pkg>.*?)(?P<ver>@.+|[=><~^].+)?$").unwrap();
    let ver_caps = ver_re.captures(package_and_version).ok_or_else(|| {
        anyhow!(
            "Failed to parse package and version from: {}",
            package_and_version
        )
    })?;

    let package = ver_caps.name("pkg").unwrap().as_str();
    let mut version_str = ver_caps.name("ver").map(|m| m.as_str().to_string());

    if let Some(v) = &version_str
        && v.starts_with('@')
    {
        version_str = Some(v[1..].to_string());
    }

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
    }

    if let Some(pm_commands) = pm::MANAGERS.get(dep.manager) {
        if let Some(check_cmd_template) = pm_commands.is_installed {
            let check_cmd = check_cmd_template.replace("{package}", dep.package);
            if utils::run_shell_command(&check_cmd).is_ok() {
                println!("Already installed. Skipping.");
                return Ok(());
            }
        }

        if dep.manager == "aur" {
            return install_aur_dependency(dep, yes);
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

        if install_cmd.starts_with('#') {
            return Err(anyhow!(
                "Installation command for '{}' is not configured: {}",
                dep.manager,
                install_cmd
            ));
        }

        println!("Running install command: {}", install_cmd.italic());
        utils::run_shell_command(&install_cmd)
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
        )
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

fn install_aur_dependency(dep: &Dependency, yes: bool) -> Result<()> {
    if !crate::utils::command_exists("git") {
        return Err(anyhow!(
            "'git' command not found. Needed for AUR installation."
        ));
    }
    if !crate::utils::command_exists("makepkg") {
        return Err(anyhow!(
            "'makepkg' command not found. Are you on Arch Linux?"
        ));
    }

    let temp_dir = tempfile::Builder::new().prefix("zoi-aur-").tempdir()?;
    let repo_url = format!("https://aur.archlinux.org/{}.git", dep.package);

    println!("-> Cloning {}...", repo_url.cyan());
    let clone_status = std::process::Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(&repo_url)
        .arg(temp_dir.path())
        .status()?;

    if !clone_status.success() {
        return Err(anyhow!(
            "Failed to clone AUR repository for {}",
            dep.package
        ));
    }

    println!("-> Building and installing with makepkg...");
    let mut makepkg_cmd = std::process::Command::new("makepkg");
    makepkg_cmd.arg("-si").current_dir(temp_dir.path());
    if yes {
        makepkg_cmd.arg("--noconfirm");
    }

    let install_status = makepkg_cmd.status()?;
    if !install_status.success() {
        return Err(anyhow!("makepkg failed for {}", dep.package));
    }

    Ok(())
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

    let stages = graph.toposort()?;
    for stage in stages {
        for id in stage {
            let node = graph.nodes.get(&id).unwrap();
            let action = install_plan
                .get(&id)
                .ok_or_else(|| anyhow!("Could not find install action for {}", id))?;
            install::installer::install_node(node, action, m, None, yes)?;
        }
    }

    Ok(())
}

type ZoiUninstaller = dyn Fn(&str) -> Result<()>;

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
