use super::build;

pub fn run(args: build::BuildCommand) {
    if let Err(e) = crate::pkg::package::test::run(&args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
