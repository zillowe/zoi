use crate::{pkg::config, utils};
use colored::*;
use git2::{
    FetchOptions, RemoteCallbacks, Repository,
    build::{CheckoutBuilder, RepoBuilder},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn get_db_url() -> Result<String, Box<dyn std::error::Error>> {
    let config = config::read_config()?;
    Ok(config
        .registry
        .unwrap_or_else(|| "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git".to_string()))
}

fn get_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn get_git_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("git"))
}

fn sync_git_repos(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
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

fn run_verbose() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    let db_url = get_db_url()?;

    if db_path.exists() {
        let status = Command::new("git")
            .arg("-C")
            .arg(db_path.to_str().unwrap())
            .arg("pull")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err("Failed to pull changes from the remote repository.".into());
        }
    } else {
        let status = Command::new("git")
            .arg("clone")
            .arg("--progress")
            .arg(&db_url)
            .arg(&db_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err("Failed to clone the package repository.".into());
        }
    }
    Ok(())
}

pub fn run(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    let db_url = get_db_url()?;

    if db_path.exists()
        && let Ok(repo) = Repository::open(&db_path)
        && let Ok(remote) = repo.find_remote("origin")
        && let Some(remote_url) = remote.url()
        && remote_url != db_url
    {
        println!(
            "Registry URL has changed from {}. Removing old database and re-cloning from {}.",
            remote_url.yellow(),
            db_url.cyan()
        );
        fs::remove_dir_all(&db_path)?;
    }

    if verbose {
        run_verbose()?;
    } else {
        let db_path = get_db_path()?;
        println!("Database path: {}", db_path.display());

        let m = MultiProgress::new();
        let fetch_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] Fetching: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")? 
            .progress_chars("#>-"),
        ));
        let checkout_pb = m.add(ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] Checkout: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")? 
            .progress_chars("#>-"),
        ));

        if db_path.exists() {
            println!("Database found. Pulling changes...");
            let repo = Repository::open(&db_path)?;
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
            println!("No local database found. Cloning from remote...");
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
                .clone(&db_url, &db_path)?;

            fetch_pb.finish_with_message("Fetched.");
            checkout_pb.finish_with_message("Checked out.");
        }

        m.clear().ok();
    }

    sync_git_repos(verbose)?;

    println!("\n{}", "Updating system configuration...".green());
    let mut config_data = config::read_config()?;
    config_data.native_package_manager = utils::get_native_package_manager();
    config_data.package_managers = Some(utils::get_all_available_package_managers());
    config::write_config(&config_data)?;
    println!("System configuration updated.");

    Ok(())
}
