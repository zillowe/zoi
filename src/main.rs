fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).ok();

    zoi::cli::run();
}
