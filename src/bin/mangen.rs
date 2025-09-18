use clap::CommandFactory;
use clap_mangen::Man;
use std::io::Result;
use std::path::PathBuf;
use std::{env, fs};
use zoi::cli::Cli;

/// Man page can be created with:
/// `cargo run --bin zoi-mangen`
/// in a directory specified by the environment variable `OUT_DIR`.
fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not set");

    let app = Cli::command();
    let name = app.get_name().to_string();
    let out_path = PathBuf::from(&out_dir).join(format!("{}.1", name));

    let man = Man::new(app);
    let mut buffer = Vec::<u8>::new();

    man.render(&mut buffer)?;

    fs::write(&out_path, buffer)?;
    println!("Man page is generated at {out_path:?}");

    Ok(())
}
