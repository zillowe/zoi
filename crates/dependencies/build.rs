use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct ManagerCommands {
    is_installed: Option<String>,
    install: String,
    uninstall: String,
    #[serde(default)]
    sudo_install: bool,
    #[serde(default)]
    sudo_uninstall: bool,
}

fn escaped_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let managers_json_path = manifest_dir.join("src").join("managers.json");

    println!("cargo:rerun-if-changed={}", managers_json_path.display());

    let out_dir = env::var("OUT_DIR")?;
    let out_dir = PathBuf::from(out_dir);
    let dest_path = out_dir.join("generated_managers.rs");
    let mut file = std::fs::File::create(dest_path)?;

    let json_str = std::fs::read_to_string(&managers_json_path)?;
    let managers: BTreeMap<String, ManagerCommands> = serde_json::from_str(&json_str)?;

    let mut map = phf_codegen::Map::new();
    let mut values = Vec::with_capacity(managers.len());

    for (name, commands) in &managers {
        let is_installed_val = match &commands.is_installed {
            Some(s) => format!("Some({})", escaped_string_literal(s)),
            None => "None".to_string(),
        };

        let value = format!(
            "ManagerCommands {{ is_installed: {}, install: {}, uninstall: {}, sudo_install: {}, sudo_uninstall: {} }}",
            is_installed_val,
            escaped_string_literal(&commands.install),
            escaped_string_literal(&commands.uninstall),
            commands.sudo_install,
            commands.sudo_uninstall
        );
        values.push((name.as_str(), value));
    }

    for (name, value) in &values {
        map.entry(name, value);
    }

    writeln!(
        &mut file,
        "use ::phf;\n\n#[derive(Debug, Clone)]\npub struct ManagerCommands {{\n    pub is_installed: Option<&'static str>,\n    pub install: &'static str,\n    pub uninstall: &'static str,\n    pub sudo_install: bool,\n    pub sudo_uninstall: bool,\n}}\n"
    )?;

    writeln!(
        &mut file,
        "pub static MANAGERS: phf::Map<&'static str, ManagerCommands> = {};",
        map.build()
    )?;

    Ok(())
}
