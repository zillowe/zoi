use colored::*;

/// The primary entry point for the Zoi CLI and its intelligent shims.
///
/// Multiplexing Logic:
/// Zoi uses a single binary for both the main CLI and all tool shims.
/// - If the binary is invoked as `zoi`, it runs the standard CLI.
/// - If it is invoked via a symlink (e.g. `node` -> `zoi`), it acts as a "Shim".
/// - In shim mode, it dynamically resolves the correct version of the requested
///    tool based on the current project context, environment, or plugins.
fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    let args: Vec<String> = std::env::args().collect();
    let program_name = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    if !program_name.is_empty()
        && program_name != "zoi"
        && !program_name.starts_with("zoi-")
        && !program_name.contains("target")
    {
        let plugin_manager = match zoi_cli::pkg::plugin::PluginManager::new() {
            Ok(m) => {
                let _ = m.load_all(false);
                m
            }
            Err(e) => {
                eprintln!(
                    "{}: Failed to initialize PluginManager: {}",
                    "Error".red().bold(),
                    e
                );
                std::process::exit(1);
            }
        };

        /// A JIT (Just-in-Time) installation callback used by shims.
        ///
        /// This is a unique Zoi feature: if a shim resolves to a version that is NOT
        /// currently installed (e.g. via a project's .tool-versions file), Zoi
        /// doesn't just fail. Instead, it triggers this callback to:
        /// - Prompt the user for JIT installation.
        /// - Resolve the required package and its dependency tree.
        /// - Install it into the appropriate store scope.
        /// - Seamlessly continue the original execution.
        let auto_install = |name: &str, version: &str| -> anyhow::Result<()> {
            let spec = format!("{}@{}", name, version);
            let scope = if std::path::Path::new("zoi.yaml").exists() {
                zoi_cli::pkg::types::Scope::Project
            } else {
                zoi_cli::pkg::types::Scope::User
            };
            let _options = zoi_cli::SourceInstallOptions {
                scope_override: Some(scope),
                yes: true,
                ..Default::default()
            };
            zoi_cli::cmd::install::run(
                &[spec],
                None,
                false,
                false,
                true,
                Some(zoi_cli::cli::InstallScope::User),
                false,
                false,
                false,
                None,
                false,
                Some(&plugin_manager),
                false,
                false,
                false,
                false,
                3,
                false,
                false,
            )
        };

        if let Err(e) = zoi_cli::pkg::shim::run_shim(
            program_name,
            args[1..].to_vec(),
            Some(&plugin_manager),
            Some(&auto_install),
        ) {
            eprintln!("{}: {}", "Shim Error".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = zoi_cli::cli::run() {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
