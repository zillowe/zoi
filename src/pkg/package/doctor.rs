use crate::pkg::{dependencies, lua, types};
use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Default)]
pub struct DoctorReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn run(
    package_file: &Path,
    platform_override: Option<&str>,
    version_override: Option<&str>,
) -> Result<DoctorReport> {
    let file_path = package_file
        .to_str()
        .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", package_file))?;
    let platform = match platform_override {
        Some(p) => p.to_string(),
        None => crate::utils::get_platform()?,
    };

    let package =
        lua::parser::parse_lua_package_for_platform(file_path, &platform, version_override, true)?;

    let mut report = DoctorReport::default();

    if package.name.trim().is_empty() {
        report.errors.push("metadata.name is empty.".to_string());
    }
    if package.repo.trim().is_empty() {
        report.errors.push("metadata.repo is empty.".to_string());
    }
    if package.description.trim().is_empty() {
        report
            .errors
            .push("metadata.description is empty.".to_string());
    }
    if package.license.trim().is_empty() {
        report
            .warnings
            .push("metadata.license is empty; set an SPDX license expression.".to_string());
    }
    if package.maintainer.name.trim().is_empty() {
        report
            .warnings
            .push("metadata.maintainer.name is empty.".to_string());
    }
    if package.maintainer.email.trim().is_empty() {
        report
            .warnings
            .push("metadata.maintainer.email is empty.".to_string());
    }

    if package.version.is_none() && package.versions.as_ref().is_none_or(|m| m.is_empty()) {
        report.errors.push(
            "Package has no version information. Set metadata.version or metadata.versions."
                .to_string(),
        );
    }

    if package.types.is_empty() {
        report
            .errors
            .push("metadata.types is empty; at least one build type is required.".to_string());
    } else {
        let known = ["source", "pre-compiled"];
        for t in &package.types {
            if !known.contains(&t.as_str()) {
                report.warnings.push(format!(
                    "Build type '{}' is custom. Ensure your pipeline supports it.",
                    t
                ));
            }
        }
    }

    if let Some(subs) = &package.sub_packages {
        let mut seen = HashSet::new();
        for sub in subs {
            if !seen.insert(sub.clone()) {
                report.errors.push(format!(
                    "Duplicate sub-package '{}' in metadata.sub_packages.",
                    sub
                ));
            }
        }
    }

    if let Some(main_subs) = &package.main_subs {
        let allowed = package
            .sub_packages
            .as_ref()
            .map(|v| v.iter().cloned().collect::<HashSet<String>>())
            .unwrap_or_default();
        for sub in main_subs {
            if !allowed.contains(sub) {
                report.errors.push(format!(
                    "main_subs contains '{}' but it is missing from sub_packages.",
                    sub
                ));
            }
        }
    }

    if let Some(deps) = &package.dependencies {
        if let Some(runtime) = &deps.runtime {
            validate_dependency_group(runtime, "runtime", &package, &mut report);
        }
        if let Some(build) = &deps.build {
            match build {
                types::BuildDependencies::Group(group) => {
                    validate_dependency_group(group, "build", &package, &mut report);
                }
                types::BuildDependencies::Typed(typed) => {
                    if typed.types.is_empty() {
                        report
                            .errors
                            .push("dependencies.build.types is empty.".to_string());
                    }
                    for (build_type, group) in &typed.types {
                        if !package.types.contains(build_type) {
                            report.warnings.push(format!(
                                "dependencies.build.types has '{}' but metadata.types does not list it.",
                                build_type
                            ));
                        }
                        validate_dependency_group(
                            group,
                            &format!("build.type={}", build_type),
                            &package,
                            &mut report,
                        );
                    }
                }
            }
        }
    }

    let lua_code = std::fs::read_to_string(package_file)?;
    validate_lua_functions(&lua_code, &mut report);

    Ok(report)
}

fn validate_dependency_group(
    group: &types::DependencyGroup,
    context: &str,
    package: &types::Package,
    report: &mut DoctorReport,
) {
    for dep in group.get_required_simple() {
        validate_dependency_string(&dep, context, "required", report);
    }

    for option in group.get_required_options() {
        for dep in option.depends {
            validate_dependency_string(&dep, context, &format!("options.{}", option.name), report);
        }
    }

    for dep in group.get_optional() {
        validate_dependency_string(dep, context, "optional", report);
    }

    if let types::DependencyGroup::Complex(complex) = group
        && let Some(subs) = &complex.sub_packages
    {
        let declared_subs = package
            .sub_packages
            .as_ref()
            .map(|v| v.iter().cloned().collect::<HashSet<String>>())
            .unwrap_or_default();

        for (sub_name, sub_group) in subs {
            if !declared_subs.is_empty() && !declared_subs.contains(sub_name) {
                report.warnings.push(format!(
                    "Dependency group for sub-package '{}' is declared, but metadata.sub_packages does not include it.",
                    sub_name
                ));
            }
            validate_dependency_group(
                sub_group,
                &format!("{}.sub_package={}", context, sub_name),
                package,
                report,
            );
        }
    }
}

fn validate_dependency_string(dep: &str, context: &str, bucket: &str, report: &mut DoctorReport) {
    if let Err(err) = dependencies::parse_dependency_string(dep) {
        report.errors.push(format!(
            "Invalid dependency '{}' in {}.{}: {}",
            dep, context, bucket, err
        ));
    }
}

fn validate_lua_functions(lua_code: &str, report: &mut DoctorReport) {
    let has_prepare = Regex::new(r"(?m)\bfunction\s+prepare\s*\(")
        .map(|re| re.is_match(lua_code))
        .unwrap_or(false);
    let has_package = Regex::new(r"(?m)\bfunction\s+package\s*\(")
        .map(|re| re.is_match(lua_code))
        .unwrap_or(false);
    let has_test = Regex::new(r"(?m)\bfunction\s+test\s*\(")
        .map(|re| re.is_match(lua_code))
        .unwrap_or(false);

    if !has_prepare {
        report.warnings.push(
            "No prepare() function detected. Add it if source fetching/setup is needed."
                .to_string(),
        );
    }
    if !has_package {
        report.warnings.push(
            "No package() function detected. Build/install steps may be incomplete.".to_string(),
        );
    }
    if !has_test {
        report.warnings.push(
            "No test() function detected. Add package tests for maintainability.".to_string(),
        );
    }
}
