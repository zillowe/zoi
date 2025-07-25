use colored::*;
use git2::{build::CheckoutBuilder, build::RepoBuilder, FetchOptions, RemoteCallbacks, Repository};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DB_URL: &str = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git";

fn get_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn run_verbose() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
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
            .arg(DB_URL)
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
    if verbose {
        return run_verbose();
    }

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
            println!("{}", "Cannot fast-forward. Please run `git pull` manually.".yellow());
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
            .clone(DB_URL, &db_path)?;

        fetch_pb.finish_with_message("Fetched.");
        checkout_pb.finish_with_message("Checked out.");
    }
    
    m.clear().ok();
    Ok(())
}
