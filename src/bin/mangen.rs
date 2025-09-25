use clap::{Command, CommandFactory};
use clap_mangen::Man;
use std::io::Result;
use std::path::{Path, PathBuf};
use std::{env, fs};
use zoi::cli::Cli;

/// Man page can be created with:
/// `cargo run --bin zoi-mangen`
/// in a directory specified by the environment variable `OUT_DIR`.
fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not set");
    let out_path = PathBuf::from(out_dir);
    fs::create_dir_all(&out_path)?;

    let app = Cli::command();

    generate_man_page(&app, &out_path)?;

    for sub_command in app.get_subcommands() {
        generate_man_pages_recursive(sub_command, &out_path, app.get_name())?;
    }

    Ok(())
}

fn generate_man_pages_recursive(cmd: &Command, out_path: &Path, parent_name: &str) -> Result<()> {
    if cmd.is_hide_set() {
        return Ok(());
    }

    let full_name = format!("{}-{}", parent_name, cmd.get_name());
    let leaked_name: &'static str = Box::leak(full_name.into_boxed_str());
    let new_cmd = cmd.clone().name(leaked_name);
    generate_man_page(&new_cmd, out_path)?;

    for sub_cmd in new_cmd.get_subcommands() {
        generate_man_pages_recursive(sub_cmd, out_path, leaked_name)?;
    }

    Ok(())
}

fn generate_man_page(app: &Command, out_path: &Path) -> Result<()> {
    let name = app.get_name();
    let out_file = out_path.join(format!("{}.1", name));

    let man = Man::new(app.clone());
    let mut buffer = Vec::<u8>::new();
    man.render(&mut buffer)?;

    fs::write(&out_file, buffer)?;
    println!("Man page is generated at {:?}", out_file);

    Ok(())
}
