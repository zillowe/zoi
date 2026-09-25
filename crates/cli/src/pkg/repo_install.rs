use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, anyhow};
use colored::Colorize;
use tempfile::TempDir;
use zoi_core::types::SourceType;
use zoi_project::config::{ImportSpec, PackageExportSpec};

/// Parsed repository URL and optional package selector.
#[derive(Debug)]
struct RepoSpec {
    /// Git repository URL.
    url: String,
    /// Optional package export selector.
    selector: Option<String>
}

/// Owns cloned repository trees for the lifetime of an installation.
pub struct RepoWorkspace {
    /// Temporary directories containing cloned repositories.
    roots: Vec<TempDir>,
    /// Absolute package definition paths selected during preparation.
    sources: Vec<String>,
    /// Exact top-level repository commit.
    revision: String
}

impl RepoWorkspace {
    /// Clones a repository and resolves its selected package definition.
    ///
    /// # Errors
    ///
    /// Returns an error when cloning, parsing the repository `zoi.lua`,
    /// resolving imports, or selecting the requested package fails.
    pub fn prepare(spec: &str) -> Result<Self> {
        let spec = parse_repo_spec(spec)?;
        let root = tempfile::Builder::new().prefix("zoi-repo-").tempdir()?;
        clone_repository(&spec.url, root.path(), None)?;
        let revision = repository_revision(root.path())?;
        let manifest_path = root.path().join("zoi.lua");
        let mut workspace = Self {
            roots: vec![root],
            sources: Vec::new(),
            revision
        };
        let root_path = workspace
            .roots
            .first()
            .map(|root| root.path().to_path_buf())
            .ok_or_else(|| anyhow!("Repository workspace is unavailable"))?;
        let source = workspace.resolve_repository_source(
            &root_path,
            &manifest_path,
            spec.selector.as_deref(),
            &mut HashSet::new()
        )?;
        workspace
            .sources
            .push(source.to_string_lossy().into_owned());
        Ok(workspace)
    }

    /// Returns the absolute package definition paths selected from the
    /// repository.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Returns the exact commit used for the top-level repository.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Resolves a repository export while detecting recursive import cycles.
    ///
    /// # Errors
    ///
    /// Returns an error when the repository manifest is invalid, the selected
    /// export is unavailable, or an import cycle is detected.
    fn resolve_repository_source(
        &mut self,
        repository_root: &Path,
        manifest_path: &Path,
        selector: Option<&str>,
        stack: &mut HashSet<String>
    ) -> Result<PathBuf> {
        let repository_key = repository_root.to_string_lossy().into_owned();
        if !stack.insert(repository_key.clone()) {
            return Err(anyhow!(
                "Repository import cycle detected at '{}'",
                manifest_path.display()
            ));
        }

        let result = (|| {
            let (imports, exports) =
                zoi_project::lua_config::load_repo_zoi_lua(manifest_path)?;
            let exports = exports.ok_or_else(|| {
                anyhow!(
                    "Repository {} declares no package. Add a top-level \
                     package(...) call.",
                    manifest_path.parent().unwrap_or(manifest_path).display()
                )
            })?;
            let source = select_export(&exports, selector)?;
            resolve_source(repository_root, &source, &imports, self, stack)
        })();

        stack.remove(&repository_key);
        result
    }
}

/// Clones a repository and optionally checks out a requested revision.
///
/// # Errors
///
/// Returns an error when the Git clone or checkout command fails.
fn clone_repository(
    url: &str,
    destination: &Path,
    revision: Option<&str>
) -> Result<()> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--no-tags")
        .arg(url)
        .arg(destination)
        .output()
        .map_err(|error| anyhow!("Failed to run git clone: {error}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to clone '{}': {}",
            url,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if let Some(revision) = revision {
        let output = Command::new("git")
            .arg("-C")
            .arg(destination)
            .arg("checkout")
            .arg("--detach")
            .arg(revision)
            .output()
            .map_err(|error| anyhow!("Failed to run git checkout: {error}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "Failed to check out import revision '{}': {}",
                revision,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

/// Reads the exact revision currently checked out in a repository.
///
/// # Errors
///
/// Returns an error when Git cannot read the repository revision.
fn repository_revision(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| {
            anyhow!("Failed to read repository revision: {error}")
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to read repository revision: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// Selects a package export from a repository manifest.
///
/// # Errors
///
/// Returns an error when the requested export is absent or the manifest has
/// no unambiguous default export.
fn select_export(
    exports: &PackageExportSpec,
    selector: Option<&str>
) -> Result<String> {
    if let Some(selector) = selector {
        if let Some(main) = &exports.main
            && (selector == "main" || exports.packages.is_empty())
        {
            return Ok(main.clone());
        }
        return exports.packages.get(selector).cloned().ok_or_else(|| {
            let mut available = Vec::new();
            if exports.main.is_some() {
                available.push("main".to_string());
            }
            available.extend(exports.packages.keys().cloned());
            anyhow!(
                "Repository export '{selector}' was not found. Available: {}",
                available.join(", ")
            )
        });
    }

    if let Some(main) = &exports.main {
        return Ok(main.clone());
    }
    match exports.packages.len() {
        1 => Ok(exports
            .packages
            .values()
            .next()
            .cloned()
            .ok_or_else(|| anyhow!("Repository package export is empty"))?),
        0 => Err(anyhow!("Repository declares no package export")),
        _ => {
            let names = exports
                .packages
                .keys()
                .map(|name| format!("--repo <repository>:{name}"))
                .collect::<Vec<_>>()
                .join(", ");
            Err(anyhow!(
                "Repository declares multiple packages and no main export. \
                 Select one with: {names}"
            ))
        }
    }
}

/// Resolves an export into a package definition contained in the workspace.
///
/// # Errors
///
/// Returns an error when an import or local package source cannot be cloned,
/// found, or safely resolved within the repository root.
fn resolve_source(
    repository_root: &Path,
    source: &str,
    imports: &BTreeMap<String, ImportSpec>,
    workspace: &mut RepoWorkspace,
    stack: &mut HashSet<String>
) -> Result<PathBuf> {
    if let Some((alias, selector)) = source.split_once(':')
        && let Some(import) = imports.get(alias)
    {
        let destination = workspace
            .roots
            .first()
            .map(TempDir::path)
            .ok_or_else(|| anyhow!("Repository workspace is unavailable"))?
            .join("imports")
            .join(alias);
        if destination.exists() {
            std::fs::remove_dir_all(&destination)?;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        clone_repository(
            &repository_url(&import.repo)?,
            &destination,
            import.rev.as_deref()
        )?;
        let manifest_path = destination.join("zoi.lua");
        if !manifest_path.is_file() {
            return Err(anyhow!(
                "Imported repository '{}' has no zoi.lua",
                import.repo
            ));
        }
        return workspace.resolve_repository_source(
            &destination,
            &manifest_path,
            Some(selector),
            stack
        );
    }

    let source = source.strip_prefix("zoi:").unwrap_or(source);
    if source.starts_with("http://") || source.starts_with("https://") {
        return Err(anyhow!(
            "Remote package URLs in cloned repositories must be materialized \
             as imports: {source}"
        ));
    }

    let path = repository_root.join(source);
    if path.is_file() {
        return ensure_within(repository_root, &path);
    }
    if path.is_dir() {
        let conventional = path.join(format!(
            "{}.pkg.lua",
            path.file_name().and_then(|name| name.to_str()).ok_or_else(
                || anyhow!("Package directory has an invalid name")
            )?
        ));
        if conventional.is_file() {
            return ensure_within(repository_root, &conventional);
        }
        let mut candidates = std::fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate.is_file()
                    && candidate
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".pkg.lua"))
            })
            .collect::<Vec<_>>();
        candidates.sort();
        return match candidates.as_slice() {
            [candidate] => ensure_within(repository_root, candidate),
            [] => Err(anyhow!(
                "Package folder '{}' contains no .pkg.lua file",
                path.display()
            )),
            _ => Err(anyhow!(
                "Package folder '{}' is ambiguous; select one of: {}",
                path.display(),
                candidates
                    .iter()
                    .filter_map(|candidate| candidate.file_stem())
                    .filter_map(|name| name.to_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        };
    }

    let flat = repository_root.join(format!("{source}.pkg.lua"));
    if flat.is_file() {
        return ensure_within(repository_root, &flat);
    }
    let nested = repository_root
        .join(source)
        .join(format!("{source}.pkg.lua"));
    if nested.is_file() {
        return ensure_within(repository_root, &nested);
    }

    ensure_within(repository_root, &path)
}

/// Canonicalizes a package source and ensures it remains inside a repository.
///
/// # Errors
///
/// Returns an error when a path cannot be canonicalized or escapes the
/// repository root.
fn ensure_within(root: &Path, path: &Path) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|error| {
        anyhow!("Failed to resolve repository root: {error}")
    })?;
    let path = path.canonicalize().map_err(|error| {
        anyhow!(
            "Failed to resolve package source '{}': {error}",
            path.display()
        )
    })?;
    if !path.starts_with(&root) {
        return Err(anyhow!(
            "Package source '{}' escapes the repository root",
            path.display()
        ));
    }
    Ok(path)
}

/// Expands a provider-qualified repository specification into a Git URL.
///
/// # Errors
///
/// Returns an error when the repository provider is unknown.
fn repository_url(spec: &str) -> Result<String> {
    if spec.starts_with("http://")
        || spec.starts_with("https://")
        || spec.starts_with("file://")
        || spec.starts_with("git@")
        || Path::new(spec).is_absolute()
    {
        return Ok(spec.to_string());
    }

    let (provider, path) = match spec.split_once(':') {
        Some((provider, path)) => (provider, path),
        None => ("github", spec)
    };
    let base = match provider {
        "gh" | "github" => "https://github.com/",
        "gl" | "gitlab" => "https://gitlab.com/",
        "cb" | "codeberg" => "https://codeberg.org/",
        _ => return Err(anyhow!("Unknown repository provider: {provider}"))
    };
    Ok(format!("{base}{}.git", path.trim_end_matches('/')))
}

/// Parses a user repository specification into its URL and package selector.
///
/// # Errors
///
/// Returns an error when the provider or repository specification is invalid.
fn parse_repo_spec(spec: &str) -> Result<RepoSpec> {
    if spec.starts_with("http://")
        || spec.starts_with("https://")
        || spec.starts_with("file://")
        || spec.starts_with("git@")
    {
        let (url, selector) = spec
            .split_once('#')
            .map_or((spec, None), |(url, selector)| {
                (url, Some(selector.to_string()))
            });
        return Ok(RepoSpec {
            url: url.to_string(),
            selector
        });
    }

    let (provider, remainder) = match spec.split_once(':') {
        Some((
            provider @ ("gh" | "github" | "gl" | "gitlab" | "cb" | "codeberg"),
            remainder
        )) => (provider, remainder),
        Some((provider, _)) => {
            return Err(anyhow!("Unknown repository provider: {provider}"));
        }
        None => ("github", spec)
    };
    let (path, selector) = match remainder.rsplit_once(':') {
        Some((path, selector)) => (path, Some(selector.to_string())),
        None => (remainder, None)
    };
    if path.is_empty() || selector.as_ref().is_some_and(String::is_empty) {
        return Err(anyhow!("Invalid repository specification: {spec}"));
    }

    let base = match provider {
        "gh" | "github" => "https://github.com/",
        "gl" | "gitlab" => "https://gitlab.com/",
        "cb" | "codeberg" => "https://codeberg.org/",
        _ => unreachable!("provider was validated")
    };
    Ok(RepoSpec {
        url: format!("{base}{}.git", path.trim_end_matches('/')),
        selector
    })
}

/// Confirms access to an untrusted repository source.
///
/// # Errors
///
/// Returns an error when the user declines the source.
pub fn confirm_repository_source(spec: &str, yes: bool) -> Result<()> {
    zoi_core::utils::confirm_untrusted_source(
        &SourceType::GitRepo(spec.to_string()),
        yes
    )
}

/// Prints the selected repository source for CLI output.
pub fn print_prepared_source(source: &str) {
    println!("Using cloned package source: {}", source.cyan());
}
