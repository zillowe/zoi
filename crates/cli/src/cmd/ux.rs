//! User experience (UX) utilities for the Zoi CLI.
//!
//! This module contains types and functions for formatting output,
//! providing hints to the user, and emitting machine-readable plans.

use std::collections::BTreeMap;

use anyhow::anyhow;
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
pub use zoi_common::ux::*;

/// Prints a preflight summary to the console.
pub fn print_preflight(summary: &PreflightSummary) {
    let title = summary.title.bold();
    println!("\n{} {title}", "::".bold().blue());
    for row in &summary.rows {
        let key = format!("{}:", row.key).cyan();
        println!("  {:<24}{}", key, row.value);
    }
}

/// Prints a transaction summary to the console.
pub fn print_transaction_summary(summary: &TransactionSummary) {
    let command = &summary.command;
    let success = summary.success.to_string().green();
    let failed = summary.failed.to_string().red();
    let skipped = summary.skipped.to_string().yellow();
    println!(
        "\n{} {command} summary: success={success}, failed={failed}, \
         skipped={skipped}",
        "::".bold().blue()
    );
}

/// Emits a plan in JSON format to stdout.
///
/// # Errors
///
/// Returns an error if the plan cannot be serialized to JSON.
pub fn emit_plan_json<T: Serialize>(plan: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(plan)?;
    println!("{json}");
    Ok(())
}

/// Emits a version 1 plan in JSON format to stdout.
///
/// # Errors
///
/// Returns an error if the plan cannot be serialized to JSON.
pub fn emit_plan_json_v1(command: &str, payload: Value) -> anyhow::Result<()> {
    let mut fields = BTreeMap::new();
    match payload {
        Value::Object(map) => {
            for (key, value) in map {
                fields.insert(key, value);
            }
        }
        other => {
            fields.insert("data".to_string(), other);
        }
    }
    let plan = PlanJsonV1::new(command, fields);
    emit_plan_json(&plan)
}

/// Prints an explanation report to the console.
pub fn print_explain(report: &ExplainReport) {
    let title = &report.title;
    println!("\n{} {title}", "::".bold().blue());
    for item in &report.items {
        let subject = item.subject.cyan();
        let reason = &item.reason;
        println!("  - {subject} {reason}");
        for detail in &item.details {
            let detail = detail.dimmed();
            println!("    {detail}");
        }
    }
}

/// Classifies the origin of a package based on its source string and the action
/// being performed.
#[must_use]
pub fn classify_source_origin(
    source: &str,
    action_name: &str
) -> InstallOrigin {
    if source.starts_with("http://") || source.starts_with("https://") {
        return InstallOrigin::RemoteUrl;
    }
    let path = std::path::Path::new(source);
    if path.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("zpa")
            || ext.eq_ignore_ascii_case("zsa")
            || source.ends_with(".pkg.tar.zst")
    }) {
        return InstallOrigin::LocalArchive;
    }
    if (source.ends_with(".pkg.lua") || source.ends_with(".manifest.yaml"))
        && path.exists()
    {
        return InstallOrigin::LocalPackage;
    }
    if action_name == "download" {
        InstallOrigin::RegistryPrebuilt
    } else if action_name == "build" {
        InstallOrigin::RegistrySource
    } else {
        InstallOrigin::Unknown
    }
}

/// Formats a package identifier for display based on registry and repository
/// configuration.
pub fn format_display_name(
    registry: &str,
    repo: &str,
    name: &str,
    sub: Option<&str>,
    config: &zoi_core::types::Config
) -> String {
    let base_name = if let Some(s) = sub {
        format!("{name}:{s}")
    } else {
        name.to_string()
    };

    if registry == "local" && repo.starts_with("git/") {
        let repo_name = &repo[4..];
        return format!("#git@{repo_name}/{base_name}");
    }

    let default_handle = config
        .default_registry
        .as_ref()
        .map_or("", |r| r.handle.as_str());
    let active_repos = &config.repos;

    if registry == default_handle || registry == "local" || registry.is_empty()
    {
        if active_repos.contains(&repo.to_string()) || repo.is_empty() {
            base_name
        } else {
            format!("@{repo}/{base_name}")
        }
    } else {
        format!("#{registry}@{repo}/{base_name}")
    }
}

/// Wraps an error with a user-friendly hint if one is available for the given
/// error message and command.
#[must_use]
pub fn with_failure_hint(command: &str, err: anyhow::Error) -> anyhow::Error {
    let msg = err.to_string();
    let hint = failure_hint(&msg, command);
    hint.map_or(err, |hint_text| anyhow!("{msg}\nHint: {hint_text}"))
}

/// Returns a user-friendly hint for a given error message and command.
fn failure_hint(message: &str, command: &str) -> Option<&'static str> {
    let m = message.to_lowercase();
    if m.contains("not synced") || m.contains("registry") && m.contains("sync")
    {
        return Some("Run `zoi sync` and retry.");
    }
    if m.contains("not enough disk space") {
        return Some("Free space (e.g. `zoi cache clear`) and retry.");
    }
    if m.contains("policy") || m.contains("compliance") {
        return Some("Review policy settings in config and rerun.");
    }
    if m.contains("vulnerab") || m.contains("advisory") {
        return Some("Run `zoi audit` to inspect advisories before retrying.");
    }
    if m.contains("lockfile") {
        return Some(
            "Regenerate project lock state with a normal project install, \
             then retry."
        );
    }
    if m.contains("hash verification failed") || m.contains("checksum") {
        return Some(
            "Resync metadata and retry; verify upstream archive integrity."
        );
    }
    if command == "uninstall" && m.contains("ambiguous package name") {
        return Some("Specify an explicit source like `#handle@repo/name`.");
    }
    if command == "update" && m.contains("not installed") {
        return Some("Use `zoi install` for new packages.");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_origin_remote_url() {
        let origin =
            classify_source_origin("https://example.com/pkg.lua", "download");
        assert_eq!(origin, InstallOrigin::RemoteUrl);
    }

    #[test]
    fn classify_origin_registry_prebuilt() {
        let origin = classify_source_origin("@core/hello", "download");
        assert_eq!(origin, InstallOrigin::RegistryPrebuilt);
    }

    #[test]
    fn appends_failure_hint_for_disk_errors() {
        let err = anyhow!("Not enough disk space");
        let with_hint = with_failure_hint("install", err).to_string();
        assert!(with_hint.contains("Hint:"));
    }

    #[test]
    fn plan_json_v1_has_schema_and_command() {
        let mut fields = BTreeMap::new();
        fields.insert("dry_run".to_string(), Value::Bool(true));
        let plan = PlanJsonV1::new("install", fields);
        assert_eq!(plan.schema, "zoi.plan.v1");
        assert_eq!(plan.command, "install");
    }

    #[test]
    fn preflight_summary_builder_collects_rows() {
        let summary = PreflightSummary::new("Install preflight")
            .row("Scope", "User")
            .row("Retry attempts", "3");
        assert_eq!(summary.rows.len(), 2);
        if let Some(row) = summary.rows.first() {
            assert_eq!(row.key, "Scope");
        }
        if let Some(row) = summary.rows.get(1) {
            assert_eq!(row.value, "3");
        }
    }
}
