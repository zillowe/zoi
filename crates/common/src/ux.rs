//! User experience (UX) data structures for Zoi.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

/// Classifies the source and method used to install a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InstallOrigin {
    /// Package installed from a prebuilt binary in the registry.
    #[serde(rename = "registry-prebuilt")]
    RegistryPrebuilt,
    /// Package built from source in the registry.
    #[serde(rename = "registry-source")]
    RegistrySource,
    /// Package installed from a local archive file.
    #[serde(rename = "local-archive")]
    LocalArchive,
    /// Package installed from a local package definition.
    #[serde(rename = "local-package")]
    LocalPackage,
    /// Package downloaded and installed from a remote URL.
    #[serde(rename = "url")]
    RemoteUrl,
    /// Origin of the package is unknown.
    #[serde(rename = "unknown")]
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

/// The standard JSON schema for Zoi execution plans.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanJsonV1 {
    /// Schema version (currently "zoi.plan.v1").
    pub schema: String,
    /// The command that generated this plan (e.g. "install", "update").
    pub command: String,
    /// Command-specific fields.
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
