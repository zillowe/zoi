use crate::doctor as pkg_doctor;
use crate::init_lsp;
use anyhow::{Result, anyhow};
use chrono::Datelike;
use colored::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use zoi_core::types;
use zoi_lua;
use zoi_purl;

pub fn init(path: &Path) -> Result<()> {
    println!(
        "{} Initializing new Zoi registry at {}...",
        "::".bold().blue(),
        path.display()
    );

    fs::create_dir_all(path)?;

    let dirs = ["core", "main", "community", "test", "archive"];
    for dir in &dirs {
        let dir_path = path.join(dir);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)?;
        }
    }

    init_lsp::setup_lsp_workspace(path)?;
    println!(
        "{} LSP support initialized. Created .luarc.json and type definitions.",
        "::".bold().green()
    );

    let repo_yaml_path = path.join("repo.yaml");
    if !repo_yaml_path.exists() {
        let content = r#"# Zoi Registry Configuration
# For detailed documentation, visit: https://zillowe.qzz.io/docs/zds/zoi/repositories#the-repoyaml-file

name: "My-Registry"
description: "A custom Zoi package registry"
handle: "my-registry"
advisory_prefix: "RSA" # Prefix for security advisories, e.g. RSA-2026-A0001

# Git mirrors for this registry
# For more info: https://zillowe.qzz.io/docs/zds/zoi/guides/mirroring
git:
  - type: main
    url: "https://github.com/user/my-registry.git"

# Pre-built package mirrors (optional)
# pkg:
#   - type: main
#     url: "https://example.com/pkgs/{repo}/{os}/{arch}/{version}"

# Trusted PGP keys for this registry
# pgp:
#   - name: maintainer-key
#     key: "https://example.com/keys/maintainer.asc"

# Repository tiers
repos:
  - name: core
    type: official
    active: true
  - name: main
    type: official
    active: true
  - name: community
    type: community
    active: false
  - name: test
    type: test
    active: false
  - name: archive
    type: archive
    active: false
"#;
        fs::write(repo_yaml_path, content)?;
    }

    let packages_json_path = path.join("packages.json");
    if !packages_json_path.exists() {
        let content = r#"{
  "version": "1",
  "packages": {}
}"#;
        fs::write(packages_json_path, content)?;
    }

    let advisories_json_path = path.join("advisories.json");
    if !advisories_json_path.exists() {
        let current_year = chrono::Utc::now().year();
        let content = format!(
            r#"{{
  "version": "1",
  "advisories": {{}},
  "last_id": 0,
  "year": {}
}}"#,
            current_year
        );
        fs::write(advisories_json_path, content)?;
    }

    println!("{}", "Registry initialized successfully.".green());
    println!(
        "{} Edit 'repo.yaml' to configure your registry mirrors and authorities.",
        "Note:".yellow()
    );
    Ok(())
}

pub fn add_package(registry_root: &Path, name: Option<&str>, repo: Option<&str>) -> Result<()> {
    if !registry_root.join("repo.yaml").exists() {
        return Err(anyhow!(
            "Not a Zoi registry (missing repo.yaml). Run 'zoi reg init' first."
        ));
    }

    use std::io::{Write, stdin, stdout};
    let get_input = |prompt: &str| -> String {
        print!("{}: ", prompt);
        let _ = stdout().flush();
        let mut input = String::new();
        let _ = stdin().read_line(&mut input);
        input.trim().to_string()
    };

    let name = match name {
        Some(n) => n.to_string(),
        None => get_input("Package name"),
    };

    let repo = match repo {
        Some(r) => r.to_string(),
        None => get_input("Repository tier (e.g. community, main)"),
    };

    if name.is_empty() || repo.is_empty() {
        return Err(anyhow!("Package name and repository tier are required."));
    }

    let pkg_dir = registry_root.join(&repo).join(&name);
    fs::create_dir_all(&pkg_dir)?;

    let pkg_lua_path = pkg_dir.join(format!("{}.pkg.lua", name));
    if pkg_lua_path.exists() {
        return Err(anyhow!(
            "Package '{}' already exists in repo '{}'.",
            name,
            repo
        ));
    }

    let content = format!(
        r#"-- Zoi Package Definition: {name}
-- For detailed documentation, visit: https://zillowe.qzz.io/docs/zds/zoi/creating-packages

metadata({{
  name = "{name}",
  repo = "{repo}",
  version = "1.0.0",
  revision = "1",
  description = "A short description of {name}.",
  website = "https://example.com",
  license = "MIT",
  maintainer = {{ name = "Your Name", email = "you@example.com" }},
  bins = {{ "{name}" }},
  types = {{ "pre-compiled" }}, -- Supports "source", "pre-compiled"
}})

dependencies({{
  build = {{
    -- Build-time dependencies
    -- For format info: https://zillowe.qzz.io/docs/zds/zoi/dependencies
  }},
  runtime = {{
    -- Runtime dependencies
  }}
}})

function prepare()
  -- Fetch source or binaries
  -- Example: UTILS.EXTRACT("https://example.com/release.tar.gz", "src")
end

function package()
  -- Stage files for the package
  -- Example: zcp("src/{name}", "${{pkgstore}}/bin/{name}")
end

-- function verify()
--   -- Security verification
--   -- return verifyHash("release.tar.gz", "sha256-...")
-- end

-- function test()
--   -- Integration tests (run via zoi package test)
--   -- local _, _, code = cmd(STAGING_DIR .. "/data/pkgstore/bin/{name} --version")
--   -- return code == 0
-- end

function uninstall()
  -- Cleanup outside the package store
end
"#,
        name = name,
        repo = repo
    );

    fs::write(pkg_lua_path, content)?;
    println!(
        "{} Package '{}' created in repo '{}'.",
        "::".bold().green(),
        name.cyan(),
        repo.cyan()
    );

    Ok(())
}

pub fn add_advisory(
    registry_root: &Path,
    package_name: Option<&str>,
    repo: Option<&str>,
) -> Result<()> {
    if !registry_root.join("repo.yaml").exists() {
        return Err(anyhow!(
            "Not a Zoi registry (missing repo.yaml). Run 'zoi reg init' first."
        ));
    }

    use std::io::{Write, stdin, stdout};
    let get_input = |prompt: &str| -> String {
        print!("{}: ", prompt);
        let _ = stdout().flush();
        let mut input = String::new();
        let _ = stdin().read_line(&mut input);
        input.trim().to_string()
    };

    let package_name = match package_name {
        Some(n) => n.to_string(),
        None => get_input("Package name"),
    };

    let package_name_str = package_name.as_str();

    let pkg_dir = if let Some(r) = repo {
        let dir = registry_root.join(r).join(package_name_str);
        if !dir.join(format!("{}.pkg.lua", package_name_str)).exists() {
            return Err(anyhow!(
                "Package '{}' not found in repo '{}'.",
                package_name_str,
                r
            ));
        }
        dir
    } else {
        let mut found = None;
        for entry in WalkDir::new(registry_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir()
                && entry.file_name().to_string_lossy() == package_name_str
                && entry
                    .path()
                    .join(format!("{}.pkg.lua", package_name_str))
                    .exists()
            {
                found = Some(entry.path().to_path_buf());
                break;
            }
        }
        found.ok_or_else(|| {
            anyhow!(
                "Package '{}' not found in registry. Try specifying --repo.",
                package_name_str
            )
        })?
    };

    let repo_config_str = fs::read_to_string(registry_root.join("repo.yaml"))?;
    let repo_config: types::RepoConfig = serde_yaml::from_str(&repo_config_str)?;
    let prefix = repo_config
        .advisory_prefix
        .unwrap_or_else(|| "ZSA".to_string());

    let current_year = chrono::Utc::now().year();
    let adv_file_path = pkg_dir.join(format!("{}-{}-TEMP.sec.yaml", prefix, current_year));

    if adv_file_path.exists() {
        return Err(anyhow!(
            "A temporary advisory already exists for this package."
        ));
    }

    println!(
        "{} Adding security advisory for package: {}",
        "::".bold().blue(),
        package_name_str.cyan()
    );
    println!(
        "For detailed documentation, visit: https://zillowe.qzz.io/docs/zds/zoi/guides/security-advisories\n"
    );

    let summary = get_input("Summary (short description)");
    let severity = get_input("Severity (low, medium, high, critical)");
    let affected_range = get_input("Affected version range (e.g. >=1.0.0, <1.2.3)");
    let fixed_in = get_input("Fixed in version");
    let description = get_input("Detailed description");
    let reference = get_input("Reference URL (optional)");

    let content = format!(
        r#"# Zoi Security Advisory
# For schema details: https://zillowe.qzz.io/docs/zds/zoi/guides/security-advisories#advisory-schema

id: "{prefix}-{year}-TEMP"
package: "{package_name}"
summary: "{summary}"
severity: "{severity}"
affected_range: "{affected_range}"
fixed_in: "{fixed_in}"
description: |
  {description}
references:
  - "{reference}"
"#,
        prefix = prefix,
        year = current_year,
        package_name = package_name_str,
        summary = summary,
        severity = severity,
        affected_range = affected_range,
        fixed_in = fixed_in,
        description = description,
        reference = reference
    );

    fs::write(&adv_file_path, content)?;
    println!(
        "\n{} Temporary advisory created: {}",
        "::".bold().green(),
        adv_file_path.display().to_string().cyan()
    );
    println!(
        "{} ID will be automatically assigned during 'zoi reg gen-meta' or in CI.",
        "Note:".yellow()
    );

    Ok(())
}

pub fn generate_metadata(registry_root: &Path) -> Result<()> {
    if !registry_root.join("repo.yaml").exists() {
        return Err(anyhow!(
            "Not a Zoi registry (missing repo.yaml). Run 'zoi reg init' first."
        ));
    }

    println!("{} Generating registry metadata...", "::".bold().blue());

    let repo_config_str = fs::read_to_string(registry_root.join("repo.yaml"))?;
    let repo_config: types::RepoConfig = serde_yaml::from_str(&repo_config_str)?;
    let advisory_prefix = repo_config
        .advisory_prefix
        .clone()
        .unwrap_or_else(|| "ZSA".to_string());

    let advisories_json_path = registry_root.join("advisories.json");
    let mut adv_registry: types::AdvisoryRegistry = if advisories_json_path.exists() {
        serde_json::from_str(&fs::read_to_string(&advisories_json_path)?)?
    } else {
        types::AdvisoryRegistry::default()
    };

    let current_year = chrono::Utc::now().year();
    if adv_registry.year != current_year as u32 {
        adv_registry.year = current_year as u32;
        adv_registry.last_id = 0;
    }

    for entry in WalkDir::new(registry_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.ends_with("-TEMP.sec.yaml") {
            let path = entry.path();
            let content_str = fs::read_to_string(path)?;
            let mut content: serde_yaml::Value = serde_yaml::from_str(&content_str)?;
            let severity = content
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("low")
                .to_lowercase();
            let sev_char = match severity.as_str() {
                "low" => "A",
                "medium" => "B",
                "high" => "C",
                "critical" => "D",
                _ => "A",
            };

            adv_registry.last_id += 1;
            let final_id = format!(
                "{}-{}-{}{:04}",
                advisory_prefix, current_year, sev_char, adv_registry.last_id
            );

            if let Some(mapping) = content.as_mapping_mut() {
                mapping.insert(
                    serde_yaml::Value::String("id".to_string()),
                    serde_yaml::Value::String(final_id.clone()),
                );
            }

            let final_path = path.with_file_name(format!("{}.sec.yaml", final_id));
            fs::write(&final_path, serde_yaml::to_string(&content)?)?;
            fs::remove_file(path)?;
            println!("Assigned ID {} to {}", final_id.green(), path.display());
        }
    }

    let mut advisories_map = std::collections::BTreeMap::new();
    let mut max_id = adv_registry.last_id;

    for entry in WalkDir::new(registry_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = entry.file_name().to_string_lossy();
        if file_name.ends_with(".sec.yaml") && !file_name.ends_with("-TEMP.sec.yaml") {
            let content_str = fs::read_to_string(entry.path())?;
            let content: types::Advisory = serde_yaml::from_str(&content_str)?;
            if let Some(last_part) = content.id.split('-').next_back() {
                let id_num_str = if last_part.len() > 4 {
                    &last_part[1..]
                } else {
                    last_part
                };
                if let Ok(id_num) = id_num_str.parse::<u32>() {
                    if id_num > max_id {
                        max_id = id_num;
                    }
                    let pkg_display = if let Some(sub) = &content.sub_package {
                        format!("{}:{}", content.package, sub)
                    } else {
                        content.package.clone()
                    };
                    advisories_map.insert(format!("{:04}", id_num), pkg_display);
                }
            }
        }
    }

    adv_registry.last_id = max_id;
    adv_registry.advisories = advisories_map;
    adv_registry.version = "1".to_string();
    fs::write(
        &advisories_json_path,
        serde_json::to_string_pretty(&adv_registry)?,
    )?;

    let mut packages_map = std::collections::BTreeMap::new();
    let repo_types: HashMap<String, String> = repo_config
        .repos
        .iter()
        .map(|r| (r.name.clone(), r.repo_type.clone()))
        .collect();

    for entry in WalkDir::new(registry_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".pkg.lua")
        {
            let path = entry.path();
            let path_str = path.to_string_lossy();
            if let Ok(pkg) = zoi_lua::parser::parse_lua_package(&path_str, None, true) {
                let rel_path = path.strip_prefix(registry_root)?;
                let mut repo_parts: Vec<_> = rel_path
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect();
                repo_parts.pop();
                repo_parts.pop();
                let repo_path = repo_parts.join("/");

                let major_repo = repo_path.split('/').next().unwrap_or_default();
                let repo_type = repo_types
                    .get(major_repo)
                    .cloned()
                    .unwrap_or_else(|| "unofficial".to_string());

                let version = pkg
                    .version
                    .clone()
                    .or_else(|| pkg.versions.as_ref().and_then(|v| v.get("stable").cloned()))
                    .unwrap_or_else(|| "unknown".to_string());

                let mut vulns = Vec::new();
                let pkg_dir = path
                    .parent()
                    .ok_or_else(|| anyhow!("Package path has no parent directory"))?;
                if let Ok(sec_entries) = fs::read_dir(pkg_dir) {
                    for sec_entry in sec_entries.flatten() {
                        if sec_entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".sec.yaml")
                        {
                            let sec_content_str = fs::read_to_string(sec_entry.path())?;
                            if let Ok(adv) =
                                serde_yaml::from_str::<types::Advisory>(&sec_content_str)
                            {
                                vulns.push(zoi_core::types::MiniVulnerability {
                                    id: adv.id,
                                    severity: format!("{:?}", adv.severity).to_lowercase(),
                                    affected_range: adv.affected_range,
                                    fixed_in: adv.fixed_in,
                                    summary: adv.summary,
                                });
                            }
                        }
                    }
                }

                let sub_packages = if let Some(subs) = &pkg.sub_packages {
                    let mut map = serde_json::Map::new();
                    for sub in subs {
                        map.insert(
                            sub.clone(),
                            serde_json::Value::Object(serde_json::Map::new()),
                        );
                    }
                    Some(serde_json::Value::Object(map))
                } else {
                    None
                };

                packages_map.insert(
                    pkg.name.clone(),
                    zoi_purl::PurlPackageIndex {
                        repo: repo_path,
                        repo_type,
                        version,
                        revision: pkg.revision.clone(),
                        description: pkg.description,
                        dependencies: None,
                        sub_packages,
                        vuln: if vulns.is_empty() { None } else { Some(vulns) },
                    },
                );
            }
        }
    }

    let index = zoi_purl::RegistryIndex {
        version: "1".to_string(),
        packages: packages_map,
    };
    fs::write(
        registry_root.join("packages.json"),
        serde_json::to_string_pretty(&index)?,
    )?;

    println!("{}", "Metadata generation complete.".green());

    Ok(())
}

pub fn check(registry_root: &Path) -> Result<()> {
    if !registry_root.join("repo.yaml").exists() {
        return Err(anyhow!(
            "Not a Zoi registry (missing repo.yaml). Run 'zoi reg init' first."
        ));
    }

    println!("{} Checking registry integrity...", "::".bold().blue());

    let mut errors = 0;
    let mut warnings = 0;

    for entry in WalkDir::new(registry_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".pkg.lua")
        {
            println!(
                "  Checking {}...",
                entry.path().display().to_string().cyan()
            );
            match pkg_doctor::run(entry.path(), None, None) {
                Ok(report) => {
                    for error in &report.errors {
                        eprintln!("    {} {}", "Error:".red().bold(), error);
                        errors += 1;
                    }
                    for warning in &report.warnings {
                        println!("    {} {}", "Warning:".yellow().bold(), warning);
                        warnings += 1;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "    {} Failed to parse package: {}",
                        "Error:".red().bold(),
                        e
                    );
                    errors += 1;
                }
            }
        }
    }

    if errors > 0 {
        return Err(anyhow!(
            "Registry check failed with {} error(s) and {} warning(s).",
            errors,
            warnings
        ));
    }

    println!(
        "{} Registry check passed with {} warning(s).",
        "::".bold().green(),
        warnings
    );
    Ok(())
}
