use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::Write;
use std::path::Path;

#[derive(serde::Deserialize)]
struct ManagerCommands {
    is_installed: Option<String>,
    install: String,
    uninstall: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let env_path = if Path::new(".env").exists() {
        Some(".env")
    } else if Path::new(".env.local").exists() {
        Some(".env.local")
    } else {
        None
    };

    if let Some(path) = env_path {
        println!("cargo:rerun-if-changed={}", path);
        if dotenvy::from_filename(path).is_err() {
            println!("cargo:warning=failed to load env file: {}", path);
        }
    }

    if let Ok(val) = env::var("POSTHOG_API_KEY") {
        println!("cargo:rustc-env=POSTHOG_API_KEY={}", val);
    }
    if let Ok(val) = env::var("POSTHOG_API_HOST") {
        println!("cargo:rustc-env=POSTHOG_API_HOST={}", val);
    }
    if let Ok(val) = env::var("ZOI_ABOUT_PACKAGER_AUTHOR") {
        println!("cargo:rustc-env=ZOI_ABOUT_PACKAGER_AUTHOR={}", val);
    }
    if let Ok(val) = env::var("ZOI_ABOUT_PACKAGER_EMAIL") {
        println!("cargo:rustc-env=ZOI_ABOUT_PACKAGER_EMAIL={}", val);
    }
    if let Ok(val) = env::var("ZOI_ABOUT_PACKAGER_HOMEPAGE") {
        println!("cargo:rustc-env=ZOI_ABOUT_PACKAGER_HOMEPAGE={}", val);
    }
    let zoi_registry = env::var("ZOI_DEFAULT_REGISTRY")
        .unwrap_or_else(|_| "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git".to_string());
    println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={}", zoi_registry);

    let managers_json_path = Path::new("src/pkg/pm/managers.json");
    println!("cargo:rerun-if-changed={}", managers_json_path.display());

    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("generated_managers.rs");
    let mut file = std::fs::File::create(dest_path)?;

    let json_str = std::fs::read_to_string(managers_json_path)?;
    let managers: HashMap<String, ManagerCommands> = serde_json::from_str(&json_str)?;

    let mut map = phf_codegen::Map::new();
    let mut values = Vec::new();

    for (name, commands) in &managers {
        let is_installed_val = match &commands.is_installed {
            Some(s) => format!("Some(\"{}\")", s.replace('\\', "\\\\").replace('"', "\\\"")),
            None => "None".to_string(),
        };

        let value = format!(
            "ManagerCommands {{ is_installed: {}, install: \"{}\", uninstall: \"{}\" }}",
            is_installed_val,
            commands.install.replace('\\', "\\\\").replace('"', "\\\""),
            commands
                .uninstall
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        );
        values.push((name.clone(), value));
    }

    for (name, value) in &values {
        map.entry(name, value);
    }

    writeln!(
        &mut file,
        "use ::phf;\n\n#[derive(Debug, Clone)]\npub struct ManagerCommands {{\n    pub is_installed: Option<&'static str>,\n    pub install: &'static str,\n    pub uninstall: &'static str,\n}}\n"
    )?;

    writeln!(
        &mut file,
        "pub static MANAGERS: phf::Map<&'static str, ManagerCommands> = {};",
        map.build()
    )?;

    Ok(())
}
