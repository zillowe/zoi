use anyhow::{Result, anyhow};
use colored::*;
use git2::{
    FetchOptions, RemoteCallbacks, Repository, ResetType,
    build::{CheckoutBuilder, RepoBuilder},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::offline;
use zoi_core::{config, pgp, sysroot, types, utils as core_utils};
use zoi_db as db;
use zoi_install::util as install_util;
use zoi_lua::parser as lua_parser;

/// Rebuilds the SQLite metadata database from the raw registry files.
///
/// This is the "Indexing Phase" of a sync. It:
/// - Scans the local Git clone for all `.pkg.lua` and `.sec.yaml` files.
/// - Parses each file (using the Lua VM where needed) to extract version info,
///   descriptions, dependencies, and security advisories.
/// - Fetches remote metadata (sizes and file lists) if configured in `repo.yaml`.
/// - Atomic Commit: Updates the SQLite tables within a single transaction.
fn refresh_registry_db(
    registry_handle: &str,
    registry_path: &Path,
    m: Option<&MultiProgress>,
    verbose: bool,
    pb: Option<&ProgressBar>,
) -> Result<()> {
    if verbose {
        let msg = format!(
            "Refreshing metadata database for {}...",
            registry_handle.cyan()
        );
        if let Some(m_ref) = m {
            let _ = m_ref.println(&msg);
        } else {
            println!("{}", msg);
        }
    }

    let mut conn = db::open_connection(registry_handle)?;
    db::clear_registry(&conn)?;

    let mut pkg_files = Vec::new();
    let mut sec_files = Vec::new();
    for entry in WalkDir::new(registry_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(".pkg.lua") {
                pkg_files.push(entry.path().to_path_buf());
            } else if name.ends_with(".sec.yaml") {
                sec_files.push(entry.path().to_path_buf());
            }
        }
    }

    let repo_config = config::read_repo_config(registry_path).ok();
    let _advisory_prefix = repo_config
        .as_ref()
        .and_then(|rc| rc.advisory_prefix.clone());
    let platform = core_utils::get_platform().unwrap_or_default();

    let has_size_tpl = repo_config
        .as_ref()
        .and_then(|rc| rc.pkg.iter().find(|p| p.link_type == "main"))
        .and_then(|p| p.size.as_ref())
        .is_some();

    let has_files_tpl = repo_config
        .as_ref()
        .and_then(|rc| rc.pkg.iter().find(|p| p.link_type == "main"))
        .and_then(|p| p.files.as_ref())
        .is_some();

    let client = if has_size_tpl || has_files_tpl {
        core_utils::get_http_client().ok()
    } else {
        None
    };

    if let Some(p) = pb {
        p.set_length(pkg_files.len() as u64);
        p.set_position(0);
        p.set_message(format!("Indexing {}", registry_handle.cyan()));
    }

    let parsed_results: Vec<(
        types::Package,
        PathBuf,
        Option<Vec<String>>,
        Option<(u64, u64)>,
        Option<String>,
    )> = pkg_files
        .par_iter()
        .filter_map(|path| {
            if let Some(p) = pb {
                p.inc(1);
            }
            let path_str = path.to_string_lossy();
            if let Ok(mut pkg) = lua_parser::parse_lua_package(&path_str, None, None, true) {
                if pkg.repo.is_empty()
                    && let Ok(rel_path) = path.strip_prefix(registry_path)
                    && let Some(parent) = rel_path.parent()
                {
                    let mut repo_path = parent.to_string_lossy().to_string().replace('\\', "/");
                    let pkg_name_suffix = format!("/{}", pkg.name);
                    if repo_path.ends_with(&pkg_name_suffix) {
                        repo_path =
                            repo_path[..repo_path.len() - pkg_name_suffix.len()].to_string();
                    } else if repo_path == pkg.name {
                        repo_path = String::new();
                    }
                    pkg.repo = repo_path;
                }

                let mut file_list = None;
                if let Some(c) = &client
                    && let Some(rc) = &repo_config
                    && let Some(pkg_link) = rc.pkg.iter().find(|p| p.link_type == "main")
                    && let Some(files_url_template) = &pkg_link.files
                {
                    let version = pkg.version.clone().unwrap_or_else(|| "latest".to_string());
                    let files_url = install_util::resolve_url_placeholders(
                        files_url_template,
                        &pkg.name,
                        &pkg.repo,
                        &version,
                        &platform,
                    );

                    if let Ok(response) = c.get(&files_url).send()
                        && response.status().is_success()
                        && let Ok(content) = response.text()
                    {
                        file_list = Some(
                            content
                                .lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect(),
                        );
                    }
                }

                let mut size_info = None;
                if let Some(c) = &client
                    && let Some(rc) = &repo_config
                    && let Some(pkg_link) = rc.pkg.iter().find(|p| p.link_type == "main")
                    && let Some(size_url_template) = &pkg_link.size
                {
                    let version = pkg.version.clone().unwrap_or_else(|| "latest".to_string());
                    let size_url = install_util::resolve_url_placeholders(
                        size_url_template,
                        &pkg.name,
                        &pkg.repo,
                        &version,
                        &platform,
                    );

                    if let Ok(response) = c.get(&size_url).send()
                        && response.status().is_success()
                        && let Ok(content) = response.text()
                    {
                        let mut download_size = 0u64;
                        let mut installed_size = 0u64;
                        for line in content.lines() {
                            if let Some((key, val)) = line.split_once(':')
                                && let Ok(num) = val.trim().parse::<u64>()
                            {
                                match key.trim() {
                                    "down" => download_size = num,
                                    "install" => installed_size = num,
                                    _ => {}
                                }
                            }
                        }
                        if download_size > 0 || installed_size > 0 {
                            size_info = Some((download_size, installed_size));
                        }
                    }
                }

                let mut hash_info = None;
                if let Some(c) = &client
                    && let Some(rc) = &repo_config
                    && let Some(pkg_link) = rc.pkg.iter().find(|p| p.link_type == "main")
                    && let Some(hash_url_template) = &pkg_link.hash
                {
                    let version = pkg.version.clone().unwrap_or_else(|| "latest".to_string());
                    let hash_url = install_util::resolve_url_placeholders(
                        hash_url_template,
                        &pkg.name,
                        &pkg.repo,
                        &version,
                        &platform,
                    );

                    if let Ok(response) = c.get(&hash_url).send()
                        && response.status().is_success()
                        && let Ok(content) = response.text()
                    {
                        let is_valid_hash = |s: &str| {
                            let len = s.len();
                            (len == 128 || len == 64 || len == 32)
                                && s.chars().all(|c| c.is_ascii_hexdigit())
                        };
                        for word in content.split_whitespace() {
                            if is_valid_hash(word) {
                                hash_info = Some(word.to_string());
                                break;
                            }
                        }
                    }
                }

                Some((pkg, path.clone(), file_list, size_info, hash_info))
            } else {
                None
            }
        })
        .collect();

    let parsed_advisories: Vec<(types::Advisory, String)> = sec_files
        .par_iter()
        .filter_map(|path| {
            if let Ok(content) = fs::read_to_string(path)
                && let Ok(advisory) = serde_yaml::from_str::<types::Advisory>(&content)
                && let Ok(rel_path) = path.strip_prefix(registry_path)
                && let Some(parent) = rel_path.parent()
            {
                let repo_path = parent.to_string_lossy().to_string().replace('\\', "/");
                return Some((advisory, repo_path));
            }
            None
        })
        .collect();

    let tx = conn.transaction()?;

    for (pkg, _path, file_list, size_info, hash_info) in parsed_results {
        let pkg_id = db::update_package(&tx, &pkg, registry_handle, None, None, None)?;

        if let Some((down_size, install_size)) = size_info {
            let _ = db::set_package_sizes(&tx, pkg_id, down_size, install_size);
        }

        if let Some(hash) = &hash_info {
            let _ = db::set_package_hash(&tx, pkg_id, hash);
        }

        if let Some(subs) = &pkg.sub_packages {
            for sub in subs {
                if let Err(e) =
                    db::update_package(&tx, &pkg, registry_handle, None, Some(sub), None)
                {
                    eprintln!(
                        "Warning: failed to sync sub-package '{}:{}': {}",
                        pkg.name, sub, e
                    );
                } else if let Ok(sub_id) =
                    db::get_package_id(&tx, &pkg.name, Some(sub), &pkg.repo, registry_handle)
                {
                    if let Some((down_size, install_size)) = &size_info {
                        let _ = db::set_package_sizes(&tx, sub_id, *down_size, *install_size);
                    }
                    if let Some(hash) = &hash_info {
                        let _ = db::set_package_hash(&tx, sub_id, hash);
                    }
                }
            }
        }

        if let Some(list) = file_list {
            let _ = db::index_package_files(&tx, pkg_id, &list);
        }
    }

    for (advisory, repo) in parsed_advisories {
        let _ = db::update_advisory(&tx, &advisory, &repo, registry_handle);
    }

    tx.commit()?;

    if let Some(p) = pb {
        p.finish_and_clear();
    }

    Ok(())
}

/// Verifies the PGP signature of the latest commit in a registry repository.
///
/// This ensures that the registry state hasn't been tampered with on the server.
/// If verification fails, the sync is aborted for security reasons.
fn verify_registry_signature(
    repo_path: &Path,
    authorities: &[String],
    verbose: bool,
) -> Result<()> {
    if authorities.is_empty() {
        return Ok(());
    }

    if verbose {
        println!("Verifying registry signature...");
    }

    let repo = Repository::open(repo_path)
        .map_err(|e| anyhow!("Failed to open registry repository: {}", e))?;
    let head = repo
        .head()
        .map_err(|e| anyhow!("Failed to get repository HEAD: {}", e))?;
    let target = head
        .target()
        .ok_or_else(|| anyhow!("HEAD is not a direct reference"))?;
    let commit = repo
        .find_commit(target)
        .map_err(|e| anyhow!("Failed to find HEAD commit: {}", e))?;

    let (sig, data) = repo
        .extract_signature(&commit.id(), None)
        .map_err(|_| anyhow!("Registry commit is not signed. Sync aborted for security."))?;

    let sig_bytes = &*sig;
    let data_bytes = &*data;

    let trusted_certs = pgp::get_certs_by_name_or_fingerprint(authorities)?;

    let mut verified = false;
    for cert in trusted_certs {
        if pgp::verify_detached_signature_raw(data_bytes, sig_bytes, &cert).is_ok() {
            verified = true;
            break;
        }
    }

    if verified {
        if verbose {
            println!("{}", "Registry signature verified successfully.".green());
        }
        Ok(())
    } else {
        Err(anyhow!(
            "Registry commit was signed but not by any authorized authority. Sync aborted."
        ))
    }
}

fn get_db_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("db"),
    ))
}

fn get_git_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("git"),
    ))
}

/// Synchronizes raw Git repositories that contain Zoi packages.
///
/// These are cloned into `~/.zoi/pkgs/git/` and are typically used for
/// personal or third-party package collections that are not full registries.
fn sync_git_repos(verbose: bool) -> Result<()> {
    if offline::is_offline() {
        println!(
            "\n{}",
            "Zoi is offline. Skipping sync of external git repositories.".yellow()
        );
        return Ok(());
    }
    let git_root = get_git_root()?;
    if !git_root.exists() {
        return Ok(());
    }

    if verbose {
        println!("\n{}", "Syncing external git repositories...".green());
    }

    let config = config::read_config()?;
    let configured_git_repos_names: HashSet<String> = config
        .git_repos
        .iter()
        .map(|url| {
            url.trim_end_matches('/')
                .split('/')
                .next_back()
                .unwrap_or_default()
                .trim_end_matches(".git")
                .to_string()
        })
        .collect();

    for entry in fs::read_dir(git_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join(".git").exists() {
            let Some(repo_name_os) = path.file_name() else {
                continue;
            };
            let repo_name = repo_name_os.to_string_lossy();

            if !configured_git_repos_names.contains(repo_name.as_ref()) {
                println!(
                    "Removing untracked git repository '{}'...",
                    repo_name.yellow()
                );
                fs::remove_dir_all(&path)?;
                continue;
            }

            println!("Pulling changes for '{}'...", repo_name.cyan());

            let mut cmd = Command::new("git");
            cmd.arg("-C").arg(&path).arg("pull");

            if verbose {
                let status = cmd
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !status.success() {
                    eprintln!(
                        "{}: Failed to pull changes for '{}'.",
                        "Warning".yellow(),
                        repo_name
                    );
                }
            } else {
                let output = cmd.output()?;
                if !output.status.success() {
                    eprintln!(
                        "{}: Failed to pull changes for '{}'.",
                        "Warning".yellow(),
                        repo_name
                    );
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
    }
    Ok(())
}

fn run_verbose_at_path(db_url: &str, db_path: &Path) -> Result<()> {
    if db_path.exists() {
        let status = Command::new("git")
            .arg("-C")
            .arg(db_path)
            .arg("pull")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(anyhow!(
                "Failed to pull changes from the remote repository."
            ));
        }
    } else {
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth=1")
            .arg("--progress")
            .arg(db_url)
            .arg(db_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(anyhow!("Failed to clone the package repository."));
        }
    }
    Ok(())
}

fn run_quiet_git_at_path(db_url: &str, db_path: &Path) -> Result<()> {
    if db_path.exists() {
        let output = Command::new("git")
            .arg("-C")
            .arg(db_path)
            .arg("pull")
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Failed to pull changes from the remote repository. {}",
                stderr.trim()
            ));
        }
    } else {
        let output = Command::new("git")
            .arg("clone")
            .arg("--depth=1")
            .arg("--quiet")
            .arg(db_url)
            .arg(db_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Failed to clone the package repository. {}",
                stderr.trim()
            ));
        }
    }
    Ok(())
}

fn run_non_verbose_at_path(
    db_url: &str,
    db_path: &Path,
    m: Option<&MultiProgress>,
    pb: Option<&ProgressBar>,
) -> Result<()> {
    let internal_m;
    let m_ref = if let Some(m_ptr) = m {
        m_ptr
    } else {
        internal_m = MultiProgress::new();
        &internal_m
    };

    let fetch_style = ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{elapsed_precise}] {msg:30.cyan} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
        )?
        .progress_chars("#>-");

    let pb_internal;
    let pb_to_use = if let Some(p) = pb {
        p
    } else {
        pb_internal = m_ref.add(ProgressBar::new(0));
        pb_internal.set_style(fetch_style);
        &pb_internal
    };

    if db_path.exists() {
        let repo = Repository::open(db_path)?;
        let mut remote = repo.find_remote("origin")?;

        let mut cb = RemoteCallbacks::new();
        let pb_clone = pb_to_use.clone();
        cb.transfer_progress(move |stats| {
            if stats.total_deltas() > 0 {
                pb_clone.set_length(stats.total_deltas() as u64);
                pb_clone.set_position(stats.indexed_deltas() as u64);
            }
            true
        });

        let head_symref = repo.find_reference("refs/remotes/origin/HEAD")?;
        let remote_default_ref = head_symref
            .symbolic_target()?
            .ok_or_else(|| anyhow!("Remote HEAD is not a symbolic ref"))?;
        let short_branch_name = remote_default_ref
            .strip_prefix("refs/remotes/origin/")
            .ok_or_else(|| anyhow!("Could not determine default branch name from remote HEAD"))?;

        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);
        pb_to_use.set_message(format!("Fetching {}", db_url.cyan()));
        remote.fetch(&[short_branch_name], Some(&mut fo), None)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_up_to_date() {
        } else if analysis.0.is_fast_forward() {
            let refname = format!("refs/heads/{}", short_branch_name);
            let mut reference = repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forwarding")?;
            repo.set_head(&refname)?;

            let mut checkout_builder = CheckoutBuilder::new();
            let pb_clone = pb_to_use.clone();
            checkout_builder.force().progress(move |_path, cur, total| {
                if total > 0 {
                    pb_clone.set_length(total as u64);
                    pb_clone.set_position(cur as u64);
                }
            });

            pb_to_use.set_message(format!("Checkout {}", db_url.cyan()));
            repo.checkout_head(Some(&mut checkout_builder))?;
        } else {
            println!(
                "{}",
                "Cannot fast-forward. Please run `git pull` manually.".yellow()
            );
        }
    } else {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut cb = RemoteCallbacks::new();
        let pb_clone = pb_to_use.clone();
        cb.transfer_progress(move |stats| {
            if stats.total_deltas() > 0 {
                pb_clone.set_length(stats.total_deltas() as u64);
            }
            pb_clone.set_position(stats.indexed_deltas() as u64);
            true
        });

        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);

        let mut checkout_builder = CheckoutBuilder::new();
        let pb_clone = pb_to_use.clone();
        checkout_builder.progress(move |_path, cur, total| {
            if total > 0 {
                pb_clone.set_length(total as u64);
            }
            pb_clone.set_position(cur as u64);
        });

        pb_to_use.set_message(format!("Cloning {}", db_url.cyan()));
        RepoBuilder::new()
            .fetch_options(fo)
            .with_checkout(checkout_builder)
            .clone(db_url, db_path)?;
    }

    Ok(())
}

fn try_sync_at_path(
    db_url: &str,
    db_path: &Path,
    verbose: bool,
    m: Option<&MultiProgress>,
    pb: Option<&ProgressBar>,
) -> Result<()> {
    if offline::is_offline() {
        if db_path.exists() {
            let msg = format!(
                "Zoi is offline. Skipping update for existing registry at {}",
                db_path.display()
            );
            if let Some(m_ref) = m {
                let _ = m_ref.println(&msg);
            } else {
                println!("{}", msg);
            }
            return Ok(());
        } else {
            return Err(anyhow!(
                "Cannot sync registry '{}': Zoi is offline and registry is not cloned.",
                db_url
            ));
        }
    }
    if db_path.exists()
        && let Ok(repo) = Repository::open(db_path)
        && let Ok(remote) = repo.find_remote("origin")
        && let Ok(remote_url) = remote.url()
        && remote_url != db_url
    {
        let msg = format!(
            "Registry URL has changed from {}. Updating origin to {}.",
            remote_url.yellow(),
            db_url.cyan()
        );
        if let Some(m_ref) = m {
            m_ref.println(&msg)?;
        } else {
            println!("{}", msg);
        }
        repo.remote_set_url("origin", db_url)?;
    }

    if verbose {
        run_verbose_at_path(db_url, db_path)
    } else {
        match run_non_verbose_at_path(db_url, db_path, m, pb) {
            Ok(()) => Ok(()),
            Err(libgit_error) => {
                let msg = format!(
                    "Git progress sync failed for {}: {}. Retrying with system git...",
                    db_url.yellow(),
                    libgit_error
                );
                if let Some(p) = pb {
                    p.println(msg);
                } else if let Some(m_ref) = m {
                    m_ref.println(msg)?;
                } else {
                    eprintln!("{}", msg);
                }
                run_quiet_git_at_path(db_url, db_path)
            }
        }
    }
}

fn sync_pgp_keys_at_path(db_path: &Path, verbose: bool, pb: Option<&ProgressBar>) -> Result<()> {
    if verbose {
        println!("\n{}", "Syncing PGP keys from repository...".green());
    }
    if !db_path.join("repo.yaml").exists() {
        if verbose {
            println!("{}", "repo.yaml not found, skipping PGP key sync.".yellow());
        }
        return Ok(());
    }

    let repo_config = config::read_repo_config(db_path)?;

    if repo_config.pgp.is_empty() {
        if verbose {
            println!("No PGP keys defined in repo.yaml.");
        }
        return Ok(());
    }

    if let Some(p) = pb {
        p.set_length(repo_config.pgp.len() as u64);
        p.set_position(0);
        p.set_message(format!("PGP Keys {}", repo_config.name.cyan()));
    }

    for key_info in repo_config.pgp {
        let key_source = &key_info.key;
        let key_name = &key_info.name;

        if let Some(p) = pb {
            p.set_message(format!("PGP Key: {}", key_name));
        }

        let result = if key_source.starts_with("http") {
            pgp::add_key_from_url(key_source, key_name, !verbose)
        } else if key_source.len() == 40 && key_source.chars().all(|c| c.is_ascii_hexdigit()) {
            pgp::add_key_from_fingerprint(key_source, key_name, !verbose)
        } else {
            Err(anyhow!(
                "Invalid key source '{}': must be a URL or a 40-character fingerprint.",
                key_source
            ))
        };

        if let Err(e) = result {
            let err_msg = format!(
                "{} Failed to import key '{}': {}",
                "Warning:".yellow(),
                key_name,
                e
            );
            if let Some(p) = pb {
                p.println(err_msg);
            } else {
                eprintln!("{}", err_msg);
            }
        }
        if let Some(p) = pb {
            p.inc(1);
        }
    }

    Ok(())
}

fn fetch_handle_by_cloning(url: &str, verbose: bool) -> Result<String> {
    let temp_dir = Builder::new().prefix("zoi-handle-fetch").tempdir()?;
    if verbose {
        println!("Cloning '{}' to fetch handle...", url.cyan());
    }
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(url)
        .arg(temp_dir.path())
        .stdout(if verbose {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stderr(if verbose {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .status()?;

    if !status.success() {
        return Err(anyhow!("git clone failed to fetch handle"));
    }

    let repo_config = config::read_repo_config(temp_dir.path())?;
    Ok(repo_config.name)
}

fn parse_full_repo_url(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git").trim_end_matches('/');
    if let Some(path) = url.strip_prefix("https://github.com/") {
        Some(("github".to_string(), path.to_string()))
    } else if let Some(path) = url.strip_prefix("https://gitlab.com/") {
        Some(("gitlab".to_string(), path.to_string()))
    } else {
        url.strip_prefix("https://codeberg.org/")
            .map(|path| ("codeberg".to_string(), path.to_string()))
    }
}

fn fetch_repo_yaml_content(url: &str) -> Result<String> {
    let (provider, repo_path) = parse_full_repo_url(url)
        .ok_or_else(|| anyhow!("Unsupported git provider or URL format for direct fetch."))?;

    let branches = ["main", "master"];
    for branch in &branches {
        let repo_yaml_url = match provider.as_str() {
            "github" => format!(
                "https://raw.githubusercontent.com/{}/{}/repo.yaml",
                repo_path, branch
            ),
            "gitlab" => format!(
                "https://gitlab.com/{}/-/raw/{}/repo.yaml",
                repo_path, branch
            ),
            "codeberg" => format!(
                "https://codeberg.org/{}/raw/branch/{}/repo.yaml",
                repo_path, branch
            ),
            _ => continue,
        };

        let client = core_utils::get_http_client().ok();
        if let Some(c) = client
            && let Ok(response) = c.get(&repo_yaml_url).send()
            && response.status().is_success()
        {
            println!("Found repo.yaml at: {}", repo_yaml_url.cyan());
            return Ok(response.text()?);
        }
    }

    Err(anyhow!(
        "Could not find 'repo.yaml' in repo '{}' on branches main or master.",
        repo_path
    ))
}

fn fetch_handle_for_url(url: &str, verbose: bool) -> Result<String> {
    if verbose {
        println!(
            "Attempting to fetch handle for '{}' directly...",
            url.cyan()
        );
    }
    match fetch_repo_yaml_content(url) {
        Ok(content) => {
            let repo_config: types::RepoConfig = serde_yaml::from_str(&content)?;
            if verbose {
                println!("Successfully fetched and parsed repo.yaml.");
            }
            Ok(repo_config.name)
        }
        Err(e) => {
            if verbose {
                println!(
                    "Direct fetch failed: {}. Falling back to cloning repository...",
                    e.to_string().yellow()
                );
            }
            fetch_handle_by_cloning(url, verbose)
        }
    }
}

/// Synchronizes a single registry (default or added) with its remote Git source.
///
/// Logic Flow:
/// - Handle Resolution: If the handle is missing, it clones the repo to find it.
/// - Mirror Fallback: If the primary Git URL fails, it automatically tries mirrors
///   defined in the registry's `repo.yaml`.
/// - Signature Verification: If `authorities` are configured, it verifies the
///   signature of the latest commit to ensure the entire registry state is trusted.
/// - Key Sync: Automatically imports PGP keys defined in the registry's `repo.yaml`.
/// - Indexing: Triggers `refresh_registry_db` to update the local SQLite cache.
fn sync_registry(
    mut reg: types::Registry,
    db_root: &Path,
    verbose: bool,
    fallback: bool,
    m: Option<&MultiProgress>,
) -> Result<(types::Registry, bool)> {
    let mut reg_changed = false;

    let pb = if !verbose && let Some(m_ref) = m {
        let p = m_ref.add(ProgressBar::new(0));
        p.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {msg:30.cyan} [{bar:40.cyan/blue}] {percent}%",
                )?
                .progress_chars("#>-"),
        );
        p.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(p)
    } else {
        None
    };

    if reg.handle.is_empty() {
        if let Some(p) = &pb {
            p.set_message(format!("Fetching handle for {}", reg.url.cyan()));
        }
        let handle = fetch_handle_for_url(&reg.url, verbose)?;
        reg.handle = handle;
        reg_changed = true;
    }

    let target_dir = db_root.join(&reg.handle);

    let mut candidate_urls = vec![reg.url.clone()];

    if fallback
        && target_dir.exists()
        && let Ok(repo_config) = config::read_repo_config(&target_dir)
    {
        for git_link in repo_config.git.iter().filter(|g| g.link_type == "mirror") {
            if git_link.url != reg.url && !candidate_urls.contains(&git_link.url) {
                candidate_urls.push(git_link.url.clone());
            }
        }
    }

    let pre_sync_head = match Repository::open(&target_dir) {
        Ok(repo) => match repo.head() {
            Ok(head) => head.target(),
            Err(_) => None,
        },
        Err(_) => None,
    };

    let mut sync_success = false;
    let mut last_error = None;

    for url in candidate_urls {
        if let Err(e) = try_sync_at_path(&url, &target_dir, verbose, m, pb.as_ref()) {
            let msg = format!("Sync with {} failed: {}", url.yellow(), e);
            if let Some(p) = &pb {
                p.println(&msg);
            } else if let Some(m_ref) = m {
                let _ = m_ref.println(&msg);
            } else {
                eprintln!("{}", msg);
            }
            last_error = Some(e);
        } else {
            if url != reg.url {
                reg.url = url;
                reg_changed = true;
            }
            sync_success = true;
            break;
        }
    }

    if !sync_success {
        let e = last_error.unwrap_or_else(|| anyhow!("All sync candidates failed."));
        if let Some(p) = &pb {
            p.abandon_with_message("Sync failed.".red().to_string());
        }
        return Err(e);
    } else {
        if let Some(authorities) = &reg.authorities
            && let Err(e) = verify_registry_signature(&target_dir, authorities, verbose)
        {
            let rollback_msg = if let Some(oid) = pre_sync_head {
                if let Ok(repo) = Repository::open(&target_dir) {
                    if let Ok(object) = repo.find_object(oid, None) {
                        let mut checkout = CheckoutBuilder::new();
                        checkout.force();
                        if repo
                            .reset(&object, ResetType::Hard, Some(&mut checkout))
                            .is_ok()
                        {
                            "Rolled back to previous signed commit.".to_string()
                        } else {
                            "Failed to rollback. Repository may be in an inconsistent state."
                                .to_string()
                        }
                    } else {
                        "Could not find previous HEAD object.".to_string()
                    }
                } else {
                    "Could not open repository for rollback.".to_string()
                }
            } else {
                let _ = fs::remove_dir_all(&target_dir);
                "Removed unsigned clone.".to_string()
            };

            let msg = format!(
                "Security: Registry signature check failed for {}: {}. {}",
                reg.url.red(),
                e,
                rollback_msg.yellow(),
            );
            if let Some(m_ref) = m {
                m_ref.println(&msg)?;
            } else {
                eprintln!("{}", msg);
            }
            return Err(e);
        }

        sync_pgp_keys_at_path(&target_dir, verbose, pb.as_ref())?;

        let mut db_downloaded = false;
        if let Ok(repo_config) = config::read_repo_config(&target_dir)
            && let Some(db_url_template) = &repo_config.db
        {
            let platform = core_utils::get_platform().unwrap_or_default();
            let db_url =
                install_util::resolve_url_placeholders(db_url_template, "", "", "", &platform);

            if let Ok(db_path) = db::get_db_path(&reg.handle) {
                if let Some(p) = pb.as_ref() {
                    p.set_message("Downloading pre-indexed DB...");
                } else if verbose {
                    println!("Downloading pre-indexed DB from {}...", db_url);
                }
                if let Some(parent) = db_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                let temp_db_path = db_path.with_extension("db.tmp");
                if install_util::download_file_with_progress(
                    &db_url,
                    &temp_db_path,
                    pb.as_ref(),
                    None,
                )
                .is_ok()
                {
                    if fs::rename(&temp_db_path, &db_path).is_ok() {
                        db_downloaded = true;
                        if verbose {
                            println!("Successfully downloaded pre-indexed DB.");
                        }
                    }
                } else {
                    let _ = fs::remove_file(&temp_db_path);
                }
            }
        }

        if !db_downloaded {
            refresh_registry_db(&reg.handle, &target_dir, m, verbose, pb.as_ref())?;
        }

        if let Ok(repo_config) = config::read_repo_config(&target_dir)
            && repo_config.advisory_prefix != reg.advisory_prefix
        {
            reg.advisory_prefix = repo_config.advisory_prefix;
            reg_changed = true;
        }

        if let Some(p) = pb {
            p.finish_with_message(format!("Synced {}", reg.handle.cyan()));
        }
    }

    Ok((reg, reg_changed))
}

/// Performs a project-local sync of registries.
///
/// In Specification v2, projects can have their own isolated package databases
/// stored in `./.zoi/pkgs/db`. This ensures that a project's dependencies
/// are reproducible and independent of the user's global registry state.
pub fn run_local(verbose: bool, _fallback: bool, force: bool, frozen: bool) -> Result<()> {
    let local_db_root = std::env::current_dir()?
        .join(".zoi")
        .join("pkgs")
        .join("db");
    fs::create_dir_all(&local_db_root)?;

    let registries: Vec<(String, String, String)> = if frozen {
        let lockfile = zoi_project::lockfile::read_zoi_lock()?;
        lockfile
            .registries
            .into_iter()
            .map(|(handle, lr)| (handle, lr.url, lr.revision))
            .collect()
    } else {
        let project = zoi_project::config::load_with_env(HashMap::new())?;

        project
            .registries
            .into_iter()
            .map(|(handle, spec)| {
                let rev = spec.revision.clone().unwrap_or_else(|| "main".to_string());
                (handle, spec.url, rev)
            })
            .collect()
    };

    if registries.is_empty() {
        println!("{} No registries found in zoi.lua.", "::".bold().yellow());
        return Ok(());
    }

    let m = if verbose {
        None
    } else {
        Some(MultiProgress::new())
    };

    let results: Vec<((String, String), String)> = registries
        .into_par_iter()
        .map(|(handle, url, revision)| {
            let target_dir = local_db_root.join(&handle);

            if force && target_dir.exists() {
                fs::remove_dir_all(&target_dir)?;
            }

            try_sync_at_path(&url, &target_dir, verbose, m.as_ref(), None)?;

            if !revision.is_empty() {
                if verbose {
                    println!(
                        "  Checking out revision '{}' for registry '{}'...",
                        revision, handle
                    );
                }
                let status = Command::new("git")
                    .arg("-C")
                    .arg(&target_dir)
                    .arg("checkout")
                    .arg(&revision)
                    .stdout(if verbose {
                        Stdio::inherit()
                    } else {
                        Stdio::null()
                    })
                    .stderr(if verbose {
                        Stdio::inherit()
                    } else {
                        Stdio::null()
                    })
                    .status()
                    .map_err(|e| anyhow!("Failed to run git checkout: {}", e))?;
                if !status.success() {
                    return Err(anyhow!(
                        "Failed to checkout revision '{}' for registry '{}'",
                        revision,
                        handle
                    ));
                }
            }

            refresh_registry_db(&handle, &target_dir, m.as_ref(), verbose, None)?;

            let resolved_hash = if frozen {
                revision.clone()
            } else if let Ok(repo) = git2::Repository::open(&target_dir) {
                repo.head()
                    .ok()
                    .and_then(|h| h.target().map(|oid| oid.to_string()))
                    .unwrap_or(revision.clone())
            } else {
                revision.clone()
            };

            Ok(((handle, url), resolved_hash))
        })
        .collect::<Result<Vec<_>>>()?;

    if !frozen {
        let mut lockfile = zoi_project::lockfile::read_zoi_lock()?;
        for ((handle, url), revision) in results {
            lockfile
                .registries
                .insert(handle, types::LockRegistryV2 { revision, url });
        }
        lockfile.version = "2".to_string();
        zoi_project::lockfile::write_zoi_lock(&mut lockfile)?;
    }

    println!("{} Local sync complete.", "::".bold().blue());
    Ok(())
}

/// The primary entry point for synchronizing Zoi registries and system state.
///
/// This function:
/// - Synchronizes all configured global registries.
/// - Updates local SQLite indexes.
/// - Detects and records available native package managers.
/// - Synchronizes the remote security policy if configured.
pub fn run(verbose: bool, fallback: bool, no_pm: bool, force: bool) -> Result<()> {
    let merged_config = config::read_config()?;
    if force {
        println!(
            "{} Force sync: removing existing databases and re-syncing from scratch...",
            "::".bold().yellow()
        );
    }
    if merged_config.protect_db || force {
        let db_root = get_db_path()?;
        if db_root.exists() {
            if verbose || force {
                println!("Making package database writable...");
            }
            if let Err(e) = core_utils::set_path_writable(&db_root) {
                eprintln!("Warning: could not make db writable: {}", e);
            }
        }
    }

    let mut config = config::read_user_config()?;
    let mut needs_config_update = false;

    if config.default_registry.is_none() {
        let merged_config = config::read_config()?;
        if merged_config.default_registry.is_some() {
            config.default_registry = merged_config.default_registry;
        }
    }

    let db_root = get_db_path()?;
    let mut registries_to_sync = Vec::new();

    if let Some(default_reg) = &config.default_registry {
        registries_to_sync.push((default_reg.clone(), true));
    }

    for reg in &config.added_registries {
        registries_to_sync.push((reg.clone(), false));
    }

    if force {
        for (reg, _) in &registries_to_sync {
            let db_file = db_root.join(format!("{}.db", reg.handle));
            if db_file.exists() {
                if verbose {
                    println!("Removing database: {}", db_file.display());
                }
                std::fs::remove_file(&db_file)?;
            }
            let clone_dir = db_root.join(&reg.handle);
            if clone_dir.exists() {
                if verbose {
                    println!("Removing clone directory: {}", clone_dir.display());
                }
                std::fs::remove_dir_all(&clone_dir)?;
            }
        }
    }

    if !registries_to_sync.is_empty() {
        println!("{} Syncing registries...", "::".bold().blue());
        let m = if verbose {
            None
        } else {
            Some(MultiProgress::new())
        };

        let results: Vec<Result<(types::Registry, bool, bool)>> = registries_to_sync
            .into_par_iter()
            .map(|(reg, is_default)| {
                let (synced_reg, changed) =
                    sync_registry(reg, &db_root, verbose, fallback, m.as_ref())?;
                Ok((synced_reg, changed, is_default))
            })
            .collect();

        let mut updated_added_registries = Vec::new();
        for res in results {
            let (reg, changed, is_default) = res?;
            if changed {
                needs_config_update = true;
            }
            if is_default {
                config.default_registry = Some(reg);
            } else {
                updated_added_registries.push(reg);
            }
        }
        config.added_registries = updated_added_registries;
    }

    if !no_pm {
        if verbose {
            println!("\n{}", "Updating system configuration...".green());
        }
        config.native_package_manager = core_utils::get_native_package_manager();
        config.package_managers = Some(core_utils::get_all_available_package_managers());
        needs_config_update = true;
        if verbose {
            println!("System configuration updated.");
        }
    }

    if needs_config_update {
        config::write_user_config(&config)?;
    }

    let _ = config::sync_remote_policy();

    sync_git_repos(verbose)?;

    if merged_config.protect_db {
        let db_root = get_db_path()?;
        if db_root.exists() {
            if verbose {
                println!("Making package database read-only...");
            }
            if let Err(e) = core_utils::set_path_read_only(&db_root) {
                eprintln!("Warning: could not make db read-only: {}", e);
            }
        }
    }

    Ok(())
}
