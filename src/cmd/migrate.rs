use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand, ValueHint};
use colored::Colorize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
pub struct MigrateCommand {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Convert a Scoop manifest (scoop.json) into a Zoi .pkg.lua package file
    Scoop(ScoopCommand),
}

#[derive(Parser, Debug)]
pub struct ScoopCommand {
    /// Path to Scoop manifest JSON/JSON5 file
    #[arg(required = true, value_hint = ValueHint::FilePath)]
    input: PathBuf,
    /// Output path for generated .pkg.lua (default: <package-name>.pkg.lua)
    #[arg(long, short = 'o', value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
    /// Repository tier to set in metadata.repo
    #[arg(long, default_value = "community")]
    repo: String,
    /// Override package name (default: from filename stem)
    #[arg(long)]
    name: Option<String>,
    /// Override version (default: Scoop manifest version)
    #[arg(long)]
    version: Option<String>,
    /// Maintainer name in generated metadata
    #[arg(long, default_value = "Scoop Migration")]
    maintainer_name: String,
    /// Maintainer email in generated metadata
    #[arg(long, default_value = "noreply@example.com")]
    maintainer_email: String,
    /// Print generated pkg.lua instead of writing a file
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone)]
struct BinMapping {
    source: String,
    target: String,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
struct DownloadSpec {
    arch_key: String,
    urls: Vec<String>,
    hashes: Vec<String>,
    extract_dir: Option<String>,
    extract_to: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct HookScripts {
    pre_install: Vec<String>,
    post_install: Vec<String>,
    pre_remove: Vec<String>,
    post_remove: Vec<String>,
}

#[derive(Debug, Clone)]
struct Generated {
    name: String,
    repo: String,
    version: String,
    description: String,
    website: String,
    license: String,
    maintainer_name: String,
    maintainer_email: String,
    bins: Vec<BinMapping>,
    runtime_deps: Vec<String>,
    backups: Vec<String>,
    downloads: Vec<DownloadSpec>,
    hooks: HookScripts,
    notes: Vec<String>,
    manifest: Value,
}

pub fn run(args: MigrateCommand) -> Result<()> {
    match args.command {
        Commands::Scoop(cmd) => run_scoop(cmd),
    }
}

fn run_scoop(args: ScoopCommand) -> Result<()> {
    let content = std::fs::read_to_string(&args.input)
        .map_err(|e| anyhow!("Failed to read '{}': {}", args.input.display(), e))?;
    let manifest: Value = json5::from_str(&content).map_err(|e| {
        anyhow!(
            "Failed to parse Scoop manifest '{}': {}",
            args.input.display(),
            e
        )
    })?;
    let manifest_obj = manifest
        .as_object()
        .ok_or_else(|| anyhow!("Scoop manifest root must be a JSON object."))?;

    let default_name = derive_name_from_path(&args.input)?;
    let name = normalize_name(args.name.as_deref().unwrap_or(&default_name));
    if name.is_empty() {
        return Err(anyhow!("Resolved package name is empty."));
    }

    let version = args
        .version
        .clone()
        .or_else(|| get_string_field(manifest_obj, "version"))
        .unwrap_or_else(|| "0.0.0".to_string());
    let description = get_string_field(manifest_obj, "description")
        .unwrap_or_else(|| format!("Migrated from Scoop: {}", name));
    let website = get_string_field(manifest_obj, "homepage")
        .unwrap_or_else(|| "https://scoop.sh/".to_string());
    let license = get_license_field(manifest_obj).unwrap_or_else(|| "NOASSERTION".to_string());

    let runtime_deps = parse_depends(manifest_obj.get("depends"));
    let bins = collect_bins(manifest_obj);
    let backups = parse_persist(manifest_obj.get("persist"));
    let downloads = collect_download_specs(manifest_obj)?;
    let hooks = collect_hook_scripts(manifest_obj);
    let notes = collect_migration_notes(manifest_obj, &bins, &downloads);

    let generated = Generated {
        name: name.clone(),
        repo: args.repo.clone(),
        version,
        description,
        website,
        license,
        maintainer_name: args.maintainer_name.clone(),
        maintainer_email: args.maintainer_email.clone(),
        bins,
        runtime_deps,
        backups,
        downloads,
        hooks,
        notes,
        manifest: manifest.clone(),
    };

    let lua = render_pkg_lua(&generated);
    if args.dry_run {
        println!("{}", lua);
        return Ok(());
    }

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_output_path(&args.input, &name));
    std::fs::write(&output, lua)
        .map_err(|e| anyhow!("Failed to write '{}': {}", output.display(), e))?;

    println!(
        "{} Generated {} from {}.",
        "::".bold().green(),
        output.display().to_string().cyan(),
        args.input.display().to_string().cyan()
    );

    Ok(())
}

fn derive_name_from_path(path: &Path) -> Result<String> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Failed to derive package name from '{}'.", path.display()))?;
    Ok(stem.to_string())
}

fn default_output_path(input: &Path, name: &str) -> PathBuf {
    match input.parent() {
        Some(parent) => parent.join(format!("{}.pkg.lua", name)),
        None => PathBuf::from(format!("{}.pkg.lua", name)),
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
}

fn get_string_field(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn get_license_field(map: &serde_json::Map<String, Value>) -> Option<String> {
    let value = map.get("license")?;
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(obj) = value.as_object() {
        for key in ["identifier", "id", "name"] {
            if let Some(v) = obj.get(key).and_then(Value::as_str) {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn value_to_string_list(value: Option<&Value>) -> Vec<String> {
    let Some(v) = value else {
        return Vec::new();
    };

    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_string()]
            }
        }
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_depends(depends: Option<&Value>) -> Vec<String> {
    value_to_string_list(depends)
        .into_iter()
        .map(|dep| {
            if dep.contains(':') {
                dep
            } else {
                format!("scoop:{}", dep)
            }
        })
        .collect()
}

fn parse_persist(persist: Option<&Value>) -> Vec<String> {
    let Some(value) = persist else {
        return Vec::new();
    };
    let mut result = Vec::new();

    match value {
        Value::String(s) => {
            let t = s.trim();
            if !t.is_empty() {
                result.push(t.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::String(s) => {
                        let t = s.trim();
                        if !t.is_empty() {
                            result.push(t.to_string());
                        }
                    }
                    Value::Array(arr) => {
                        if let Some(first) = arr.first().and_then(Value::as_str) {
                            let t = first.trim();
                            if !t.is_empty() {
                                result.push(t.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    result
}

fn collect_bins(manifest: &serde_json::Map<String, Value>) -> Vec<BinMapping> {
    let mut all = Vec::new();
    all.extend(parse_bin_value(manifest.get("bin")));

    if let Some(arch_obj) = manifest.get("architecture").and_then(Value::as_object) {
        let mut keys: Vec<_> = arch_obj.keys().cloned().collect();
        keys.sort();
        for key in keys {
            if let Some(bin_val) = arch_obj
                .get(&key)
                .and_then(Value::as_object)
                .and_then(|o| o.get("bin"))
            {
                all.extend(parse_bin_value(Some(bin_val)));
            }
        }
    }

    let mut seen = BTreeSet::new();
    all.into_iter()
        .filter(|bin| seen.insert((bin.source.clone(), bin.target.clone())))
        .collect()
}

fn parse_bin_value(value: Option<&Value>) -> Vec<BinMapping> {
    let Some(value) = value else {
        return Vec::new();
    };

    let mut mappings = Vec::new();
    match value {
        Value::String(path) => {
            let src = path.trim();
            if !src.is_empty() {
                let target = file_stem_or_basename(src);
                mappings.push(BinMapping {
                    source: src.to_string(),
                    target,
                    args: Vec::new(),
                });
            }
        }
        Value::Array(items) => {
            let all_strings = items.iter().all(|v| v.is_string());
            if all_strings && items.len() >= 2 {
                let src = items[0].as_str().unwrap_or("").trim().to_string();
                if !src.is_empty() {
                    let alias = items[1]
                        .as_str()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| file_stem_or_basename(&src));
                    let args = items
                        .iter()
                        .skip(2)
                        .filter_map(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    mappings.push(BinMapping {
                        source: src,
                        target: alias,
                        args,
                    });
                }
                return mappings;
            }

            for item in items {
                mappings.extend(parse_bin_value(Some(item)));
            }
        }
        _ => {}
    }

    mappings
}

fn file_stem_or_basename(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let base = normalized.rsplit('/').next().unwrap_or(path).trim();
    let lower = base.to_lowercase();

    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if lower.ends_with(ext) {
            return base[..base.len() - ext.len()].to_string();
        }
    }
    if let Some((stem, _)) = base.rsplit_once('.') {
        return stem.to_string();
    }
    base.to_string()
}

fn collect_download_specs(map: &serde_json::Map<String, Value>) -> Result<Vec<DownloadSpec>> {
    let root_urls = value_to_string_list(map.get("url"));
    let root_hashes = value_to_string_list(map.get("hash"));
    let root_extract_dir = get_string_field(map, "extract_dir");
    let root_extract_to = get_string_field(map, "extract_to");

    let mut specs = Vec::new();

    if !root_urls.is_empty() {
        specs.push(DownloadSpec {
            arch_key: "default".to_string(),
            urls: root_urls.clone(),
            hashes: root_hashes.clone(),
            extract_dir: root_extract_dir.clone(),
            extract_to: root_extract_to.clone(),
        });
    }

    if let Some(arch) = map.get("architecture").and_then(Value::as_object) {
        let mut keys: Vec<_> = arch.keys().cloned().collect();
        keys.sort();

        for key in keys {
            let arch_data = arch
                .get(&key)
                .and_then(Value::as_object)
                .ok_or_else(|| anyhow!("architecture.{} must be an object.", key))?;

            let urls = value_to_string_list(arch_data.get("url"));
            let hashes = value_to_string_list(arch_data.get("hash"));
            let extract_dir = get_string_field(arch_data, "extract_dir");
            let extract_to = get_string_field(arch_data, "extract_to");

            let merged_urls = if urls.is_empty() {
                root_urls.clone()
            } else {
                urls
            };
            let merged_hashes = if hashes.is_empty() {
                root_hashes.clone()
            } else {
                hashes
            };
            let merged_extract_dir = extract_dir.or(root_extract_dir.clone());
            let merged_extract_to = extract_to.or(root_extract_to.clone());

            if !merged_urls.is_empty() {
                specs.push(DownloadSpec {
                    arch_key: key,
                    urls: merged_urls,
                    hashes: merged_hashes,
                    extract_dir: merged_extract_dir,
                    extract_to: merged_extract_to,
                });
            }
        }
    }

    if specs.is_empty() {
        return Err(anyhow!(
            "Scoop manifest is missing download URL fields (`url` or `architecture.<arch>.url`)."
        ));
    }

    let mut by_key = BTreeMap::<String, DownloadSpec>::new();
    for spec in specs {
        by_key.insert(spec.arch_key.clone(), spec);
    }

    Ok(by_key.into_values().collect())
}

fn collect_hook_scripts(map: &serde_json::Map<String, Value>) -> HookScripts {
    let mut hooks = HookScripts::default();

    hooks
        .pre_install
        .extend(value_to_string_list(map.get("pre_install")));
    hooks
        .post_install
        .extend(value_to_string_list(map.get("post_install")));
    hooks
        .pre_remove
        .extend(value_to_string_list(map.get("pre_uninstall")));
    hooks
        .post_remove
        .extend(value_to_string_list(map.get("post_uninstall")));

    if let Some(installer_obj) = map.get("installer").and_then(Value::as_object) {
        hooks
            .post_install
            .extend(value_to_string_list(installer_obj.get("script")));
    }
    if let Some(uninstaller_obj) = map.get("uninstaller").and_then(Value::as_object) {
        hooks
            .pre_remove
            .extend(value_to_string_list(uninstaller_obj.get("script")));
    }

    if let Some(arch) = map.get("architecture").and_then(Value::as_object) {
        for arch_data in arch.values().filter_map(Value::as_object) {
            hooks
                .pre_install
                .extend(value_to_string_list(arch_data.get("pre_install")));
            hooks
                .post_install
                .extend(value_to_string_list(arch_data.get("post_install")));
            hooks
                .pre_remove
                .extend(value_to_string_list(arch_data.get("pre_uninstall")));
            hooks
                .post_remove
                .extend(value_to_string_list(arch_data.get("post_uninstall")));

            if let Some(installer_obj) = arch_data.get("installer").and_then(Value::as_object) {
                hooks
                    .post_install
                    .extend(value_to_string_list(installer_obj.get("script")));
            }
            if let Some(uninstaller_obj) = arch_data.get("uninstaller").and_then(Value::as_object) {
                hooks
                    .pre_remove
                    .extend(value_to_string_list(uninstaller_obj.get("script")));
            }
        }
    }

    dedupe_strings(&mut hooks.pre_install);
    dedupe_strings(&mut hooks.post_install);
    dedupe_strings(&mut hooks.pre_remove);
    dedupe_strings(&mut hooks.post_remove);
    hooks
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|v| seen.insert(v.clone()));
}

fn collect_migration_notes(
    manifest: &serde_json::Map<String, Value>,
    bins: &[BinMapping],
    downloads: &[DownloadSpec],
) -> Vec<String> {
    let mut notes = Vec::new();

    if manifest.get("checkver").is_some() {
        notes.push("`checkver` is preserved in `SCOOP_MANIFEST` for maintainer automation, but has no direct runtime equivalent in Zoi.".to_string());
    }
    if manifest.get("autoupdate").is_some() {
        notes.push("`autoupdate` is preserved in `SCOOP_MANIFEST`; update automation remains a maintainer workflow.".to_string());
    }
    if manifest.get("shortcuts").is_some() {
        notes.push("`shortcuts` are preserved in `SCOOP_MANIFEST`; shortcut creation is not auto-generated in this migration.".to_string());
    }
    if manifest.get("env_set").is_some() || manifest.get("env_add_path").is_some() {
        notes.push("Environment modifications (`env_set` / `env_add_path`) are preserved in `SCOOP_MANIFEST` and should be reviewed manually.".to_string());
    }
    if manifest.get("suggest").is_some() {
        notes.push("`suggest` is preserved in `SCOOP_MANIFEST` for manual review; it is not mapped to hard dependencies.".to_string());
    }
    if manifest.get("psmodule").is_some() {
        notes.push("`psmodule` is preserved in `SCOOP_MANIFEST`; validate module installation behavior manually.".to_string());
    }
    if manifest.get("innosetup").is_some() || manifest.get("msi").is_some() {
        notes.push("Installer metadata (`innosetup` / `msi`) is preserved in `SCOOP_MANIFEST`; verify installer flow manually.".to_string());
    }
    if let Some(installer) = manifest.get("installer").and_then(Value::as_object)
        && (installer.get("file").is_some() || installer.get("args").is_some())
    {
        notes.push("`installer.file` / `installer.args` are preserved in `SCOOP_MANIFEST`; they are not auto-translated into package() steps.".to_string());
    }
    if let Some(uninstaller) = manifest.get("uninstaller").and_then(Value::as_object)
        && (uninstaller.get("file").is_some() || uninstaller.get("args").is_some())
    {
        notes.push("`uninstaller.file` / `uninstaller.args` are preserved in `SCOOP_MANIFEST`; they are not auto-translated into uninstall hooks.".to_string());
    }
    if bins.iter().any(|b| !b.args.is_empty()) {
        notes.push("Bin shim extra args are preserved in BIN_MAP but not automatically applied in generated `bins` metadata.".to_string());
    }
    if downloads.len() > 1 {
        notes.push("Multiple architecture/source entries detected. Review generated arch selection and extraction paths.".to_string());
    }
    if manifest.get("cookie").is_some() {
        notes.push("`cookie` is preserved in `SCOOP_MANIFEST`; authenticated download behavior may require manual adaptation.".to_string());
    }
    if notes.is_empty() {
        notes.push("No major migration warnings detected. Review generated paths and hooks before publishing.".to_string());
    }

    notes
}

fn render_pkg_lua(g: &Generated) -> String {
    let mut out = String::new();

    out.push_str("-- Generated by `zoi migrate scoop`\n");
    out.push_str("-- This attempts broad Scoop manifest coverage.\n");
    out.push_str("-- Review generated behavior before publishing.\n");
    out.push_str("-- Migration notes:\n");
    for note in &g.notes {
        out.push_str(&format!("-- - {}\n", note));
    }
    out.push('\n');

    out.push_str("local SCOOP_MANIFEST = ");
    out.push_str(&json_to_lua(&g.manifest, 0));
    out.push_str("\n\n");

    out.push_str("local DOWNLOADS = {\n");
    for spec in &g.downloads {
        out.push_str(&format!("  [{}] = {{\n", lua_quote(&spec.arch_key)));
        out.push_str("    urls = {");
        for (idx, url) in spec.urls.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&lua_quote(url));
        }
        out.push_str("},\n");

        out.push_str("    hashes = {");
        for (idx, hash) in spec.hashes.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&lua_quote(hash));
        }
        out.push_str("},\n");

        out.push_str(&format!(
            "    extract_dir = {},\n",
            lua_optional(spec.extract_dir.as_deref())
        ));
        out.push_str(&format!(
            "    extract_to = {},\n",
            lua_optional(spec.extract_to.as_deref())
        ));
        out.push_str("  },\n");
    }
    out.push_str("}\n\n");

    out.push_str("local BIN_MAP = {\n");
    for bin in &g.bins {
        out.push_str("  {\n");
        out.push_str(&format!("    source = {},\n", lua_quote(&bin.source)));
        out.push_str(&format!("    target = {},\n", lua_quote(&bin.target)));
        out.push_str("    args = {");
        for (idx, arg) in bin.args.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&lua_quote(arg));
        }
        out.push_str("},\n");
        out.push_str("  },\n");
    }
    out.push_str("}\n\n");

    out.push_str("local RUNTIME_DEPS = {");
    for (idx, dep) in g.runtime_deps.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(dep));
    }
    out.push_str("}\n\n");

    out.push_str("local HOOK_SCRIPTS = {\n");
    out.push_str("  pre_install = {");
    for (idx, line) in g.hooks.pre_install.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(line));
    }
    out.push_str("},\n");
    out.push_str("  post_install = {");
    for (idx, line) in g.hooks.post_install.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(line));
    }
    out.push_str("},\n");
    out.push_str("  pre_remove = {");
    for (idx, line) in g.hooks.pre_remove.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(line));
    }
    out.push_str("},\n");
    out.push_str("  post_remove = {");
    for (idx, line) in g.hooks.post_remove.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(line));
    }
    out.push_str("},\n");
    out.push_str("}\n\n");

    out.push_str("local function scoop_arch_key()\n");
    out.push_str("  if SYSTEM.ARCH == \"amd64\" then return \"64bit\" end\n");
    out.push_str("  if SYSTEM.ARCH == \"arm64\" then return \"arm64\" end\n");
    out.push_str(
        "  if SYSTEM.ARCH == \"386\" or SYSTEM.ARCH == \"i386\" then return \"32bit\" end\n",
    );
    out.push_str("  return SYSTEM.ARCH\n");
    out.push_str("end\n\n");

    out.push_str("local function active_download()\n");
    out.push_str("  local arch = scoop_arch_key()\n");
    out.push_str("  if DOWNLOADS[arch] then return DOWNLOADS[arch] end\n");
    out.push_str("  if DOWNLOADS.default then return DOWNLOADS.default end\n");
    out.push_str("  for _, key in ipairs({ \"64bit\", \"32bit\", \"arm64\" }) do\n");
    out.push_str("    if DOWNLOADS[key] then return DOWNLOADS[key] end\n");
    out.push_str("  end\n");
    out.push_str("  for _, value in pairs(DOWNLOADS) do\n");
    out.push_str("    return value\n");
    out.push_str("  end\n");
    out.push_str("  return nil\n");
    out.push_str("end\n\n");

    out.push_str("local function powershell_cmd(line)\n");
    out.push_str("  return \"powershell -NoProfile -ExecutionPolicy Bypass -Command \" .. string.format(\"%q\", line)\n");
    out.push_str("end\n\n");

    out.push_str("local function register_hooks()\n");
    out.push_str("  local generated = {}\n");
    out.push_str("  local function add(name, lines)\n");
    out.push_str("    if not lines or #lines == 0 then return end\n");
    out.push_str("    generated[name] = generated[name] or {}\n");
    out.push_str("    generated[name].windows = generated[name].windows or {}\n");
    out.push_str("    for _, line in ipairs(lines) do\n");
    out.push_str("      table.insert(generated[name].windows, powershell_cmd(line))\n");
    out.push_str("    end\n");
    out.push_str("  end\n");
    out.push_str("  add(\"pre_install\", HOOK_SCRIPTS.pre_install)\n");
    out.push_str("  add(\"post_install\", HOOK_SCRIPTS.post_install)\n");
    out.push_str("  add(\"pre_remove\", HOOK_SCRIPTS.pre_remove)\n");
    out.push_str("  add(\"post_remove\", HOOK_SCRIPTS.post_remove)\n");
    out.push_str("  if next(generated) then hooks(generated) end\n");
    out.push_str("end\n\n");
    out.push_str("register_hooks()\n\n");

    out.push_str("local function split_hash(hash)\n");
    out.push_str("  if not hash or hash == \"\" then return nil, nil end\n");
    out.push_str("  local algo, digest = hash:match(\"^(sha512|sha256|sha1|md5)[:%-](.+)$\")\n");
    out.push_str("  if algo and digest then return algo, digest end\n");
    out.push_str("  return \"sha256\", hash\n");
    out.push_str("end\n\n");

    out.push_str("local function sanitize_url_file_name(url)\n");
    out.push_str("  if not url then return nil end\n");
    out.push_str("  local cleaned = url:gsub(\"#.*$\", \"\")\n");
    out.push_str("  return cleaned:match(\"([^/]+)$\")\n");
    out.push_str("end\n\n");

    out.push_str("local function source_roots_for_download(dl)\n");
    out.push_str("  local roots = {}\n");
    out.push_str("  local base_roots = {}\n");
    out.push_str("  if dl.extract_to and dl.extract_to ~= \"\" then\n");
    out.push_str("    table.insert(base_roots, dl.extract_to)\n");
    out.push_str("  elseif #dl.urls <= 1 then\n");
    out.push_str("    table.insert(base_roots, \"source\")\n");
    out.push_str("  else\n");
    out.push_str("    for i = 1, #dl.urls do table.insert(base_roots, \"source_\" .. i) end\n");
    out.push_str("  end\n");
    out.push_str("  for _, root in ipairs(base_roots) do table.insert(roots, root) end\n");
    out.push_str("  if dl.extract_dir and dl.extract_dir ~= \"\" then\n");
    out.push_str("    for _, root in ipairs(base_roots) do table.insert(roots, root .. \"/\" .. dl.extract_dir) end\n");
    out.push_str("  end\n");
    out.push_str("  return roots\n");
    out.push_str("end\n\n");

    out.push_str("local function resolve_bin_source(bin_source, roots)\n");
    out.push_str("  for _, root in ipairs(roots) do\n");
    out.push_str("    local candidate = root .. \"/\" .. bin_source\n");
    out.push_str("    if UTILS.FS.exists(candidate) then return candidate end\n");
    out.push_str("    local file_name = bin_source:match(\"([^/\\\\]+)$\")\n");
    out.push_str("    if file_name then\n");
    out.push_str("      local found = UTILS.FIND.file(root, file_name)\n");
    out.push_str("      if found and UTILS.FS.exists(found) then return found end\n");
    out.push_str("    end\n");
    out.push_str("  end\n");
    out.push_str("  return nil\n");
    out.push_str("end\n\n");

    out.push_str("metadata({\n");
    out.push_str(&format!("  name = {},\n", lua_quote(&g.name)));
    out.push_str(&format!("  repo = {},\n", lua_quote(&g.repo)));
    out.push_str(&format!("  version = {},\n", lua_quote(&g.version)));
    out.push_str(&format!("  description = {},\n", lua_quote(&g.description)));
    out.push_str(&format!("  website = {},\n", lua_quote(&g.website)));
    out.push_str(&format!("  license = {},\n", lua_quote(&g.license)));
    out.push_str(&format!(
        "  maintainer = {{ name = {}, email = {} }},\n",
        lua_quote(&g.maintainer_name),
        lua_quote(&g.maintainer_email)
    ));
    out.push_str("  types = { \"pre-compiled\" },\n");
    out.push_str("  bins = {");
    for (idx, bin) in g.bins.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&lua_quote(&bin.target));
    }
    out.push_str(" },\n");
    if !g.backups.is_empty() {
        out.push_str("  backup = {");
        for (idx, path) in g.backups.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&lua_quote(path));
        }
        out.push_str(" },\n");
    }
    out.push_str("})\n\n");

    if !g.runtime_deps.is_empty() {
        out.push_str("dependencies({\n");
        out.push_str("  runtime = {\n");
        out.push_str("    required = RUNTIME_DEPS,\n");
        out.push_str("  }\n");
        out.push_str("})\n\n");
    }

    out.push_str("function prepare()\n");
    out.push_str("  local dl = active_download()\n");
    out.push_str(
        "  if not dl then error(\"No download source matched current architecture\") end\n",
    );
    out.push_str("  for i, url in ipairs(dl.urls) do\n");
    out.push_str("    local out_dir\n");
    out.push_str("    if dl.extract_to and dl.extract_to ~= \"\" then\n");
    out.push_str("      out_dir = dl.extract_to\n");
    out.push_str("    elseif #dl.urls <= 1 then\n");
    out.push_str("      out_dir = \"source\"\n");
    out.push_str("    else\n");
    out.push_str("      out_dir = \"source_\" .. i\n");
    out.push_str("    end\n");
    out.push_str("    UTILS.EXTRACT(url, out_dir)\n");
    out.push_str("  end\n");
    out.push_str("end\n\n");

    out.push_str("function package()\n");
    out.push_str("  local dl = active_download()\n");
    out.push_str(
        "  if not dl then error(\"No download source matched current architecture\") end\n",
    );
    out.push_str("  local roots = source_roots_for_download(dl)\n");
    out.push_str("  for _, bin in ipairs(BIN_MAP) do\n");
    out.push_str("    local source_path = resolve_bin_source(bin.source, roots)\n");
    out.push_str("    if not source_path then\n");
    out.push_str(
        "      error(\"Could not locate bin source in extracted files: \" .. bin.source)\n",
    );
    out.push_str("    end\n");
    out.push_str("    zcp(source_path, \"${pkgstore}/bin/\" .. bin.target)\n");
    out.push_str("  end\n");
    out.push_str("end\n\n");

    out.push_str("function verify()\n");
    out.push_str("  local dl = active_download()\n");
    out.push_str("  if not dl then return true end\n");
    out.push_str("  local roots = source_roots_for_download(dl)\n");
    out.push_str("  for i, hash in ipairs(dl.hashes or {}) do\n");
    out.push_str("    local algo, digest = split_hash(hash)\n");
    out.push_str("    if algo and digest then\n");
    out.push_str("      local url = dl.urls[i] or dl.urls[1]\n");
    out.push_str("      local file_name = sanitize_url_file_name(url)\n");
    out.push_str("      local file_path = nil\n");
    out.push_str("      if file_name then\n");
    out.push_str("        for _, root in ipairs(roots) do\n");
    out.push_str("          local candidate = root .. \"/\" .. file_name\n");
    out.push_str("          if UTILS.FS.exists(candidate) then\n");
    out.push_str("            file_path = candidate\n");
    out.push_str("            break\n");
    out.push_str("          end\n");
    out.push_str("          local found = UTILS.FIND.file(root, file_name)\n");
    out.push_str("          if found and UTILS.FS.exists(found) then\n");
    out.push_str("            file_path = found\n");
    out.push_str("            break\n");
    out.push_str("          end\n");
    out.push_str("        end\n");
    out.push_str("      end\n");
    out.push_str("      if file_path then\n");
    out.push_str("        verifyHash(file_path, algo .. \"-\" .. digest)\n");
    out.push_str("      end\n");
    out.push_str("    end\n");
    out.push_str("  end\n");
    out.push_str("  return true\n");
    out.push_str("end\n");

    out
}

fn lua_quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{}\"", escaped)
}

fn lua_optional(value: Option<&str>) -> String {
    match value {
        Some(v) if !v.trim().is_empty() => lua_quote(v),
        _ => "nil".to_string(),
    }
}

fn json_to_lua(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "nil".to_string(),
        Value::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => lua_quote(s),
        Value::Array(arr) => {
            if arr.is_empty() {
                return "{}".to_string();
            }
            let mut out = String::new();
            out.push_str("{\n");
            let next_indent = indent + 2;
            for item in arr {
                out.push_str(&" ".repeat(next_indent));
                out.push_str(&json_to_lua(item, next_indent));
                out.push_str(",\n");
            }
            out.push_str(&" ".repeat(indent));
            out.push('}');
            out
        }
        Value::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();

            let mut out = String::new();
            out.push_str("{\n");
            let next_indent = indent + 2;
            for key in keys {
                if let Some(v) = map.get(&key) {
                    out.push_str(&" ".repeat(next_indent));
                    out.push_str(&format!("[{}] = ", lua_quote(&key)));
                    out.push_str(&json_to_lua(v, next_indent));
                    out.push_str(",\n");
                }
            }
            out.push_str(&" ".repeat(indent));
            out.push('}');
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_bin_string_and_array() {
        let bins = parse_bin_value(Some(&json!(["pwsh.exe", ["tools/foo.exe", "foo", "--x"]])));
        assert_eq!(bins.len(), 2);
        assert_eq!(bins[0].target, "pwsh");
        assert_eq!(bins[1].target, "foo");
        assert_eq!(bins[1].args, vec!["--x".to_string()]);
    }

    #[test]
    fn collect_download_specs_arch_fallback() {
        let manifest = json!({
            "url": "https://example.com/default.zip",
            "hash": "abc",
            "architecture": {
                "64bit": {
                    "url": "https://example.com/x64.zip",
                    "hash": "def"
                },
                "arm64": {}
            }
        });
        let specs = collect_download_specs(manifest.as_object().expect("object"))
            .expect("download specs should parse");
        assert!(specs.iter().any(|s| s.arch_key == "64bit"));
        assert!(specs.iter().any(|s| s.arch_key == "arm64"));
        assert!(specs.iter().any(|s| s.arch_key == "default"));
    }
}
