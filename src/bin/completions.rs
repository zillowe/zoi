use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;
use clap_complete::generate_to;
use std::env;
use std::io::Result;
use zoi::cli::Cli;

/// Shell completions can be created with:
/// `cargo run --bin zoi-completions`
/// in a directory specified by the environment variable `OUT_DIR`.
fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not set");
    let mut app = Cli::command();
    let bin_name = app.get_name().to_string();

    for &shell in Shell::value_variants() {
        generate_to(shell, &mut app, &bin_name, &out_dir)?;
    }

    println!("Completion scripts are generated in {out_dir:?}");
    Ok(())
}
