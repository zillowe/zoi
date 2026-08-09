use anyhow::Result;
use colored::Colorize;
use diffy::merge;
use std::fs;
use std::path::Path;

/// Performs a 3-way merge on configuration files defined in the `backup` manifest field.
///
/// It compares:
/// - Base: The original package default from the previous version (`.zoiorig`).
/// - Yours: The user's modified config in the previous version's directory.
/// - Theirs: The new package default in the incoming version's directory.
///
/// Merging logic:
/// - If Yours == Base: User didn't change it. Use Theirs (do nothing, already in place).
/// - If Theirs == Base: Upstream didn't change it. Use Yours (copy Yours over Theirs).
/// - If both changed: Perform 3-way merge.
///   - Clean merge: Write result to Theirs.
///   - Conflict: Write result with markers to Theirs, save Theirs as `.zoinew`.
///
/// # Errors
///
/// Returns an error if:
/// - Placeholders in the backup file path cannot be expanded.
/// - The configuration file directory cannot be created.
pub fn handle_backup_files(
    old_version_dir: &Path,
    new_version_dir: &Path,
    backup_files: &[String],
    scope: crate::pkg::types::Scope,
) -> Result<()> {
    for backup_file_rel in backup_files {
        let old_expanded =
            crate::pkg::utils::expand_placeholders(backup_file_rel, old_version_dir, scope)?;
        let new_expanded =
            crate::pkg::utils::expand_placeholders(backup_file_rel, new_version_dir, scope)?;

        let old_path = std::path::PathBuf::from(old_expanded);
        let new_path = std::path::PathBuf::from(new_expanded);

        // Zoi creates .zoiorig in pkg_install.rs
        let mut old_orig_path = old_path.clone();
        let ext = old_orig_path
            .extension()
            .and_then(|s| s.to_str())
            .map_or_else(|| "zoiorig".to_string(), |s| format!("{s}.zoiorig"));
        old_orig_path.set_extension(ext);

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
                        eprintln!("Warning: failed to restore user config: {e}");
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
                            eprintln!("Warning: failed to write merged config: {e}");
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
                            eprintln!("Warning: failed to write conflicted config: {e}");
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
                    eprintln!("Warning: failed to rename to .zoinew: {e}");
                    continue;
                }
            }
            if let Some(p) = new_path.parent() {
                fs::create_dir_all(p)?;
            }
            if let Err(e) = fs::rename(&old_path, &new_path) {
                eprintln!("Warning: failed to restore backup file: {e}");
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
        let dir = tempdir().expect("Failed to create temp dir");
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).expect("Failed to create old dir");
        fs::create_dir_all(&new_dir).expect("Failed to create new dir");

        let base = "line1\nline2\n";

        fs::write(old_dir.join("config.txt.zoiorig"), base).expect("Failed to write base");
        fs::write(old_dir.join("config.txt"), base).expect("Failed to write yours");
        fs::write(new_dir.join("config.txt"), "line1\nline2\nline3\n")
            .expect("Failed to write theirs");

        handle_backup_files(
            &old_dir,
            &new_dir,
            &["config.txt".to_string()],
            crate::pkg::types::Scope::User,
        )
        .expect("Failed to handle backup files");

        // Should keep new version
        assert_eq!(
            fs::read_to_string(new_dir.join("config.txt")).expect("Failed to read result"),
            "line1\nline2\nline3\n"
        );
    }

    #[test]
    fn test_merge_case_b_upstream_unchanged() {
        let dir = tempdir().expect("Failed to create temp dir");
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).expect("Failed to create old dir");
        fs::create_dir_all(&new_dir).expect("Failed to create new dir");

        let base = "line1\nline2\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).expect("Failed to write base");
        fs::write(old_dir.join("config.txt"), "line1\nline2\nuser_mod\n")
            .expect("Failed to write yours");
        fs::write(new_dir.join("config.txt"), base).expect("Failed to write theirs");

        handle_backup_files(
            &old_dir,
            &new_dir,
            &["config.txt".to_string()],
            crate::pkg::types::Scope::User,
        )
        .expect("Failed to handle backup files");

        // Should keep user version
        assert_eq!(
            fs::read_to_string(new_dir.join("config.txt")).expect("Failed to read result"),
            "line1\nline2\nuser_mod\n"
        );
    }

    #[test]
    fn test_merge_case_c_clean_merge() {
        let dir = tempdir().expect("Failed to create temp dir");
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).expect("Failed to create old dir");
        fs::create_dir_all(&new_dir).expect("Failed to create new dir");

        let base = "common\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).expect("Failed to write base");
        fs::write(old_dir.join("config.txt"), "user_pref\ncommon\n")
            .expect("Failed to write yours");
        fs::write(new_dir.join("config.txt"), "common\nupstream_add\n")
            .expect("Failed to write theirs");

        handle_backup_files(
            &old_dir,
            &new_dir,
            &["config.txt".to_string()],
            crate::pkg::types::Scope::User,
        )
        .expect("Failed to handle backup files");

        let result = fs::read_to_string(new_dir.join("config.txt")).expect("Failed to read result");
        assert!(result.contains("user_pref"));
        assert!(result.contains("upstream_add"));
        assert!(result.contains("common"));
    }

    #[test]
    fn test_merge_case_c_conflict() {
        let dir = tempdir().expect("Failed to create temp dir");
        let old_dir = dir.path().join("1.0.0");
        let new_dir = dir.path().join("1.1.0");
        fs::create_dir_all(&old_dir).expect("Failed to create old dir");
        fs::create_dir_all(&new_dir).expect("Failed to create new dir");

        let base = "line\n";
        fs::write(old_dir.join("config.txt.zoiorig"), base).expect("Failed to write base");
        fs::write(old_dir.join("config.txt"), "user\n").expect("Failed to write yours");
        fs::write(new_dir.join("config.txt"), "upstream\n").expect("Failed to write theirs");

        handle_backup_files(
            &old_dir,
            &new_dir,
            &["config.txt".to_string()],
            crate::pkg::types::Scope::User,
        )
        .expect("Failed to handle backup files");

        let result = fs::read_to_string(new_dir.join("config.txt")).expect("Failed to read result");
        assert!(result.contains("<<<<<<<"));
        assert!(result.contains("user"));
        assert!(result.contains("upstream"));

        assert!(new_dir.join("config.txt.zoinew").exists());
    }
}
