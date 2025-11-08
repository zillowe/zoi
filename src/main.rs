use colored::*;

fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    if let Err(e) = zoi::cli::run() {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
