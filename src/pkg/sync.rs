use colored::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const DB_URL: &str = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git";

fn get_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    println!("Database path: {}", db_path.display());

    if db_path.exists() {
        println!("Database found. Checking remote URL and pulling changes...");

        let git_dir = db_path.join(".git");
        if !git_dir.exists() {
            return Err(format!(
                "Directory exists at '{}' but it is not a git repository.",
                db_path.display()
            )
            .into());
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(db_path.to_str().unwrap())
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .output()?;

        let remote_url = String::from_utf8(output.stdout)?.trim().to_string();

        if remote_url != DB_URL {
            return Err(format!(
                "Remote URL mismatch! Expected '{}', but found '{}'.",
                DB_URL, remote_url
            )
            .into());
        }

        let pull_status = Command::new("git")
            .arg("-C")
            .arg(db_path.to_str().unwrap())
            .arg("pull")
            .status()?;

        if !pull_status.success() {
            return Err("Failed to pull changes from the remote repository.".into());
        }

        println!("{}", "Successfully pulled the latest changes.".green());
    } else {
        println!("No local database found. Cloning from remote...");

        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let clone_status = Command::new("git")
            .arg("clone")
            .arg(DB_URL)
            .arg(&db_path)
            .status()?;

        if !clone_status.success() {
            return Err("Failed to clone the package repository.".into());
        }
        println!("{}", "Successfully cloned the package database.".green());
    }

    Ok(())
}
