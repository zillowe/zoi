//! Lua-based project configuration loading.
//!
//! This module implements the parsing and execution of `zoi.lua` files.
//! It uses `mlua` to provide a declarative API within Lua for defining
//! project metadata, packages, registries, tasks, and environments.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mlua::{Lua, LuaSerdeExt, Table, Value};

use crate::config::{
    CommandSpec, EnvironmentSpec, ImportSpec, PackageCheck, PackageExportSpec,
    PackageSpec, ProjectConfig, ProjectLocalConfig, RegistrySpec, ShellSpec
};

/// Configuration values captured while evaluating a Lua project file.
#[derive(Clone, Default)]
struct EvaluatorState {
    /// Project metadata values.
    project: HashMap<String, serde_json::Value>,
    /// Package specifications by name.
    packages: HashMap<String, PackageSpec>,
    /// Registry specifications by name.
    registries: HashMap<String, RegistrySpec>,
    /// Project tasks.
    tasks: Vec<CommandSpec>,
    /// Project environments.
    environments: Vec<EnvironmentSpec>,
    /// Package checks.
    checks: Vec<PackageCheck>,
    /// Shell configuration.
    shell: Option<ShellSpec>,
    /// Repository imports by alias.
    imports: BTreeMap<String, ImportSpec>,
    /// Optional package export definition.
    package_export: Option<PackageExportSpec>
}

/// Lexical category of a token in a Lua configuration file.
#[derive(Clone, Copy, Eq, PartialEq)]
enum TokenKind {
    /// An identifier token.
    Identifier,
    /// A string token.
    String,
    /// A symbol token.
    Symbol
}

/// A token extracted from a Lua configuration file.
struct Token {
    /// Source text represented by the token.
    text: String,
    /// Lexical category of the token.
    kind: TokenKind,
    /// One-based source line containing the token.
    line: usize
}

/// Registers the declarative configuration functions in the Lua environment.
///
/// # Errors
///
/// Returns an error if a Lua value or global function cannot be created.
fn register_functions<S: ::std::hash::BuildHasher>(
    lua: &Lua,
    env: &HashMap<String, String, S>,
    state: Arc<Mutex<EvaluatorState>>
) -> mlua::Result<()> {
    let env_table = lua.create_table()?;
    for (key, value) in env {
        env_table.set(key.as_str(), value.as_str())?;
    }
    lua.globals().set("ENV", env_table)?;

    let project_state = state.clone();
    let project_fn = lua.create_function(move |lua, table: Table| {
        let mut state = project_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for pair in table.pairs::<String, Value>() {
            let (key, value) = pair?;
            state
                .project
                .insert(key, lua.from_value::<serde_json::Value>(value)?);
        }
        Ok(())
    })?;
    lua.globals().set("project", project_fn)?;

    let packages_state = state.clone();
    let packages_fn = lua.create_function(move |lua, table: Table| {
        let mut state = packages_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for pair in table.pairs::<Value, Value>() {
            let (key, value) = pair?;
            match key {
                Value::String(name) => {
                    let name = name.to_str()?.trim().to_string();
                    let spec = lua.from_value::<PackageSpec>(value)?;
                    state.packages.insert(name, spec);
                }
                Value::Integer(_) => {
                    if let Value::String(spec) = value {
                        state.packages.insert(
                            spec.to_str()?.trim().to_string(),
                            PackageSpec {
                                package_type: None,
                                install_method: None,
                                sub_packages: None,
                                version: None,
                                dependencies: None,
                                options: None,
                                optionals: None
                            }
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })?;
    lua.globals().set("packages", packages_fn)?;

    let registries_state = state.clone();
    let registries_fn = lua.create_function(move |lua, table: Table| {
        let mut state = registries_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for pair in table.pairs::<String, Value>() {
            let (key, value) = pair?;
            let spec = lua.from_value::<RegistrySpec>(value)?;
            state.registries.insert(key, spec);
        }
        Ok(())
    })?;
    lua.globals().set("registries", registries_fn)?;

    let tasks_state = state.clone();
    let tasks_fn = lua.create_function(move |lua, table: Table| {
        let mut state = tasks_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for value in table.sequence_values::<Value>() {
            state.tasks.push(lua.from_value::<CommandSpec>(value?)?);
        }
        Ok(())
    })?;
    lua.globals().set("tasks", tasks_fn)?;

    let environments_state = state.clone();
    let environments_fn = lua.create_function(move |lua, table: Table| {
        let mut state = environments_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for value in table.sequence_values::<Value>() {
            state
                .environments
                .push(lua.from_value::<EnvironmentSpec>(value?)?);
        }
        Ok(())
    })?;
    lua.globals().set("environments", environments_fn)?;

    let checks_state = state.clone();
    let checks_fn = lua.create_function(move |lua, table: Table| {
        let mut state = checks_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for value in table.sequence_values::<Value>() {
            state.checks.push(lua.from_value::<PackageCheck>(value?)?);
        }
        Ok(())
    })?;
    lua.globals().set("checks", checks_fn)?;

    let shell_state = state.clone();
    let shell_fn = lua.create_function(move |lua, table: Table| {
        let mut state = shell_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        state.shell = Some(lua.from_value::<ShellSpec>(Value::Table(table))?);
        Ok(())
    })?;
    lua.globals().set("shell", shell_fn)?;

    let imports_state = state.clone();
    let imports_fn = lua.create_function(move |lua, value: Value| {
        let Value::Table(table) = value else {
            return Err(mlua::Error::runtime(
                "imports() expects a table mapping names to repository specs"
            ));
        };
        let mut state = imports_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        for pair in table.pairs::<Value, Value>() {
            let (key, value) = pair?;
            let Value::String(name) = key else {
                return Err(mlua::Error::runtime(
                    "imports() keys must be string names"
                ));
            };
            let name_value = name.to_str()?;
            let import_name = name_value.trim();
            if import_name.is_empty() {
                return Err(mlua::Error::runtime(
                    "imports() names must not be empty"
                ));
            }
            let spec =
                lua.from_value::<ImportSpec>(value).map_err(|error| {
                    mlua::Error::runtime(format!(
                        "invalid import `{import_name}`: {error}; expected a \
                         table with `repo` and optional `rev`"
                    ))
                })?;
            if spec.repo.is_empty() {
                return Err(mlua::Error::runtime(format!(
                    "invalid import `{import_name}`: `repo` must not be empty"
                )));
            }
            if spec.rev.as_ref().is_some_and(String::is_empty) {
                return Err(mlua::Error::runtime(format!(
                    "invalid import `{import_name}`: `rev` must not be empty \
                     when provided"
                )));
            }
            if state
                .imports
                .insert(import_name.to_string(), spec)
                .is_some()
            {
                return Err(mlua::Error::runtime(format!(
                    "duplicate import name `{import_name}`"
                )));
            }
        }
        Ok(())
    })?;
    lua.globals().set("imports", imports_fn)?;

    let export_capture_state = state;
    let export_capture_fn = lua.create_function(move |lua, value: Value| {
        if export_capture_state
            .lock()
            .map_err(|_| {
                mlua::Error::runtime("zoi.lua configuration state is poisoned")
            })?
            .package_export
            .is_some()
        {
            return Err(mlua::Error::runtime(
                "package() may only be declared once"
            ));
        }

        let spec = match value {
            Value::String(path) => PackageExportSpec {
                main: Some(path.to_str()?.to_string()),
                packages: BTreeMap::new()
            },
            Value::Table(table) => lua
                .from_value::<PackageExportSpec>(Value::Table(table))
                .map_err(|error| {
                    mlua::Error::runtime(format!(
                        "invalid package table: {error}; expected optional \
                         `main` and optional string `packages` entries"
                    ))
                })?,
            _ => {
                return Err(mlua::Error::runtime(
                    "package() expects a path string or a table"
                ));
            }
        };

        if spec.main.as_ref().is_some_and(String::is_empty) {
            return Err(mlua::Error::runtime(
                "package.main must not be empty when provided"
            ));
        }
        if spec
            .packages
            .iter()
            .any(|(name, path)| name.is_empty() || path.is_empty())
        {
            return Err(mlua::Error::runtime(
                "package.packages names and paths must not be empty"
            ));
        }
        if spec.main.is_none() && spec.packages.is_empty() {
            return Err(mlua::Error::runtime(
                "package table must define `main` or at least one entry in \
                 `packages`"
            ));
        }

        let mut state = export_capture_state.lock().map_err(|_| {
            mlua::Error::runtime("zoi.lua configuration state is poisoned")
        })?;
        state.package_export = Some(spec);
        Ok(())
    })?;
    lua.globals().set("package", export_capture_fn)?;

    Ok(())
}

/// Finds the content range and end position of a Lua long bracket at `start`.
fn long_bracket(bytes: &[u8], start: usize) -> Option<(usize, usize, usize)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut equals = start.checked_add(1)?;
    while bytes.get(equals) == Some(&b'=') {
        equals = equals.checked_add(1)?;
    }
    if bytes.get(equals) != Some(&b'[') {
        return None;
    }
    let content_start = equals.checked_add(1)?;
    let mut close = content_start;
    while close < bytes.len() {
        if bytes.get(close) == Some(&b']') {
            let mut cursor = close.checked_add(1)?;
            while bytes.get(cursor) == Some(&b'=') {
                cursor = cursor.checked_add(1)?;
            }
            if bytes.get(cursor) == Some(&b']') {
                return Some((content_start, close, cursor.checked_add(1)?));
            }
        }
        close = close.checked_add(1)?;
    }
    None
}

/// Splits Lua configuration source into the tokens used for validation.
fn tokenize(content: &str) -> Vec<Token> {
    let bytes = content.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < bytes.len() {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        if byte == b'\n' {
            line += 1;
            index += 1;
        } else if byte.is_ascii_whitespace() {
            index += 1;
        } else if byte == b'-'
            && bytes.get(index.wrapping_add(1)) == Some(&b'-')
        {
            index = index.wrapping_add(2);
            if let Some((_, close, after)) = long_bracket(bytes, index) {
                line += content
                    .get(index..close)
                    .unwrap_or_default()
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count();
                index = after;
            } else {
                while index < bytes.len()
                    && bytes.get(index).is_some_and(|byte| *byte != b'\n')
                {
                    index += 1;
                }
            }
        } else if byte == b'\'' || byte == b'"' {
            let quote = byte;
            let token_line = line;
            let start = index;
            index += 1;
            while index < bytes.len() {
                let Some(&current) = bytes.get(index) else {
                    break;
                };
                if current == b'\\' {
                    index += 1;
                    if let Some(escaped) = bytes.get(index) {
                        if *escaped == b'\n' {
                            line += 1;
                        }
                        index += 1;
                    }
                } else {
                    if current == b'\n' {
                        line += 1;
                    }
                    index += 1;
                    if current == quote {
                        break;
                    }
                }
            }
            let end = index.min(bytes.len());
            let text = end
                .checked_sub(1)
                .and_then(|content_end| {
                    content.get(start.checked_add(1)?..content_end)
                })
                .unwrap_or_default()
                .to_string();
            tokens.push(Token {
                text,
                kind: TokenKind::String,
                line: token_line
            });
        } else if let Some((content_start, close, after)) =
            long_bracket(bytes, index)
        {
            let token_line = line;
            line += content
                .get(index..close)
                .unwrap_or_default()
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            tokens.push(Token {
                text: content
                    .get(content_start..close)
                    .unwrap_or_default()
                    .to_string(),
                kind: TokenKind::String,
                line: token_line
            });
            index = after;
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && bytes.get(index).is_some_and(|byte| {
                    byte.is_ascii_alphanumeric() || *byte == b'_'
                })
            {
                index += 1;
            }
            tokens.push(Token {
                text: content.get(start..index).unwrap_or_default().to_string(),
                kind: TokenKind::Identifier,
                line
            });
        } else {
            let symbol = bytes.get(index..index + 1).unwrap_or_default();
            tokens.push(Token {
                text: String::from_utf8_lossy(symbol).into_owned(),
                kind: TokenKind::Symbol,
                line
            });
            index += 1;
        }
    }

    tokens
}

/// Finds the matching closing delimiter for a token range.
fn matching_delimiter(
    tokens: &[Token],
    open: usize,
    opening: char,
    closing: char
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, token) in tokens.iter().enumerate().skip(open) {
        if token.text.chars().eq(std::iter::once(opening)) {
            depth += 1;
        } else if token.text.chars().eq(std::iter::once(closing)) {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

/// Finds the top-level key in a token range when one is present.
fn entry_key(tokens: &[Token], start: usize, end: usize) -> Option<&Token> {
    let mut depth = 0usize;
    for index in start..end {
        let text = tokens.get(index)?.text.as_str();
        match text {
            "{" | "[" | "(" => depth = depth.checked_add(1)?,
            "}" | "]" | ")" => depth = depth.checked_sub(1)?,
            "=" if depth == 0 => {
                return index.checked_sub(1).and_then(|key| tokens.get(key));
            }
            _ => {}
        }
    }
    None
}

/// Rejects duplicate keys within each top-level table in a token range.
///
/// # Errors
///
/// Returns an error for malformed delimiters or duplicate string and
/// identifier keys.
fn validate_table_duplicates(
    tokens: &[Token],
    open: usize,
    close: usize,
    block: &str
) -> Result<()> {
    let mut seen = BTreeSet::new();
    let mut entry_start = open
        .checked_add(1)
        .ok_or_else(|| anyhow!("zoi.lua contains an invalid {block} table"))?;
    let mut depth = 0usize;

    for index in open.checked_add(1).unwrap_or(close)..close {
        let text = tokens.get(index).map(|token| token.text.as_str());
        match text {
            Some("{" | "[" | "(") => {
                depth += 1;
            }
            Some("}" | "]" | ")") => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    anyhow!("zoi.lua contains an invalid {block} table")
                })?;
            }
            Some(",") if depth == 0 => {
                if let Some(key) = entry_key(tokens, entry_start, index)
                    && matches!(
                        key.kind,
                        TokenKind::String | TokenKind::Identifier
                    )
                    && !seen.insert(key.text.clone())
                {
                    return Err(anyhow!(
                        "zoi.lua line {}: duplicate key `{}` in {block} table",
                        key.line,
                        key.text
                    ));
                }
                entry_start = index.checked_add(1).ok_or_else(|| {
                    anyhow!("zoi.lua contains an invalid {block} table")
                })?;
            }
            _ => {}
        }
    }

    if let Some(key) = entry_key(tokens, entry_start, close)
        && matches!(key.kind, TokenKind::String | TokenKind::Identifier)
        && !seen.insert(key.text.clone())
    {
        return Err(anyhow!(
            "zoi.lua line {}: duplicate key `{}` in {block} table",
            key.line,
            key.text
        ));
    }

    Ok(())
}

/// Validates duplicate keys in supported declarative configuration tables.
///
/// # Errors
///
/// Returns an error if a table is malformed or contains a duplicate key.
fn validate_duplicate_keys(content: &str) -> Result<()> {
    let tokens = tokenize(content);
    for index in 0..tokens.len() {
        let token = tokens.get(index).ok_or_else(|| {
            anyhow!("zoi.lua contains an invalid configuration table")
        })?;
        if token.kind != TokenKind::Identifier
            || !matches!(token.text.as_str(), "imports" | "package")
        {
            continue;
        }
        let Some(paren) = tokens
            .get(index.checked_add(1).unwrap_or(tokens.len()))
            .filter(|token| token.text == "(")
            .map(|_| index.checked_add(1).unwrap_or(index))
        else {
            continue;
        };
        let call_close = matching_delimiter(&tokens, paren, '(', ')')
            .ok_or_else(|| anyhow!("zoi.lua contains an unterminated table"))?;
        let table_open = (paren + 1..call_close).find(|candidate| {
            tokens
                .get(*candidate)
                .is_some_and(|candidate| candidate.text == "{")
        });
        let Some(table_open) = table_open else {
            if token.text == "package"
                && (paren + 1..call_close).any(|candidate| {
                    tokens.get(candidate).is_some_and(|candidate| {
                        candidate.kind == TokenKind::String
                    })
                })
            {
                continue;
            }
            return Err(anyhow!(
                "{}() expects a table",
                if token.text == "imports" {
                    "imports"
                } else {
                    "package"
                }
            ));
        };

        let mut nested = table_open;
        while nested < call_close {
            if tokens
                .get(nested)
                .is_some_and(|candidate| candidate.text == "{")
            {
                let nested_close =
                    matching_delimiter(&tokens, nested, '{', '}').ok_or_else(
                        || anyhow!("zoi.lua contains an unterminated table")
                    )?;
                validate_table_duplicates(
                    &tokens,
                    nested,
                    nested_close,
                    &token.text
                )?;
                nested = nested_close.checked_add(1).unwrap_or(call_close);
            } else {
                nested += 1;
            }
        }
    }
    Ok(())
}

/// Executes a Lua project file and returns its captured configuration.
///
/// # Errors
///
/// Returns an error if the file cannot be read, validated, or executed.
fn evaluate_zoi_lua<S: ::std::hash::BuildHasher>(
    path: &Path,
    env: &HashMap<String, String, S>
) -> Result<EvaluatorState> {
    let content = fs::read_to_string(path)?;
    validate_duplicate_keys(&content)?;

    let lua = Lua::new();
    let state = Arc::new(Mutex::new(EvaluatorState::default()));
    register_functions(&lua, env, state.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    lua.load(&content)
        .exec()
        .map_err(|error| anyhow!("Failed to execute zoi.lua: {error}"))?;
    state
        .lock()
        .map_err(|_| anyhow!("zoi.lua configuration state is poisoned"))
        .map(|state| state.clone())
}

/// Converts captured Lua configuration into a project configuration.
///
/// # Errors
///
/// Returns an error if the configuration does not define a project name.
fn into_project_config(state: EvaluatorState) -> Result<ProjectConfig> {
    let name = state
        .project
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("zoi.lua must define project name"))?;
    let local = state
        .project
        .get("config")
        .and_then(serde_json::Value::as_object)
        .and_then(|object| object.get("local"))
        .and_then(serde_json::Value::as_bool)
        .or_else(|| {
            state
                .project
                .get("local")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false);
    let mut pkgs = Vec::new();
    for (name, spec) in &state.packages {
        if let Some(version) = &spec.version {
            pkgs.push(format!("{name}@{version}"));
        } else {
            pkgs.push(name.clone());
        }
    }

    Ok(ProjectConfig {
        name,
        registries: state.registries,
        packages: state.checks,
        pkgs,
        pkgs_v2: state.packages,
        config: ProjectLocalConfig { local },
        commands: state.tasks,
        environments: state.environments,
        shell: state.shell,
        imports: state.imports,
        package_export: state.package_export
    })
}

/// Parses and executes a `zoi.lua` file to load project-specific configuration.
///
/// This is the core of Zoi Specification v2. It:
/// - Exposes a declarative API (`project`, `packages`, `tasks`, etc.) to Lua.
/// - Executes the script, capturing metadata into temporary thread-safe maps.
/// - Resolves script-level choices into a static `ProjectConfig` struct.
///
/// This allows project environments to be programmable, enabling logic like
/// conditionally selecting registries based on environment variables.
///
/// # Errors
///
/// Returns an error if the `zoi.lua` file is missing, cannot be read, or
/// contains invalid syntax or logic.
pub fn load_zoi_lua<S: ::std::hash::BuildHasher>(
    path: &Path,
    env: &HashMap<String, String, S>
) -> Result<ProjectConfig> {
    into_project_config(evaluate_zoi_lua(path, env)?)
}

/// Evaluates `zoi.lua` and returns only repository imports and package exports.
///
/// The loader accepts repository metadata files that omit `project()` and do
/// not require `project.name`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or contains invalid Lua,
/// configuration, imports, or package export definitions.
pub fn load_repo_zoi_lua(
    path: &Path
) -> Result<(BTreeMap<String, ImportSpec>, Option<PackageExportSpec>)> {
    let env: HashMap<String, String> = std::env::vars().collect();
    let state = evaluate_zoi_lua(path, &env)?;
    Ok((state.imports, state.package_export))
}
