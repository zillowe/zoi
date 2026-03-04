use crate::{
    pkg::{config, db, pgp, types},
    utils,
};
use anyhow::{Result, anyhow};
use colored::*;
use git2::{
    FetchOptions, RemoteCallbacks, Repository,
    build::{CheckoutBuilder, RepoBuilder},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_yaml;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::Builder;
use walkdir::WalkDir;

fn refresh_registry_db(
    registry_handle: &str,
    registry_path: &Path,
    sync_files: bool,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let msg = format!(
        "Refreshing metadata database for {}...",
        registry_handle.cyan()
    );
    if let Some(m_ref) = m {
        let _ = m_ref.println(&msg);
    } else {
        println!("{}", msg);
    }

    let _conn = db::open_connection(registry_handle)?;
    db::clear_registry(&_conn)?;

    let mut pkg_files = Vec::new();
    for entry in WalkDir::new(registry_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".pkg.lua")
        {
            pkg_files.push(entry.path().to_path_buf());
        }
    }

    let repo_config = config::read_repo_config(registry_path).ok();
    let platform = utils::get_platform().unwrap_or_default();

    pkg_files.par_iter().for_each(|path| {
        if let Ok(pkg) =
            crate::pkg::lua::parser::parse_lua_package(path.to_str().unwrap(), None, true)
        {
            let mut pkg = pkg;
            if let Ok(rel_path) = path.strip_prefix(registry_path)
                && let Some(parent) = rel_path.parent()
            {
                pkg.repo = parent.to_string_lossy().to_string().replace('\\', "/");
            }

            if let Ok(conn) = db::open_connection_no_setup(registry_handle) {
                let pkg_id =
                    match db::update_package(&conn, &pkg, registry_handle, None, None, None) {
                        Ok(id) => id,
                        Err(_) => return,
                    };

                if sync_files
                    && let Some(rc) = &repo_config
                    && let Some(pkg_link) = rc.pkg.iter().find(|p| p.link_type == "main")
                    && let Some(files_url_template) = &pkg_link.files
                {
                    let version = pkg.version.clone().unwrap_or_else(|| "latest".to_string());
                    let files_url = crate::pkg::install::util::resolve_url_placeholders(
                        files_url_template,
                        &pkg.name,
                        &pkg.repo,
                        &version,
                        &platform,
                    );

                    if let Ok(response) = reqwest::blocking::get(&files_url)
                        && response.status().is_success()
                        && let Ok(content) = response.text()
                    {
                        let file_list: Vec<String> = content
                            .lines()
                            .map(|l| l.trim().to_string())
                            .filter(|l| !l.is_empty())
                            .collect();
                        let _ = db::index_package_files(&conn, pkg_id, &file_list);
                    }
                }
            }
        }
    });

    Ok(())
}

fn verify_registry_signature(repo_path: &Path, authorities: &[String]) -> Result<()> {
    if authorities.is_empty() {
        return Ok(());
    }

    println!("Verifying registry signature...");

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
        println!("{}", "Registry signature verified successfully.".green());
        Ok(())
    } else {
        Err(anyhow!(
            "Registry commit was signed but not by any authorized authority. Sync aborted."
        ))
    }
}

fn get_db_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(crate::pkg::sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("db"),
    ))
}

fn get_git_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(crate::pkg::sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("git"),
    ))
}

fn sync_git_repos(verbose: bool) -> Result<()> {
    if crate::pkg::offline::is_offline() {
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

    println!("\n{}", "Syncing external git repositories...".green());

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
            let repo_name = path.file_name().unwrap().to_string_lossy();

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
            .arg(db_path.to_str().unwrap())
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

fn run_non_verbose_at_path(db_url: &str, db_path: &Path, m: Option<&MultiProgress>) -> Result<()> {
    let internal_m;
    let m = if let Some(m_ref) = m {
        m_ref
    } else {
        internal_m = MultiProgress::new();
        &internal_m
    };

    let fetch_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] Fetching: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
            )?
            .progress_chars("#>-"),
    ));
    let checkout_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] Checkout: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
            )?
            .progress_chars("#>-"),
    ));

    if db_path.exists() {
        let repo = Repository::open(db_path)?;
        let mut remote = repo.find_remote("origin")?;

        let mut cb = RemoteCallbacks::new();
        let fetch_pb_clone = fetch_pb.clone();
        cb.transfer_progress(move |stats| {
            if stats.total_deltas() > 0 {
                fetch_pb_clone.set_length(stats.total_deltas() as u64);
                fetch_pb_clone.set_position(stats.indexed_deltas() as u64);
            }
            true
        });

        let head_symref = repo.find_reference("refs/remotes/origin/HEAD")?;
        let remote_default_ref = head_symref
            .symbolic_target()
            .ok_or_else(|| anyhow!("Remote HEAD is not a symbolic ref"))?;
        let short_branch_name = remote_default_ref
            .strip_prefix("refs/remotes/origin/")
            .ok_or_else(|| anyhow!("Could not determine default branch name from remote HEAD"))?;

        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);
        remote.fetch(&[short_branch_name], Some(&mut fo), None)?;
        fetch_pb.finish_with_message("Fetched.");

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_up_to_date() {
            checkout_pb.finish_with_message("Already up to date.");
        } else if analysis.0.is_fast_forward() {
            let refname = format!("refs/heads/{}", short_branch_name);
            let mut reference = repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forwarding")?;
            repo.set_head(&refname)?;

            let mut checkout_builder = CheckoutBuilder::new();
            let checkout_pb_clone = checkout_pb.clone();
            checkout_builder.force().progress(move |_path, cur, total| {
                if total > 0 {
                    checkout_pb_clone.set_length(total as u64);
                    checkout_pb_clone.set_position(cur as u64);
                }
            });

            repo.checkout_head(Some(&mut checkout_builder))?;
            checkout_pb.finish_with_message("Checked out.");
        } else {
            checkout_pb.finish_with_message("Cannot fast-forward.");
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
        let fetch_pb_clone = fetch_pb.clone();
        cb.transfer_progress(move |stats| {
            if stats.total_deltas() > 0 {
                fetch_pb_clone.set_length(stats.total_deltas() as u64);
            }
            fetch_pb_clone.set_position(stats.indexed_deltas() as u64);
            true
        });

        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);

        let mut checkout_builder = CheckoutBuilder::new();
        let checkout_pb_clone = checkout_pb.clone();
        checkout_builder.progress(move |_path, cur, total| {
            if total > 0 {
                checkout_pb_clone.set_length(total as u64);
            }
            checkout_pb_clone.set_position(cur as u64);
        });

        RepoBuilder::new()
            .fetch_options(fo)
            .with_checkout(checkout_builder)
            .clone(db_url, db_path)?;

        fetch_pb.finish_with_message("Fetched.");
        checkout_pb.finish_with_message("Checked out.");
    }

    m.clear().ok();
    Ok(())
}

fn try_sync_at_path(
    db_url: &str,
    db_path: &Path,
    verbose: bool,
    m: Option<&MultiProgress>,
) -> Result<()> {
    if crate::pkg::offline::is_offline() {
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
        && let Some(remote_url) = remote.url()
        && remote_url != db_url
    {
        let msg = format!(
            "Registry URL has changed from {}. Removing old database and re-cloning from {}.",
            remote_url.yellow(),
            db_url.cyan()
        );
        if let Some(m_ref) = m {
            m_ref.println(&msg)?;
        } else {
            println!("{}", msg);
        }
        fs::remove_dir_all(db_path)?;
    }

    if verbose {
        run_verbose_at_path(db_url, db_path)
    } else {
        run_non_verbose_at_path(db_url, db_path, m)
    }
}

fn sync_pgp_keys_at_path(db_path: &Path) -> Result<()> {
    println!("\n{}", "Syncing PGP keys from repository...".green());
    if !db_path.join("repo.yaml").exists() {
        println!("{}", "repo.yaml not found, skipping PGP key sync.".yellow());
        return Ok(());
    }

    let repo_config = config::read_repo_config(db_path)?;

    if repo_config.pgp.is_empty() {
        println!("No PGP keys defined in repo.yaml.");
        return Ok(());
    }

    for key_info in repo_config.pgp {
        let key_source = &key_info.key;
        let key_name = &key_info.name;

        let result = if key_source.starts_with("http") {
            crate::pkg::pgp::add_key_from_url(key_source, key_name)
        } else if key_source.len() == 40 && key_source.chars().all(|c| c.is_ascii_hexdigit()) {
            crate::pkg::pgp::add_key_from_fingerprint(key_source, key_name)
        } else {
            Err(anyhow!(
                "Invalid key source '{}': must be a URL or a 40-character fingerprint.",
                key_source
            ))
        };

        if let Err(e) = result {
            eprintln!(
                "{} Failed to import key '{}': {}",
                "Warning:".yellow(),
                key_name,
                e
            );
        }
    }

    Ok(())
}

fn fetch_handle_by_cloning(url: &str) -> Result<String> {
    let temp_dir = Builder::new().prefix("zoi-handle-fetch").tempdir()?;
    println!("Cloning '{}' to fetch handle...", url.cyan());
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(url)
        .arg(temp_dir.path())
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

        if let Ok(response) = reqwest::blocking::get(&repo_yaml_url)
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

fn fetch_handle_for_url(url: &str) -> Result<String> {
    println!(
        "Attempting to fetch handle for '{}' directly...",
        url.cyan()
    );
    match fetch_repo_yaml_content(url) {
        Ok(content) => {
            let repo_config: crate::pkg::types::RepoConfig = serde_yaml::from_str(&content)?;
            println!("Successfully fetched and parsed repo.yaml.");
            Ok(repo_config.name)
        }
        Err(e) => {
            println!(
                "Direct fetch failed: {}. Falling back to cloning repository...",
                e.to_string().yellow()
            );
            fetch_handle_by_cloning(url)
        }
    }
}

fn sync_registry(
    mut reg: types::Registry,
    db_root: &Path,
    verbose: bool,
    sync_files: bool,
    m: Option<&MultiProgress>,
) -> Result<(types::Registry, bool)> {
    let mut reg_changed = false;
    if reg.handle.is_empty() {
        let handle = fetch_handle_for_url(&reg.url)?;
        reg.handle = handle;
        reg_changed = true;
    }

    let target_dir = db_root.join(&reg.handle);
    if let Err(e) = try_sync_at_path(&reg.url, &target_dir, verbose, m) {
        let msg = format!("Sync with {} failed: {}", reg.url.yellow(), e);
        if let Some(m_ref) = m {
            m_ref.println(msg)?;
        } else {
            eprintln!("{}", msg);
        }
    } else {
        if let Some(authorities) = &reg.authorities
            && let Err(e) = verify_registry_signature(&target_dir, authorities)
        {
            let msg = format!(
                "Security: Registry signature check failed for {}: {}",
                reg.url.red(),
                e
            );
            if let Some(m_ref) = m {
                m_ref.println(&msg)?;
            } else {
                eprintln!("{}", msg);
            }
            return Err(e);
        }

        let msg = format!("{} with {}", "Sync successful".green(), reg.url.cyan());
        if let Some(m_ref) = m {
            m_ref.println(msg)?;
        } else {
            println!("{}", msg);
        }
        sync_pgp_keys_at_path(&target_dir)?;
        refresh_registry_db(&reg.handle, &target_dir, sync_files, m)?;
    }

    Ok((reg, reg_changed))
}

pub fn run(verbose: bool, _fallback: bool, no_pm: bool, sync_files: bool) -> Result<()> {
    let merged_config = config::read_config()?;
    if merged_config.protect_db {
        let db_root = get_db_path()?;
        if db_root.exists() {
            println!("Making package database writable...");
            if let Err(e) = utils::set_path_writable(&db_root) {
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

    if !registries_to_sync.is_empty() {
        println!("Syncing registries...");
        let m = if verbose {
            None
        } else {
            Some(MultiProgress::new())
        };

        let results: Vec<Result<(types::Registry, bool, bool)>> = registries_to_sync
            .into_par_iter()
            .map(|(reg, is_default)| {
                let (synced_reg, changed) =
                    sync_registry(reg, &db_root, verbose, sync_files, m.as_ref())?;
                Ok((synced_reg, changed, is_default))
            })
            .collect();

        if let Some(m_ref) = m {
            m_ref.clear().ok();
        }

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
        println!("\n{}", "Updating system configuration...".green());
        config.native_package_manager = utils::get_native_package_manager();
        config.package_managers = Some(utils::get_all_available_package_managers());
        needs_config_update = true;
        println!("System configuration updated.");
    }

    if needs_config_update {
        config::write_user_config(&config)?;
    }

    sync_git_repos(verbose)?;

    if merged_config.protect_db {
        let db_root = get_db_path()?;
        if db_root.exists() {
            println!("Making package database read-only...");
            if let Err(e) = utils::set_path_read_only(&db_root) {
                eprintln!("Warning: could not make db read-only: {}", e);
            }
        }
    }

    Ok(())
}
