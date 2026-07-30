use anyhow::Result;
use std::fs;
use zoi_core::types;

pub fn install_manual_if_available(
    pkg: &types::Package,
    version: &str,
    registry_handle: &str,
    pb: Option<&indicatif::ProgressBar>,
) -> Result<()> {
    if let Some(man_spec) = &pkg.man {
        let version_dir = zoi_resolver::local::get_package_version_dir(
            pkg.scope,
            registry_handle,
            &pkg.repo,
            &pkg.name,
            version,
        )?;
        fs::create_dir_all(&version_dir)?;

        let client = zoi_core::utils::get_http_client()?;

        match man_spec {
            types::ManSpec::Single(url) => {
                let msg = format!("Downloading manual from {}...", url);
                if let Some(p) = pb {
                    p.println(msg);
                } else {
                    println!("{}", msg);
                }
                let content = client.get(url).send()?.bytes()?;
                let extension = if url.ends_with(".md") { "md" } else { "txt" };
                fs::write(version_dir.join(format!("man.{}", extension)), &content)?;
            }
            types::ManSpec::Multiple(urls) => {
                let share_man = version_dir.join("share").join("man");
                fs::create_dir_all(&share_man)?;
                for url in urls {
                    let name = url.split('/').next_back().unwrap_or("man.md");
                    let msg = format!("Downloading manual page {}...", name);
                    if let Some(p) = pb {
                        p.println(msg);
                    } else {
                        println!("{}", msg);
                    }
                    let content = client.get(url).send()?.bytes()?;
                    fs::write(share_man.join(name), &content)?;
                }
            }
            types::ManSpec::Map(map) => {
                let share_man = version_dir.join("share").join("man");
                fs::create_dir_all(&share_man)?;
                for (name, url) in map {
                    let msg = format!("Downloading manual page {}...", name);
                    if let Some(p) = pb {
                        p.println(msg);
                    } else {
                        println!("{}", msg);
                    }
                    let content = client.get(url).send()?.bytes()?;
                    fs::write(share_man.join(name), &content)?;
                }
            }
        }

        let success_msg = format!("Manual for '{}' installed.", pkg.name);
        if let Some(p) = pb {
            p.println(success_msg);
        } else {
            println!("{}", success_msg);
        }
    }
    Ok(())
}
