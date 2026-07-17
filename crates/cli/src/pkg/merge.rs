use anyhow::Result;
use colored::*;
use diffy::merge;
use std::fs;
use std::path::Path;

/// Performs a 3-way merge on configuration files defined in the `backup` manifest field.
///
/// It compares:
/// 1. Base: The original package default from the previous version (`.zoiorig`).
/// 2. Yours: The user's modified config in the previous version's directory.
/// 3. Theirs: The new package default in the incoming version's directory.
///
/// Merging logic:
/// - If Yours == Base: User didn't change it. Use Theirs (do nothing, already in place).
/// - If Theirs == Base: Upstream didn't change it. Use Yours (copy Yours over Theirs).
/// - If both changed: Perform 3-way merge.
///   - Clean merge: Write result to Theirs.
///   - Conflict: Write result with markers to Theirs, save Theirs as `.zoinew`.
pub fn handle_backup_files(
    old_version_dir: &Path,
    new_version_dir: &Path,
    backup_files: &[String],
) -> Result<()> {
    for backup_file_rel in backup_files {
        let old_path = old_version_dir.join(backup_file_rel);
        let new_path = new_version_dir.join(backup_file_rel);

        // Zoi creates .zoiorig in pkg_install.rs
        let old_orig_path = old_path.with_extension(format!(
            "{}.zoiorig",
            old_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
        ));

        if old_path.exists() {
            // Try 3-way merge if we have the original base
            if old_orig_path.exists()
                && new_path.exists()
                && let (Ok(base), Ok(yours), Ok(theirs)) = (
                    fs::read_to_string(&old_orig_path),
                    fs::read_to_string(&old_path),
                    fs::read_to_string(&new_path),
                )
            {
                if yours == base {
                    // Case A: Unmodified by user. Use new upstream default.
                    continue;
                }

                if theirs == base {
                    // Case B: Upstream unchanged. Keep user's changes.
                    if let Err(e) = fs::copy(&old_path, &new_path) {
                        eprintln!("Warning: failed to restore user config: {}", e);
                    }
                    continue;
                }

                // Case C: 3-Way Merge
                println!(
                    "{} Merging changes for '{}'...",
                    "::".bold().blue(),
                    backup_file_rel.cyan()
                );
                match merge(&base, &yours, &theirs) {
                    Ok(merged) => {
                        println!("   {} Automatically merged.", "Success:".green());
                        if let Err(e) = fs::write(&new_path, merged) {
                            eprintln!("Warning: failed to write merged config: {}", e);
                        }
                    }
                    Err(conflicted) => {
                        eprintln!(
                            "   {} Conflict in {}. Standard markers inserted.",
                            "Warning:".yellow().bold(),
                            backup_file_rel.bold()
                        );
                        // Save new default as .zoinew
                        let zoinew_path = new_path.with_extension(format!(
                            "{}.zoinew",
                            new_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                        ));
                        let _ = fs::copy(&new_path, &zoinew_path);

                        if let Err(e) = fs::write(&new_path, conflicted) {
                            eprintln!("Warning: failed to write conflicted config: {}", e);
                        }
                    }
                }
                continue;
            }

            // Legacy Fallback / Binary File handling
            if new_path.exists() {
                let zoinew_path = new_path.with_extension(format!(
                    "{}.zoinew",
                    new_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                ));
                println!(
                    "Configuration file '{}' exists in new version. Saving as .zoinew",
                    new_path.display()
                );
                if let Err(e) = fs::rename(&new_path, &zoinew_path) {
                    eprintln!("Warning: failed to rename to .zoinew: {}", e);
                    continue;
                }
            }
            if let Some(p) = new_path.parent() {
                fs::create_dir_all(p)?;
            }
            if let Err(e) = fs::rename(&old_path, &new_path) {
                eprintln!("Warning: failed to restore backup file: {}", e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_merge_case_a_unmodified() {
        let dir = tempdir().unwrap();
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        let _config_rel = "config.txt";
        let base = "line1\nline2\n";

        fs::write(old_dir.join("config.txt.zoiorig"), base).unwrap();
        fs::write(old_dir.join("config.txt"), base).unwrap();
        fs::write(new_dir.join("config.txt"), "line1\nline2\nline3\n").unwrap();

        handle_backup_files(&old_dir, &new_dir, &["config.txt".to_string()]).unwrap();

        // Should keep new version
        assert_eq!(
            fs::read_to_string(new_dir.join("config.txt")).unwrap(),
            "line1\nline2\nline3\n"
        );
    }

    #[test]
    fn test_merge_case_b_upstream_unchanged() {
        let dir = tempdir().unwrap();
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        let base = "line1\nline2\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).unwrap();
        fs::write(old_dir.join("config.txt"), "line1\nline2\nuser_mod\n").unwrap();
        fs::write(new_dir.join("config.txt"), base).unwrap();

        handle_backup_files(&old_dir, &new_dir, &["config.txt".to_string()]).unwrap();

        // Should keep user version
        assert_eq!(
            fs::read_to_string(new_dir.join("config.txt")).unwrap(),
            "line1\nline2\nuser_mod\n"
        );
    }

    #[test]
    fn test_merge_case_c_clean_merge() {
        let dir = tempdir().unwrap();
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        let base = "common\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).unwrap();
        fs::write(old_dir.join("config.txt"), "user_pref\ncommon\n").unwrap();
        fs::write(new_dir.join("config.txt"), "common\nupstream_add\n").unwrap();

        handle_backup_files(&old_dir, &new_dir, &["config.txt".to_string()]).unwrap();

        let result = fs::read_to_string(new_dir.join("config.txt")).unwrap();
        assert!(result.contains("user_pref"));
        assert!(result.contains("upstream_add"));
        assert!(result.contains("common"));
    }

    #[test]
    fn test_merge_case_c_conflict() {
        let dir = tempdir().unwrap();
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        let base = "line\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).unwrap();
        fs::write(old_dir.join("config.txt"), "user\n").unwrap();
        fs::write(new_dir.join("config.txt"), "upstream\n").unwrap();

        handle_backup_files(&old_dir, &new_dir, &["config.txt".to_string()]).unwrap();

        let result = fs::read_to_string(new_dir.join("config.txt")).unwrap();
        assert!(result.contains("<<<<<<<"));
        assert!(result.contains("user"));
        assert!(result.contains("upstream"));

        assert!(new_dir.join("config.txt.zoinew").exists());
    }
}
