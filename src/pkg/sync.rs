use crate::{pkg::config, utils};
use anyhow::{Result, anyhow};
use colored::*;
use git2::{
    FetchOptions, RemoteCallbacks, Repository,
    build::{CheckoutBuilder, RepoBuilder},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::Builder;

fn get_db_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn get_git_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("git"))
}

fn sync_git_repos(verbose: bool) -> Result<()> {
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
                .unwrap_or("")
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

fn run_non_verbose_at_path(db_url: &str, db_path: &Path) -> Result<()> {
    let m = MultiProgress::new();
    let fetch_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] Fetching: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)?",
            )?
            .progress_chars("#>-"),
    ));
    let checkout_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] Checkout: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)?",
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

        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);
        remote.fetch(&["main"], Some(&mut fo), None)?;
        fetch_pb.finish_with_message("Fetched.");

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_up_to_date() {
            checkout_pb.finish_with_message("Already up to date.");
        } else if analysis.0.is_fast_forward() {
            let refname = "refs/heads/main";
            let mut reference = repo.find_reference(refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forwarding")?;
            repo.set_head(refname)?;

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

fn try_sync_at_path(db_url: &str, db_path: &Path, verbose: bool) -> Result<()> {
    if db_path.exists()
        && let Ok(repo) = Repository::open(db_path)
        && let Ok(remote) = repo.find_remote("origin")
        && let Some(remote_url) = remote.url()
        && remote_url != db_url
    {
        println!(
            "Registry URL has changed from {}. Removing old database and re-cloning from {}.",
            remote_url.yellow(),
            db_url.cyan()
        );
        fs::remove_dir_all(db_path)?;
    }

    if verbose {
        run_verbose_at_path(db_url, db_path)
    } else {
        run_non_verbose_at_path(db_url, db_path)
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

fn fetch_handle_for_url(url: &str) -> Result<String> {
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

pub fn run(verbose: bool, _fallback: bool, no_pm: bool) -> Result<()> {
    let mut config = config::read_user_config()?;
    let mut needs_config_update = false;

    if config.default_registry.is_none() {
        let merged_config = config::read_config()?;
        if merged_config.default_registry.is_some() {
            config.default_registry = merged_config.default_registry;
        }
    }

    let db_root = get_db_path()?;

    if let Some(mut default_reg) = config.default_registry.clone() {
        println!("Syncing default registry...");
        let mut reg_changed = false;
        if default_reg.handle.is_empty() {
            let handle = fetch_handle_for_url(&default_reg.url)?;
            default_reg.handle = handle;
            reg_changed = true;
        }

        let target_dir = db_root.join(&default_reg.handle);
        if let Err(e) = try_sync_at_path(&default_reg.url, &target_dir, verbose) {
            eprintln!("Sync with {} failed: {}", default_reg.url.yellow(), e);
        } else {
            println!(
                "{} with {}",
                "Sync successful".green(),
                default_reg.url.cyan()
            );
            sync_pgp_keys_at_path(&target_dir)?;
        }

        if reg_changed {
            config.default_registry = Some(default_reg);
            needs_config_update = true;
        }
    }

    let mut updated_added_registries = Vec::new();
    if !config.added_registries.is_empty() {
        println!("\nSyncing added registries...");
    }
    for mut reg in config.added_registries.clone() {
        let mut reg_changed = false;
        if reg.handle.is_empty() {
            let handle = fetch_handle_for_url(&reg.url)?;
            reg.handle = handle;
            reg_changed = true;
        }

        let target_dir = db_root.join(&reg.handle);
        if let Err(e) = try_sync_at_path(&reg.url, &target_dir, verbose) {
            eprintln!("Sync with {} failed: {}", reg.url.yellow(), e);
        } else {
            println!("{} with {}", "Sync successful".green(), reg.url.cyan());
            sync_pgp_keys_at_path(&target_dir)?;
        }

        if reg_changed {
            needs_config_update = true;
        }
        updated_added_registries.push(reg);
    }
    config.added_registries = updated_added_registries;

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

    Ok(())
}
