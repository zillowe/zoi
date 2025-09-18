use crate::cli::Cli;
use clap::CommandFactory;
use clap_mangen::Man;
use std::io::{self, Write};

pub fn run() -> io::Result<()> {
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    io::stdout().write_all(&buffer)?;
    Ok(())
}
