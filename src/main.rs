// Copyright (c) 2026 Zillowe Foundation
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0
use colored::*;

fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    if let Err(e) = zoi::cli::run() {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
