//! Command for displaying manual pages for packages.
//!
//! Manual pages staged with `zman()` are opened with the system's `man`
//! viewer, falling back to `info` and finally to plain stdout. `--page`
//! selects which page to show when a package ships several, and `--raw`
//! prints the raw page source so it can be piped elsewhere.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Result, anyhow};
use walkdir::WalkDir;

use crate::pkg::types::{self};
use crate::pkg::{db, local, resolve};

/// Runs the manual page viewer for the specified package.
///
/// Without `--page`, the page matching the package name in section 1
/// (`<name>.1`) is shown. Pass `--raw` to print the raw page source to
/// stdout instead of opening a viewer.
///
/// # Errors
///
/// Returns an error if:
/// - The package cannot be resolved.
/// - No manual pages can be found, or the requested `--page` does not exist.
/// - The selected viewer fails.
pub fn run(
    package_name: &str,
    upstream: bool,
    raw: bool,
    page: Option<&str>
) -> Result<()> {
    let (pkg, registry_handle) = resolve_package_for_man(package_name)?;

    let pages =
        gather_manual_pages(&pkg, registry_handle.as_deref(), upstream)?;

    if pages.is_empty() {
        return Err(anyhow!(
            "Package '{}' does not have any manual pages.",
            pkg.name
        ));
    }

    let (name, content) = select_page(&pages, &pkg.name, page)?;

    if raw {
        write_stdout_lossless(content)?;
        return Ok(());
    }

    show_with_viewer(name, content)?;
    Ok(())
}

/// Picks which manual page to display.
///
/// An explicit `--page` value matches a page file name exactly
/// (e.g. `zbsdiff.3`). Without it, the `<package>.1` page is preferred;
/// a lone page is used directly regardless of its name.
///
/// # Errors
///
/// Returns an error if the requested page does not exist, or if no page
/// can be chosen by default. The error lists the available pages.
pub fn select_page<'a>(
    pages: &'a BTreeMap<String, String>,
    pkg_name: &str,
    requested: Option<&str>
) -> Result<(&'a String, &'a String)> {
    let available = || {
        let mut names: Vec<&str> = pages.keys().map(String::as_str).collect();
        names.sort_unstable();
        names.join(", ")
    };

    if let Some(want) = requested {
        if let Some((key, content)) =
            pages.iter().find(|(key, _)| page_key_matches(key, want))
        {
            return Ok((key, content));
        }
        return Err(anyhow!(
            "Package '{pkg_name}' has no manual page '{want}'. Available: {}",
            available()
        ));
    }

    let default = format!("{pkg_name}.1");
    if let Some((key, content)) = pages
        .iter()
        .find(|(key, _)| page_key_matches(key, &default))
    {
        return Ok((key, content));
    }

    if let Some((key, content)) = pages.iter().next() {
        return Ok((key, content));
    }

    Err(anyhow!(
        "Package '{pkg_name}' has no '{default}' page. Pick one with --page. \
         Available: {}",
        available()
    ))
}

/// Checks whether a stored page key refers to the requested page name.
///
/// Upstream pages may carry a `[sub:Scope]` suffix on their key, so both
/// the full key and the suffix-stripped form are accepted.
fn page_key_matches(key: &str, want: &str) -> bool {
    key == want || strip_page_suffix(key) == want
}

/// Removes a trailing `[sub:Scope]` display suffix from a page key.
fn strip_page_suffix(key: &str) -> &str {
    if let Some(idx) = key.find('[') {
        key[..idx].trim_end()
    } else {
        key
    }
}

/// Displays a page with the system's `man` viewer, falling back to `info`
/// and finally to plain stdout when neither viewer is installed.
///
/// The viewer runs in the foreground so it must finish before the
/// temporary file holding the page is cleaned up.
///
/// # Errors
///
/// Returns an error if the temporary file cannot be written or a viewer
/// fails after starting successfully.
fn show_with_viewer(name: &str, content: &str) -> Result<()> {
    let dir = tempfile::Builder::new().prefix("zoi-man-").tempdir()?;
    let path = dir.path().join(sanitize_filename(name));
    fs::write(&path, content)?;

    match try_viewer("man", &path)? {
        ViewerOutcome::Shown => return Ok(()),
        ViewerOutcome::Missing => {}
    }
    match try_viewer("info", &path)? {
        ViewerOutcome::Shown => return Ok(()),
        ViewerOutcome::Missing => {}
    }

    eprintln!(
        "Neither 'man' nor 'info' is installed; printing the raw page instead."
    );
    write_stdout_lossless(content)?;
    Ok(())
}

/// Outcome of attempting to display a page with an external viewer.
enum ViewerOutcome {
    /// The viewer displayed the page.
    Shown,
    /// The viewer program is not installed.
    Missing
}

/// Runs an external viewer (`man`, `info`) on a page file and waits for it.
///
/// # Errors
///
/// Returns an error if the viewer starts but exits unsuccessfully.
fn try_viewer(program: &str, path: &Path) -> Result<ViewerOutcome> {
    let mut child = match std::process::Command::new(program).arg(path).spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(ViewerOutcome::Missing);
        }
        Err(e) => {
            return Err(anyhow!("Failed to spawn '{program}': {e}"));
        }
    };
    let status = child.wait()?;
    if status.success() {
        Ok(ViewerOutcome::Shown)
    } else {
        Err(anyhow!("'{program}' exited with status {status}"))
    }
}

/// Makes a page key safe to use as a temporary file name while keeping its
/// man section extension (e.g. `zbsdiff.1[main:User]` becomes
/// `zbsdiff.1_main_User`).
fn sanitize_filename(name: &str) -> PathBuf {
    let mut sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    while sanitized.ends_with(['_', '.']) {
        sanitized.pop();
    }
    if sanitized.is_empty() {
        sanitized.push_str("manpage");
    }
    PathBuf::from(sanitized)
}

/// Writes to stdout without panicking on a closed pipe, so invocations like
/// `zoi man <pkg> --raw | man` or `... | head` exit quietly instead of
/// panicking with a broken pipe error.
///
/// # Errors
///
/// Returns an error for any I/O failure other than a broken pipe.
fn write_stdout_lossless(content: &str) -> Result<()> {
    use std::io::Write;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match out.write_all(content.as_bytes()) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
        Err(e) => return Err(e.into())
    }
    match out.flush() {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
        Err(e) => return Err(e.into())
    }
    Ok(())
}

/// Resolves a package and optional registry handle for a given search term.
///
/// # Errors
///
/// Returns an error if:
/// - The package or binary cannot be found.
/// - The project configuration cannot be read.
pub fn resolve_package_for_man(
    term: &str
) -> Result<(types::Package, Option<String>)> {
    if let Ok((pkg, _, _, _, registry_handle, _, _)) =
        resolve::resolve_package_and_version(term, None, false, false)
    {
        return Ok((pkg, registry_handle));
    }

    let config = crate::pkg::config::read_config()?;
    let mut registries = Vec::new();
    if let Some(default) = &config.default_registry {
        registries.push(default.handle.clone());
    }
    for reg in &config.added_registries {
        registries.push(reg.handle.clone());
    }

    for handle in registries {
        if let Ok(results) = db::find_provides(&handle, term)
            && let Some(result) = results.first()
        {
            return Ok((result.0.clone(), Some(handle)));
        }
    }

    Err(anyhow!("Could not find package or binary named '{term}'."))
}

/// Gathers raw manual page sources for a package, checking locally first
/// and then upstream.
///
/// Pages are returned as `file name -> raw content` with no reformatting,
/// so they stay pipeable into `man` and render natively in viewers.
/// Informational notes go to stderr to keep stdout clean for `--raw`.
///
/// # Errors
///
/// Returns an error if:
/// - Local manual pages cannot be found.
/// - Upstream manual pages cannot be gathered.
pub fn gather_manual_pages(
    pkg: &types::Package,
    registry_handle: Option<&str>,
    upstream: bool
) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();

    if !upstream {
        let handle = registry_handle.unwrap_or("local");
        let scopes_to_check = [
            types::Scope::Project,
            types::Scope::User,
            types::Scope::System
        ];

        for scope in scopes_to_check {
            if let Ok(package_dir) =
                local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)
            {
                let latest_dir = package_dir.join("latest");
                if latest_dir.exists() {
                    let local_pages = find_local_man_pages(&latest_dir)?;
                    if !local_pages.is_empty() {
                        eprintln!(
                            "Displaying locally installed manual from \
                             {scope:?} scope..."
                        );
                        pages.extend(local_pages);
                        break;
                    }
                }
            }

            // Also check standard system locations if scope is system
            if scope == types::Scope::System {
                let system_man = Path::new("/usr/share/man");
                if system_man.exists() {
                    let system_pages =
                        find_man_pages_in_hierarchy(system_man, &pkg.name)?;
                    if !system_pages.is_empty() {
                        eprintln!(
                            "Displaying manual from system /usr/share/man..."
                        );
                        pages.extend(system_pages);
                        break;
                    }
                }
            }
        }
    }

    if pages.is_empty() {
        eprintln!(
            "Package not installed or local manual not found. Fetching from \
             upstream..."
        );
        let upstream_pages =
            gather_manual_pages_from_upstream(pkg, registry_handle)?;
        pages.extend(upstream_pages);
    }

    Ok(pages)
}

/// Recursively finds manual pages in a directory hierarchy.
///
/// # Errors
///
/// Returns an error if directory traversal or file reading fails.
fn find_man_pages_in_hierarchy(
    root: &Path,
    term: &str
) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();
    if !root.exists() {
        return Ok(pages);
    }

    for entry in WalkDir::new(root).max_depth(3) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with(term) {
                let content = fs::read_to_string(entry.path())?;
                pages.insert(name.to_string(), content);
            }
        }
    }
    Ok(pages)
}

/// Fetches manual pages for a package from the upstream registry.
///
/// # Errors
///
/// Returns an error if:
/// - Dependency resolution fails.
/// - The package archive cannot be downloaded or extracted.
/// - File reading fails.
///
/// # Panics
///
/// Panics if the internal dependency graph is inconsistent.
fn gather_manual_pages_from_upstream(
    pkg: &types::Package,
    registry_handle: Option<&str>
) -> Result<BTreeMap<String, String>> {
    // Resolve the package to get its archive source
    let source = registry_handle.map_or_else(
        || pkg.name.clone(),
        |handle| format!("#{handle}@{}", pkg.name)
    );

    let (mut graph, _) =
        crate::pkg::install::resolver::resolve_dependency_graph(
            &[source],
            None,
            false,
            true,
            true,
            None,
            true,
            None
        )?;

    if graph.nodes.is_empty() {
        return Ok(BTreeMap::new());
    }

    let node_id = graph
        .nodes
        .keys()
        .next()
        .expect("Graph should not be empty")
        .clone();
    let node = graph
        .nodes
        .remove(&node_id)
        .expect("Node should exist in graph");

    let install_plan = crate::pkg::install::plan::create_install_plan(
        &HashMap::from([(node_id.clone(), node.clone())]),
        None,
        false
    )?;

    let action = install_plan
        .get(&node_id)
        .ok_or_else(|| anyhow!("No install action for package"))?;

    // Prepare the node (download/build)
    let prepared = crate::pkg::install::installer::prepare_node(
        &node, action, None, None, false, false
    )?;

    // Extract to a temp directory
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-man-extract-")
        .tempdir()?;
    let extract_path = temp_dir.path();

    if prepared.archive_path.exists() {
        let file = fs::File::open(&prepared.archive_path)?;
        let decoder = zstd::stream::read::Decoder::new(file)?;
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(extract_path)?;
    }

    // Look for man pages in the extracted content
    // We check:
    // - manifest.json (for pooled ZPA)
    // - data/pkgstore/man/
    // - data/usrroot/usr/share/man/
    // - any .pkg.lua in the root

    let mut pages = BTreeMap::new();

    let pooled_manifest = extract_path.join("manifest.json");
    if pooled_manifest.exists() {
        let content = fs::read_to_string(&pooled_manifest)?;
        let manifest: types::PooledZpaManifest =
            serde_json::from_str(&content)?;
        let pool_dir = extract_path.join("pool");

        for (sub_name, sub_mapping) in manifest.mappings {
            for (scope, scope_mapping) in sub_mapping.scopes {
                for file in scope_mapping.files {
                    if file.dest.contains("/man/")
                        || file.dest.ends_with(".1")
                        || file.dest.ends_with(".5")
                    {
                        let pool_file = pool_dir.join(&file.hash);
                        if pool_file.exists() {
                            let content = fs::read_to_string(pool_file)?;
                            let file_name = Path::new(&file.dest)
                                .file_name()
                                .expect("Dest should have a file name")
                                .to_string_lossy();
                            let display_name =
                                format!("{file_name}[{sub_name}:{scope:?}]");
                            pages.insert(display_name, content);
                        }
                    }
                }
            }
        }
    }

    let legacy_man = extract_path.join("data/pkgstore/man");
    if legacy_man.exists() {
        pages
            .extend(find_local_man_pages(&extract_path.join("data/pkgstore"))?);
    }

    Ok(pages)
}

/// Finds manual pages in a package's installation directory.
fn find_local_man_pages(latest_dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();

    let md_path = latest_dir.join("man.md");
    let txt_path = latest_dir.join("man.txt");

    if md_path.exists() {
        pages.insert("main".to_string(), fs::read_to_string(md_path)?);
        return Ok(pages);
    }

    if txt_path.exists() {
        pages.insert("main".to_string(), fs::read_to_string(txt_path)?);
        return Ok(pages);
    }

    let search_dirs =
        [latest_dir.join("share").join("man"), latest_dir.join("man")];

    for dir in search_dirs {
        if dir.exists() {
            for entry in WalkDir::new(dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .expect("Path should have a file name")
                        .to_string_lossy()
                        .to_string();
                    let content = fs::read_to_string(path)?;
                    pages.insert(name, content);
                }
            }
        }
    }

    Ok(pages)
}
