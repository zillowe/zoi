// Copyright (c) 2026 Zillowe Foundation
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0
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
        && program_name != "zoi-rs"
        && !program_name.starts_with("zoi-")
        && !program_name.contains("target")
    {
        let plugin_manager = match zoi::pkg::plugin::PluginManager::new() {
            Ok(m) => {
                let _ = m.load_all();
                m
            }
            Err(_) => {
                zoi::pkg::plugin::PluginManager::new().expect("Failed to initialize PluginManager")
            }
        };

        if let Err(e) = zoi::pkg::shim::run_shim(program_name, args[1..].to_vec(), &plugin_manager)
        {
            eprintln!("{}: {}", "Shim Error".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = zoi::cli::run() {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
