use crate::utils;
use anyhow::Result;
use clap_complete::Shell;
use zoi_resolver::local;

pub fn run(shell: Shell, index: usize, words: Vec<String>) -> Result<()> {
    if index <= 1 {
        return Ok(());
    }

    if words.len() < 2 {
        return Ok(());
    }

    let subcmd = &words[1];

    match subcmd.as_str() {
        "install" | "i" | "in" | "add" | "show" | "exec" | "x" | "create" | "clone" | "use"
        | "tree" | "man" | "shell" => {
            let pkgs = utils::get_all_packages_for_completion();
            for pkg in pkgs {
                if shell == Shell::Zsh {
                    println!("{}:{}", pkg.display, pkg.description);
                } else {
                    println!("{}", pkg.display);
                }
            }
        }
        "uninstall" | "un" | "rm" | "remove" | "mark" | "m" | "update" | "up" | "why" | "files"
        | "pin" | "unpin" | "downgrade" | "dg" | "rollback" => {
            if let Ok(installed) = local::get_installed_packages() {
                for pkg in installed {
                    let display = local::installed_manifest_source(&pkg);
                    if shell == Shell::Zsh {
                        println!("{}:{}", display, pkg.description);
                    } else {
                        println!("{}", display);
                    }
                }
            }
        }
        _ => {
            // Future: add more contexts (e.g. registry handles, scopes, etc.)
        }
    }

    Ok(())
}
