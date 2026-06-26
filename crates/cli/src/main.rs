use colored::*;

fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    let args: Vec<String> = std::env::args().collect();
    let program_name = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

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

        let auto_install = |name: &str, version: &str| -> anyhow::Result<()> {
            let spec = format!("{}@{}", name, version);
            let scope = if std::path::Path::new("zoi.yaml").exists() {
                zoi_cli::pkg::types::Scope::Project
            } else {
                zoi_cli::pkg::types::Scope::User
            };
            let options = zoi_cli::SourceInstallOptions {
                scope_override: Some(scope),
                yes: true,
                ..Default::default()
            };
            zoi_cli::install_sources(&[spec], &options)
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
