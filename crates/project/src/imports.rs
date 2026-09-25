//! Repository import resolution and materialization.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha512};
use zoi_core::types::{LockImportV2, ZoiLockV2};

use crate::config::{ImportSpec, PackageExportSpec, ProjectConfig};

/// Resolves a repository shorthand into a cloneable URL.
///
/// # Errors
///
/// Returns an error when the shorthand uses an unknown provider.
fn repository_url(spec: &str) -> Result<String> {
    if spec.starts_with("http://")
        || spec.starts_with("https://")
        || spec.starts_with("file://")
        || spec.starts_with("git@")
        || Path::new(spec).is_absolute()
    {
        return Ok(spec.to_string());
    }
    let (provider, path) = spec
        .split_once(':')
        .map_or(("github", spec), |(provider, path)| (provider, path));
    let base = match provider {
        "gh" | "github" => "https://github.com/",
        "gl" | "gitlab" => "https://gitlab.com/",
        "cb" | "codeberg" => "https://codeberg.org/",
        _ => return Err(anyhow!("Unknown repository provider: {provider}"))
    };
    Ok(format!("{base}{}.git", path.trim_end_matches('/')))
}

/// Clones a repository and optionally checks out a requested revision.
///
/// # Errors
///
/// Returns an error if repository preparation, cloning, or checkout fails.
fn clone_repository(
    repository: &str,
    destination: &Path,
    revision: Option<&str>
) -> Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if destination.exists() {
        std::fs::remove_dir_all(destination)?;
    }
    let output = Command::new("git")
        .arg("clone")
        .arg("--no-tags")
        .arg(repository_url(repository)?)
        .arg(destination)
        .output()
        .map_err(|error| {
            anyhow!("Failed to clone import '{repository}': {error}")
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to clone import '{repository}': {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if let Some(revision) = revision {
        checkout_revision(destination, revision)?;
    }
    Ok(())
}

/// Checks out a detached repository revision.
///
/// # Errors
///
/// Returns an error if Git cannot check out the requested revision.
fn checkout_revision(repository: &Path, revision: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .arg("checkout")
        .arg("--detach")
        .arg(revision)
        .output()
        .map_err(|error| {
            anyhow!("Failed to checkout import revision: {error}")
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to checkout import revision '{revision}': {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Reads the currently checked-out revision from a repository.
///
/// # Errors
///
/// Returns an error if Git cannot determine the repository revision.
fn repository_revision(repository: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| anyhow!("Failed to read import revision: {error}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to read import revision: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// Computes the SHA-512 content hash of a file.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
fn file_hash(path: &Path) -> Result<String> {
    Ok(format!(
        "sha512-{}",
        hex::encode(Sha512::digest(std::fs::read(path)?))
    ))
}

/// Selects the key holding a named export of a repository manifest.
///
/// Export sources may point at nested imports, so the selected key is used to
/// look up an already resolved path instead of the raw manifest source.
///
/// # Errors
///
/// Returns an error if the requested export does not exist.
fn select_export_key<'a>(
    exports: &'a PackageExportSpec,
    selector: &str
) -> Result<&'a str> {
    if exports.main.is_some()
        && (selector == "main" || exports.packages.is_empty())
    {
        return Ok("main");
    }
    let (key, _) = exports
        .packages
        .get_key_value(selector)
        .ok_or_else(|| unavailable_export(selector, exports))?;
    Ok(key.as_str())
}

/// Builds the error listing the exports a repository makes available.
fn unavailable_export(
    selector: &str,
    exports: &PackageExportSpec
) -> anyhow::Error {
    let mut available: Vec<_> = exports.packages.keys().cloned().collect();
    if exports.main.is_some() {
        available.insert(0, "main".to_string());
    }
    anyhow!(
        "Imported package export '{selector}' was not found. Available: {}",
        available.join(", ")
    )
}

/// Selects an exported package source by name or the default main export.
///
/// # Errors
///
/// Returns an error if the requested export does not exist.
fn select_export(
    exports: &PackageExportSpec,
    selector: &str
) -> Result<String> {
    let key = select_export_key(exports, selector)?;
    if key == "main"
        && let Some(main) = &exports.main
    {
        return Ok(main.clone());
    }
    exports
        .packages
        .get(key)
        .cloned()
        .ok_or_else(|| unavailable_export(selector, exports))
}

/// Resolves an export source to a path contained by its repository root.
///
/// # Errors
///
/// Returns an error if the source is missing, ambiguous, or outside the root.
fn resolve_export_path(root: &Path, source: &str) -> Result<PathBuf> {
    let source = source.strip_prefix("zoi:").unwrap_or(source);
    let path = root.join(source);
    if path.is_file() {
        return ensure_within(root, &path);
    }
    if path.is_dir() {
        let conventional = path.join(format!(
            "{}.pkg.lua",
            path.file_name().and_then(|name| name.to_str()).ok_or_else(
                || anyhow!("Imported package folder has an invalid name")
            )?
        ));
        if conventional.is_file() {
            return ensure_within(root, &conventional);
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
        let candidate = candidates.first().cloned().ok_or_else(|| {
            anyhow!("Imported package folder has no .pkg.lua file")
        })?;
        if candidates.len() != 1 {
            return Err(anyhow!(
                "Imported package folder '{}' is ambiguous",
                path.display()
            ));
        }
        return ensure_within(root, &candidate);
    }
    let flat = root.join(format!("{source}.pkg.lua"));
    if flat.is_file() {
        return ensure_within(root, &flat);
    }
    Err(anyhow!(
        "Imported package source '{source}' does not exist in '{}'",
        root.display()
    ))
}

/// Canonicalizes a path and verifies that it remains within the root.
///
/// # Errors
///
/// Returns an error if canonicalization fails or the path escapes the root.
fn ensure_within(root: &Path, path: &Path) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    if !path.starts_with(&root) {
        return Err(anyhow!(
            "Imported package source '{}' escapes '{}'",
            path.display(),
            root.display()
        ));
    }
    Ok(path)
}

/// Splits a package reference into its path and optional version suffix.
fn split_reference(source: &str) -> (&str, Option<&str>) {
    source
        .rsplit_once('@')
        .filter(|(path, _)| !path.contains("://"))
        .map_or((source, None), |(path, version)| (path, Some(version)))
}

/// Formats a package path with an optional version suffix.
fn normalize_reference(path: &Path, suffix: Option<&str>) -> String {
    let mut source = path.to_string_lossy().into_owned();
    if let Some(version) = suffix {
        source.push('@');
        source.push_str(version);
    }
    source
}

/// Builds the stable lockfile key for an import at a given nesting level.
///
/// Top-level imports are keyed by their alias, while nested imports extend the
/// key of the import that declares them (for example `base/nested`).
fn import_key(key_prefix: &str, alias: &str) -> String {
    if key_prefix.is_empty() {
        alias.to_string()
    } else {
        format!("{key_prefix}/{alias}")
    }
}

/// Ensures an import alias is usable as a single path segment and lock key.
///
/// # Errors
///
/// Returns an error if the alias is empty or traverses outside its parent
/// directory.
fn validate_import_alias(alias: &str) -> Result<()> {
    if alias.is_empty()
        || alias.contains(['/', '\\'])
        || alias == "."
        || alias == ".."
    {
        return Err(anyhow!("Invalid import name '{alias}'"));
    }
    Ok(())
}

/// Splits an `alias:selector` reference from a plain export source.
fn split_alias_selector(source: &str) -> Option<(&str, &str)> {
    let (alias, selector) = source.split_once(':')?;
    if alias.is_empty() || selector.is_empty() || alias.contains(['/', '\\']) {
        return None;
    }
    Some((alias, selector))
}

/// Resolves one declared export source of a repository manifest.
///
/// A source is either a path inside the repository or an `alias:selector`
/// reference into one of the nested imports declared by the same manifest.
///
/// # Errors
///
/// Returns an error if the nested alias, export selector, or local source
/// cannot be resolved.
fn resolve_export_source(
    root: &Path,
    nested: &BTreeMap<String, ResolvedImport>,
    source: &str
) -> Result<PathBuf> {
    if let Some((alias, selector)) = split_alias_selector(source) {
        let import = nested
            .get(alias)
            .ok_or_else(|| anyhow!("Unknown import alias '{alias}'"))?;
        let key = select_export_key(&import.exports, selector)?;
        let path = import
            .export_paths
            .get(key)
            .ok_or_else(|| unavailable_export(selector, &import.exports))?;
        return Ok(PathBuf::from(path));
    }
    resolve_export_path(root, source)
}

/// Resolves every export declared by a repository manifest into a concrete
/// path.
///
/// The `main` key always holds the default export, even for manifests that only
/// declare named packages, so `alias:main` keeps working for both forms.
///
/// # Errors
///
/// Returns an error if the manifest declares no export or if one of the
/// declared sources cannot be resolved.
fn resolve_export_paths(
    root: &Path,
    nested: &BTreeMap<String, ResolvedImport>,
    exports: &PackageExportSpec
) -> Result<BTreeMap<String, String>> {
    let default_selector = if exports.main.is_some() {
        "main".to_string()
    } else {
        exports.packages.keys().next().cloned().unwrap_or_default()
    };
    let mut paths = BTreeMap::new();
    paths.insert(
        "main".to_string(),
        resolve_export_source(
            root,
            nested,
            &select_export(exports, &default_selector)?
        )?
        .to_string_lossy()
        .into_owned()
    );
    for (selector, source) in &exports.packages {
        paths.insert(
            selector.clone(),
            resolve_export_source(root, nested, source)?
                .to_string_lossy()
                .into_owned()
        );
    }
    Ok(paths)
}

/// A repository import that has been cloned, verified, and fully expanded.
struct ResolvedImport {
    /// The package exports declared by the repository manifest.
    exports: PackageExportSpec,
    /// Resolved export paths keyed by export selector.
    export_paths: BTreeMap<String, String>
}

/// Resolves an import tree declared by a project or repository manifest.
///
/// The resolver walks imports recursively: every cloned repository may declare
/// further imports, which are materialized below the parent import and recorded
/// under a nested lock key.
struct ImportResolver<'a> {
    /// Lockfile pinning revisions and manifest hashes in frozen mode.
    frozen_lock: Option<&'a ZoiLockV2>,
    /// Canonical repository URLs currently being resolved, used to break
    /// cycles.
    active: Vec<String>,
    /// Lock entries for every resolved import, keyed by nested import key.
    locked: BTreeMap<String, LockImportV2>
}

impl ImportResolver<'_> {
    /// Resolves every import declared at a single nesting level.
    ///
    /// # Errors
    ///
    /// Returns an error if any import at this level cannot be resolved.
    fn resolve_level(
        &mut self,
        imports: &BTreeMap<String, ImportSpec>,
        parent_dir: &Path,
        key_prefix: &str
    ) -> Result<BTreeMap<String, ResolvedImport>> {
        let mut level = BTreeMap::new();
        for (alias, import) in imports {
            validate_import_alias(alias)?;
            let key = import_key(key_prefix, alias);
            let destination =
                parent_dir.join(".zoi").join("imports").join(alias);
            let resolved = self.resolve_import(&key, import, &destination)?;
            level.insert(alias.clone(), resolved);
        }
        Ok(level)
    }

    /// Clones, verifies, and expands one import, rejecting repository cycles.
    ///
    /// # Errors
    ///
    /// Returns an error if the import closes a repository cycle or if expanding
    /// the repository fails.
    fn resolve_import(
        &mut self,
        key: &str,
        import: &ImportSpec,
        destination: &Path
    ) -> Result<ResolvedImport> {
        // Cycle detection keys on the canonical repository URL rather than the
        // alias, so `a` importing `b` importing `a` is caught even when the two
        // manifests spell the repository differently.
        let url = repository_url(&import.repo)?;
        if self.active.contains(&url) {
            return Err(anyhow!(
                "Import cycle detected: import '{key}' resolves to '{url}', \
                 which is already being imported"
            ));
        }
        self.active.push(url);
        let outcome = self.expand_import(key, import, destination);
        self.active.pop();
        outcome
    }

    /// Materializes one import and recursively expands its nested imports.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be cloned or verified, if its
    /// manifest is missing or declares no package, or if one of its exports
    /// cannot be resolved.
    fn expand_import(
        &mut self,
        key: &str,
        import: &ImportSpec,
        destination: &Path
    ) -> Result<ResolvedImport> {
        let locked = self.frozen_lock.and_then(|lock| lock.imports.get(key));
        let revision = locked
            .map(|entry| entry.resolved_revision.as_str())
            .or(import.rev.as_deref());
        clone_repository(&import.repo, destination, revision)?;
        let actual_revision = repository_revision(destination)?;
        if let Some(expected) =
            locked.map(|entry| entry.resolved_revision.as_str())
            && expected != actual_revision
        {
            return Err(anyhow!(
                "Import '{key}' resolved to {actual_revision}, expected \
                 {expected}"
            ));
        }
        let manifest_path = destination.join("zoi.lua");
        if !manifest_path.is_file() {
            return Err(anyhow!(
                "Imported repository '{}' has no zoi.lua",
                import.repo
            ));
        }
        let manifest_hash = file_hash(&manifest_path)?;
        if let Some(expected) = locked.map(|entry| entry.manifest_hash.as_str())
            && expected != manifest_hash
        {
            return Err(anyhow!(
                "Import '{key}' manifest hash does not match zoi.lock"
            ));
        }
        let (nested_imports, exports) =
            crate::lua_config::load_repo_zoi_lua(&manifest_path)?;
        let exports = exports.ok_or_else(|| {
            anyhow!("Imported repository '{}' declares no package", import.repo)
        })?;
        let nested = self.resolve_level(&nested_imports, destination, key)?;
        let export_paths =
            resolve_export_paths(destination, &nested, &exports)?;
        self.locked.insert(
            key.to_string(),
            LockImportV2 {
                repo: import.repo.clone(),
                requested_revision: import.rev.clone(),
                resolved_revision: actual_revision,
                manifest_hash,
                path: None,
                exports: export_paths.clone()
            }
        );
        Ok(ResolvedImport {
            exports,
            export_paths
        })
    }
}

/// Resolves repository imports declared by a project `zoi.lua`.
///
/// Nested imports declared by an imported repository are materialized below
/// their parent and recorded under a nested lock key.
///
/// # Errors
///
/// Returns an error if a repository cannot be cloned, pinned, parsed, if the
/// import graph contains a cycle, or if a referenced package export does not
/// exist.
pub fn resolve(
    config: &mut ProjectConfig,
    project_root: &Path
) -> Result<BTreeMap<String, LockImportV2>> {
    let frozen_lock = if zoi_core::frozen::is_frozen() {
        Some(crate::lockfile::read_zoi_lock()?)
    } else {
        None
    };
    let mut resolver = ImportResolver {
        frozen_lock: frozen_lock.as_ref(),
        active: Vec::new(),
        locked: BTreeMap::new()
    };
    let imports = resolver.resolve_level(&config.imports, project_root, "")?;

    for source in &mut config.pkgs {
        *source = resolve_source_reference(source, &imports)?;
    }
    let old_packages = config.pkgs_v2.clone();
    config.pkgs_v2.clear();
    for (source, spec) in old_packages {
        let renamed = resolve_source_reference(&source, &imports)?;
        config.pkgs_v2.insert(renamed, spec);
    }
    Ok(resolver.locked)
}

/// Resolves an import alias and export selector to a package source reference.
///
/// # Errors
///
/// Returns an error if the alias, export, or source path cannot be resolved.
fn resolve_source_reference(
    source: &str,
    imports: &BTreeMap<String, ResolvedImport>
) -> Result<String> {
    let (source, suffix) = split_reference(source);
    let Some((alias, selector)) = source.split_once(':') else {
        return Ok(match suffix {
            Some(version) => format!("{source}@{version}"),
            None => source.to_string()
        });
    };
    let import = imports
        .get(alias)
        .ok_or_else(|| anyhow!("Unknown import alias '{alias}'"))?;
    let key = select_export_key(&import.exports, selector)?;
    let path = import
        .export_paths
        .get(key)
        .ok_or_else(|| unavailable_export(selector, &import.exports))?;
    Ok(normalize_reference(Path::new(path), suffix))
}

/// Reconstructs locked imports below `.zoi/imports` for frozen installs.
///
/// Nested imports are materialized below their parent import, mirroring the
/// layout produced by [`resolve`].
///
/// # Errors
///
/// Returns an error if a locked repository, revision, or manifest cannot be
/// reproduced.
pub fn materialize_locked(lock: &ZoiLockV2, project_root: &Path) -> Result<()> {
    materialize_locked_level(lock, "", project_root)
}

/// Reconstructs one nesting level of locked imports and recurses into children.
///
/// # Errors
///
/// Returns an error if a locked repository, revision, or manifest cannot be
/// reproduced.
fn materialize_locked_level(
    lock: &ZoiLockV2,
    key_prefix: &str,
    parent_dir: &Path
) -> Result<()> {
    let child_prefix = if key_prefix.is_empty() {
        String::new()
    } else {
        format!("{key_prefix}/")
    };
    for (key, import) in &lock.imports {
        // Keys without `/` are top-level imports; deeper keys belong to a
        // nested level and are handled by the recursive call below.
        let Some(alias) = key.strip_prefix(child_prefix.as_str()) else {
            continue;
        };
        if alias.is_empty() || alias.contains('/') {
            continue;
        }
        validate_import_alias(alias)?;
        let destination = parent_dir.join(".zoi").join("imports").join(alias);
        clone_repository(
            &import.repo,
            &destination,
            Some(&import.resolved_revision)
        )?;
        let actual_revision = repository_revision(&destination)?;
        if actual_revision != import.resolved_revision {
            return Err(anyhow!(
                "Import '{key}' resolved to {actual_revision}, expected {}",
                import.resolved_revision
            ));
        }
        let manifest_path =
            destination.join(import.path.as_deref().unwrap_or("zoi.lua"));
        if file_hash(&manifest_path)? != import.manifest_hash {
            return Err(anyhow!(
                "Import '{key}' manifest hash does not match zoi.lock"
            ));
        }
        materialize_locked_level(lock, key, &destination)?;
    }
    Ok(())
}
