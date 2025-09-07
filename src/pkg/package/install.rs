use super::structs::FinalMetadata;
use colored::*;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::read::Decoder as ZstdDecoder;

fn get_store_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi/pkgs/store"))
}

fn get_bin_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi/pkgs/bin"))
}

pub fn run(package_file: &Path) -> Result<(), Box<dyn Error>> {
    println!(
        "Installing from package archive: {}",
        package_file.display()
    );

    let file = File::open(package_file)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);

    let temp_dir = Builder::new().prefix("zoi-install-").tempdir()?;
    archive.unpack(temp_dir.path())?;

    let metadata_path = temp_dir.path().join("metadata.json");
    let metadata_content = fs::read_to_string(metadata_path)?;
    let metadata: FinalMetadata = serde_json::from_str(&metadata_content)?;

    println!(
        "Installing package: {} v{}",
        metadata.name.cyan(),
        metadata.version.yellow()
    );

    let store_dir = get_store_root()?.join(&metadata.name);
    if store_dir.exists() {
        println!("Removing existing installation...");
        fs::remove_dir_all(&store_dir)?;
    }
    fs::create_dir_all(&store_dir)?;

    let data_dir = temp_dir.path().join("data");
    if data_dir.exists() {
        println!("Copying package files...");
        for entry in WalkDir::new(&data_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .skip(1)
        {
            let dest_path = store_dir.join(entry.path().strip_prefix(&data_dir)?);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                fs::copy(entry.path(), &dest_path)?;
            }
        }
    }

    let man_md_path = temp_dir.path().join("man.md");
    if man_md_path.exists() {
        fs::copy(man_md_path, store_dir.join("man.md"))?;
        println!("Installed manual (man.md).");
    }

    let man_txt_path = temp_dir.path().join("man.txt");
    if man_txt_path.exists() {
        fs::copy(man_txt_path, store_dir.join("man.txt"))?;
        println!("Installed manual (man.txt).");
    }

    if let Some(bins) = &metadata.bins {
        let bin_root = get_bin_root()?;
        fs::create_dir_all(&bin_root)?;

        for bin_name in bins {
            let mut found_bin = false;
            for entry in WalkDir::new(&store_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() && entry.file_name().to_string_lossy() == *bin_name {
                    let target_path = entry.path();
                    let link_path = bin_root.join(bin_name);

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs as unix_fs;
                        if link_path.exists() {
                            fs::remove_file(&link_path)?;
                        }
                        unix_fs::symlink(target_path, &link_path)?;
                    }
                    #[cfg(windows)]
                    {
                        fs::copy(target_path, &link_path)?;
                    }

                    println!("Linked binary: {}", bin_name.green());
                    found_bin = true;
                    break;
                }
            }
            if !found_bin {
                eprintln!(
                    "Warning: could not find binary '{}' to link.",
                    bin_name.yellow()
                );
            }
        }
    }

    println!("{}", "Installation complete.".green());
    Ok(())
}
