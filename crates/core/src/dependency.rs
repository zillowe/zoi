use anyhow::{Result, anyhow};
use regex::Regex;
use semver::VersionReq;
use std::sync::LazyLock;

/// A parsed dependency specification.
///
/// This represents a requirement for another package, which may be managed by Zoi
/// or an external package manager (e.g. `apt`, `brew`, `npm`).
#[derive(Debug)]
pub struct Dependency<'a> {
    /// The name of the package manager responsible for this dependency (e.g. "zoi", "apt").
    pub manager: &'a str,
    /// The name of the package in the target ecosystem.
    pub package: &'a str,
    /// A semver version requirement, if applicable.
    pub req: Option<VersionReq>,
    /// The raw version string or channel name (e.g. "1.2.3", "stable").
    pub version_str: Option<String>,
    /// An optional description of the dependency.
    pub description: Option<&'a str>,
}

static DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<pkg_and_ver>.+?)(?::(?P<desc>[^:].*))?$")
        .expect("Static DEP_RE regex is valid")
});

static VER_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches package name and optional version.
    // Supports scoped packages (@repo/name) and versions (@ver, =ver, etc.)
    Regex::new(r"^(?P<pkg>@[^@/]+/[^@]+|[^@=><~^]+)(?P<ver>@.+|[=><~^].+)?$")
        .expect("Static VER_RE regex is valid")
});

/// Parses a dependency string into its constituent parts.
///
/// The expected format is `manager:package[@version][:description]`.
/// If the manager is omitted, it defaults to "zoi".
///
/// Arguments:
/// - `dep_str`: The raw dependency string from a package definition.
/// - `is_known_manager`: A closure that determines if a prefix should be treated as a manager name.
pub fn parse_dependency_string<'a>(
    dep_str: &'a str,
    is_known_manager: impl Fn(&str) -> bool,
) -> Result<Dependency<'a>> {
    let (manager, rest) = match dep_str.split_once(':') {
        Some((m, r)) if !m.is_empty() && is_known_manager(m) => (m, r),
        _ => ("zoi", dep_str),
    };

    let rest = rest.trim();
    if rest.is_empty() {
        return Err(anyhow!("Invalid dependency string: {}", dep_str));
    }

    let caps = DEP_RE
        .captures(rest)
        .ok_or_else(|| anyhow!("Failed to parse dependency string: {}", rest))?;

    let package_and_version = caps
        .name("pkg_and_ver")
        .ok_or_else(|| {
            anyhow!(
                "Regex matched but pkg_and_ver group not found in '{}'",
                rest
            )
        })?
        .as_str()
        .trim();
    let description = caps.name("desc").map(|m| m.as_str().trim());

    let ver_caps = VER_RE.captures(package_and_version).ok_or_else(|| {
        anyhow!(
            "Failed to parse package and version from: {}",
            package_and_version
        )
    })?;

    let package = ver_caps
        .name("pkg")
        .ok_or_else(|| {
            anyhow!(
                "Regex matched but pkg group not found in '{}'",
                package_and_version
            )
        })?
        .as_str()
        .trim();
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

        if manager == "zoi" && VersionReq::parse(&req_parse_str).is_err() {
            None
        } else {
            Some(VersionReq::parse(&req_parse_str)?)
        }
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
