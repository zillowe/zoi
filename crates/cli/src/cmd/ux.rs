//! User experience (UX) utilities for the Zoi CLI.
//!
//! This module contains types and functions for formatting output,
//! providing hints to the user, and emitting machine-readable plans.

use std::collections::BTreeMap;

use anyhow::anyhow;
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;

/// Classifies the source and method used to install a package.
///
/// This is used for reporting and telemetry to understand where packages
/// are coming from in the ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOrigin {
    /// Package installed from a prebuilt binary in the registry.
    RegistryPrebuilt,
    /// Package built from source in the registry.
    RegistrySource,
    /// Package installed from a local archive file.
    LocalArchive,
    /// Package installed from a local package definition.
    LocalPackage,
    /// Package downloaded and installed from a remote URL.
    RemoteUrl,
    /// Origin of the package is unknown.
    Unknown
}

impl InstallOrigin {
    /// Returns the string representation of the install origin.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegistryPrebuilt => "registry-prebuilt",
            Self::RegistrySource => "registry-source",
            Self::LocalArchive => "local-archive",
            Self::LocalPackage => "local-package",
            Self::RemoteUrl => "url",
            Self::Unknown => "unknown"
        }
    }
}

/// A summary of a transaction's results.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionSummary {
    /// The command that was executed.
    pub command: String,
    /// Number of successful operations.
    pub success: usize,
    /// Number of failed operations.
    pub failed: usize,
    /// Number of skipped operations.
    pub skipped: usize
}

/// A single row in a preflight summary table.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreflightRow {
    /// The key/label for the row.
    pub key: String,
    /// The value for the row.
    pub value: String
}

/// A summary of preflight checks before an operation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreflightSummary {
    /// The title of the summary.
    pub title: String,
    /// The rows containing detailed information.
    pub rows: Vec<PreflightRow>
}

impl PreflightSummary {
    /// Creates a new preflight summary with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new()
        }
    }

    /// Adds a row to the summary.
    #[must_use]
    pub fn row(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>
    ) -> Self {
        self.rows.push(PreflightRow {
            key: key.into(),
            value: value.into()
        });
        self
    }
}

/// An item in an explanation report.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExplainItem {
    /// The subject of the explanation.
    pub subject: String,
    /// The reason or brief explanation.
    pub reason: String,
    /// Additional details about the item.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>
}

/// A report explaining the reasons for certain actions or states.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExplainReport {
    /// The title of the report.
    pub title: String,
    /// The items in the report.
    pub items: Vec<ExplainItem>
}

impl ExplainReport {
    /// Creates a new explanation report with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new()
        }
    }

    /// Adds an item to the report.
    #[must_use]
    pub fn item(
        mut self,
        subject: impl Into<String>,
        reason: impl Into<String>,
        details: Vec<String>
    ) -> Self {
        self.items.push(ExplainItem {
            subject: subject.into(),
            reason: reason.into(),
            details
        });
        self
    }
}

/// The standard JSON schema for Zoi execution plans (Specification v2).
///
/// This provides a stable, machine-readable interface for CI/CD pipelines
/// and external tools to understand what Zoi intends to do before it
/// performs any destructive actions.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanJsonV1 {
    /// Schema version (currently "zoi.plan.v1").
    pub schema: String,
    /// The command that generated this plan (e.g. "install", "update").
    pub command: String,
    /// Command-specific fields (e.g. "packages", "totals", "`dry_run`").
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>
}

impl PlanJsonV1 {
    /// Creates a new version 1 plan JSON object.
    pub fn new(
        command: impl Into<String>,
        fields: BTreeMap<String, Value>
    ) -> Self {
        Self {
            schema: "zoi.plan.v1".to_string(),
            command: command.into(),
            fields
        }
    }
}

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
        return Some("Free space (e.g. `zoi clean`) and retry.");
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
