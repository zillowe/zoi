use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};

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

const BUILD_ENV_VARS: &[&str] = &[
    "ZOI_COMMIT_HASH",
    "POSTHOG_API_KEY",
    "POSTHOG_API_HOST",
    "ZOI_ABOUT_PACKAGER_AUTHOR",
    "ZOI_ABOUT_PACKAGER_EMAIL",
    "ZOI_ABOUT_PACKAGER_HOMEPAGE",
    "ZOI_DEFAULT_REGISTRY",
];
const DEFAULT_REGISTRY: &str = "https://gitlab.com/zillowe/zillwen/zusty/zoidberg.git";
const AUTHORITIES_KEY_RANGE: std::ops::Range<usize> = 1..10;

fn forward_env_var(var: &str) {
    if let Ok(val) = env::var(var) {
        println!("cargo:rustc-env={var}={val}");
    }
}

fn escaped_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn read_sorted_paths(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some(extension) {
                paths.push(path);
            }
        }
    }

    paths.sort();
    Ok(paths)
}

fn file_stem(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("path has no valid UTF-8 file stem: {}", path.display()).into())
}

fn write_builtin_bytes(
    out_dir: &Path,
    input_dir: &Path,
    output_file: &str,
    static_name: &str,
    extension: &str,
) -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", input_dir.display());

    let dest_path = out_dir.join(output_file);
    let mut file = std::fs::File::create(dest_path)?;

    writeln!(&mut file, "pub static {static_name}: &[(&str, &[u8])] = &[")?;
    for path in read_sorted_paths(input_dir, extension)? {
        let name = file_stem(&path)?;
        let content = std::fs::read(&path)?;
        writeln!(
            &mut file,
            "    ({}, &{:?}),",
            escaped_string_literal(name),
            content
        )?;
    }
    writeln!(&mut file, "];")?;

    Ok(())
}

fn write_builtin_strings(
    out_dir: &Path,
    input_dir: &Path,
    output_file: &str,
    static_name: &str,
    extension: &str,
) -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", input_dir.display());

    let dest_path = out_dir.join(output_file);
    let mut file = std::fs::File::create(dest_path)?;

    writeln!(&mut file, "pub static {static_name}: &[(&str, &str)] = &[")?;
    for path in read_sorted_paths(input_dir, extension)? {
        let name = file_stem(&path)?;
        let content = std::fs::read_to_string(&path)?;
        writeln!(
            &mut file,
            "    ({}, {}),",
            escaped_string_literal(name),
            escaped_string_literal(&content)
        )?;
    }
    writeln!(&mut file, "];")?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    for var in BUILD_ENV_VARS {
        println!("cargo:rerun-if-env-changed={}", var);
    }
    for i in AUTHORITIES_KEY_RANGE {
        println!("cargo:rerun-if-env-changed=ZOI_AUTHORITIES_KEY_{}", i);
    }

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

    for var in BUILD_ENV_VARS
        .iter()
        .copied()
        .filter(|var| *var != "ZOI_DEFAULT_REGISTRY")
    {
        forward_env_var(var);
    }

    let zoi_registry = env::var("ZOI_DEFAULT_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY.into());
    println!("cargo:rustc-env=ZOI_DEFAULT_REGISTRY={}", zoi_registry);

    let mut authorities = Vec::new();
    for i in AUTHORITIES_KEY_RANGE {
        if let Ok(val) = env::var(format!("ZOI_AUTHORITIES_KEY_{}", i))
            && !val.is_empty()
        {
            authorities.push(val);
        }
    }
    println!(
        "cargo:rustc-env=ZOI_BUILTIN_AUTHORITIES={}",
        authorities.join(",")
    );

    let managers_json_path = Path::new("src/pkg/pm/managers.json");
    println!("cargo:rerun-if-changed={}", managers_json_path.display());

    let out_dir = env::var("OUT_DIR")?;
    let out_dir = PathBuf::from(out_dir);
    let dest_path = out_dir.join("generated_managers.rs");
    let mut file = std::fs::File::create(dest_path)?;

    let json_str = std::fs::read_to_string(managers_json_path)?;
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

    write_builtin_bytes(
        &out_dir,
        Path::new("src/builtin/pgp"),
        "generated_pgp_keys.rs",
        "BUILTIN_KEYS",
        "asc",
    )?;
    write_builtin_strings(
        &out_dir,
        Path::new("src/builtin/hooks"),
        "generated_builtin_hooks.rs",
        "BUILTIN_HOOKS",
        "yaml",
    )?;

    Ok(())
}
