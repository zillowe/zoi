use sha2::{Digest, Sha512};

/// Generates a unique ID for a package based on its origin.
/// The format for the hash is `#{registry-handle}@{repo/path/to/package}`.
pub fn generate_package_id(registry_handle: &str, repo_path: &str) -> String {
    let format_string = format!("#{}@{}", registry_handle, repo_path);
    let mut hasher = Sha512::new();
    hasher.update(format_string.as_bytes());
    let result = hasher.finalize();
    let hex_string = hex::encode(result);
    hex_string[..32].to_string()
}

/// Creates the directory name for the package in the store.
/// Format: `{hash}-{name}`
pub fn get_package_dir_name(package_id: &str, package_name: &str) -> String {
    format!("{}-{}", package_id, package_name)
}
