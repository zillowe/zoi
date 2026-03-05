use crate::pkg::types;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::PathBuf;

pub fn get_db_path(registry_handle: &str) -> Result<PathBuf> {
    let db_root = crate::pkg::resolve::get_db_root()?;
    Ok(db_root.join(format!("{}.db", registry_handle)))
}

pub fn open_connection(registry_handle: &str) -> Result<Connection> {
    let conn = open_connection_no_setup(registry_handle)?;
    setup_schema(&conn)?;
    Ok(conn)
}

pub fn open_connection_no_setup(registry_handle: &str) -> Result<Connection> {
    let db_path = get_db_path(registry_handle)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

fn setup_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sub_package TEXT,
            repo TEXT NOT NULL,
            version TEXT,
            description TEXT,
            package_type TEXT,
            tags TEXT,
            bins TEXT,
            license TEXT,
            registry TEXT,
            scope TEXT,
            reason TEXT,
            dependencies TEXT,
            UNIQUE(name, sub_package, repo, scope)
        )",
        [],
    )?;

    let has_deps: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE name='dependencies'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !has_deps {
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN dependencies TEXT", []);
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE name='bins'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN bins TEXT", []);
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_repo ON packages(repo)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS package_files (
            id INTEGER PRIMARY KEY,
            package_id INTEGER,
            path TEXT NOT NULL,
            FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_package_files_path ON package_files(path)",
        [],
    )?;

    let mut fts_needs_rebuild = false;
    let fts_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='packages_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if fts_exists {
        let has_bins_fts: bool = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('packages_fts') WHERE name='bins'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_bins_fts {
            fts_needs_rebuild = true;
        }
    } else {
        fts_needs_rebuild = true;
    }

    if fts_needs_rebuild {
        let _ = conn.execute("DROP TABLE IF EXISTS packages_fts", []);
        let _ = conn.execute("DROP TRIGGER IF EXISTS packages_ai", []);
        let _ = conn.execute("DROP TRIGGER IF EXISTS packages_ad", []);
        let _ = conn.execute("DROP TRIGGER IF EXISTS packages_au", []);

        let _ = conn.execute(
            "CREATE VIRTUAL TABLE packages_fts USING fts5(name, description, tags, bins, content='packages', content_rowid='id')",
            [],
        );

        let _ = conn.execute(
            "INSERT INTO packages_fts(rowid, name, description, tags, bins) 
             SELECT id, name, description, tags, bins FROM packages",
            [],
        );

        let _ = conn.execute(
            "CREATE TRIGGER packages_ai AFTER INSERT ON packages BEGIN
                INSERT INTO packages_fts(rowid, name, description, tags, bins) VALUES (new.id, new.name, new.description, new.tags, new.bins);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_ad AFTER DELETE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, description, tags, bins) VALUES('delete', old.id, old.name, old.description, old.tags, old.bins);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_au AFTER UPDATE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, description, tags, bins) VALUES('delete', old.id, old.name, old.description, old.tags, old.bins);
                INSERT INTO packages_fts(rowid, name, description, tags, bins) VALUES (new.id, new.name, new.description, new.tags, new.bins);
            END",
            [],
        );
    }

    let files_fts_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='package_files_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !files_fts_exists {
        let _ = conn.execute(
            "CREATE VIRTUAL TABLE package_files_fts USING fts5(path, content='package_files', content_rowid='id')",
            [],
        );

        let _ = conn.execute(
            "INSERT INTO package_files_fts(rowid, path) SELECT id, path FROM package_files",
            [],
        );

        let _ = conn.execute(
            "CREATE TRIGGER package_files_ai AFTER INSERT ON package_files BEGIN
                INSERT INTO package_files_fts(rowid, path) VALUES (new.id, new.path);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER package_files_ad AFTER DELETE ON package_files BEGIN
                INSERT INTO package_files_fts(package_files_fts, rowid, path) VALUES('delete', old.id, old.path);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER package_files_au AFTER UPDATE ON package_files BEGIN
                INSERT INTO package_files_fts(package_files_fts, rowid, path) VALUES('delete', old.id, old.path);
                INSERT INTO package_files_fts(rowid, path) VALUES (new.id, new.path);
            END",
            [],
        );
    }

    Ok(())
}

pub fn update_package(
    conn: &Connection,
    pkg: &types::Package,
    registry: &str,
    scope: Option<types::Scope>,
    sub_package: Option<&str>,
    reason: Option<&types::InstallReason>,
) -> Result<i64> {
    let tags_json = serde_json::to_string(&pkg.tags)?;
    let bins_json =
        serde_json::to_string(&pkg.bins.as_ref().unwrap_or(&vec![])).unwrap_or_default();
    let pkg_type = format!("{:?}", pkg.package_type).to_lowercase();
    let scope_str = scope.map(|s| format!("{:?}", s).to_lowercase());
    let reason_str = reason.map(|r| match r {
        types::InstallReason::Direct => "direct".to_string(),
        types::InstallReason::Dependency { parent } => format!("dependency:{}", parent),
        types::InstallReason::Declarative => "declarative".to_string(),
    });

    let deps_json = if let Some(deps) = &pkg.dependencies {
        serde_json::to_string(deps).unwrap_or_default()
    } else {
        String::new()
    };

    conn.execute(
        "INSERT INTO packages (name, sub_package, repo, version, description, package_type, tags, bins, license, registry, scope, reason, dependencies)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(name, sub_package, repo, scope) DO UPDATE SET
            version = excluded.version,
            description = excluded.description,
            package_type = excluded.package_type,
            tags = excluded.tags,
            bins = excluded.bins,
            license = excluded.license,
            registry = excluded.registry,
            reason = COALESCE(excluded.reason, packages.reason),
            dependencies = excluded.dependencies",
        params![
            pkg.name,
            sub_package,
            pkg.repo,
            pkg.version,
            pkg.description,
            pkg_type,
            tags_json,
            bins_json,
            pkg.license,
            registry,
            scope_str,
            reason_str,
            deps_json,
        ],
    )?;

    let row_id = conn.query_row(
        "SELECT id FROM packages WHERE name = ?1 AND (sub_package IS ?2) AND repo = ?3 AND (scope IS ?4 OR (scope IS NULL AND ?4 IS NULL))",
        params![pkg.name, sub_package, pkg.repo, scope_str],
        |row| row.get(0),
    )?;

    Ok(row_id)
}

#[derive(Debug)]
pub struct CompletionEntry {
    pub name: String,
    pub repo: String,
    pub description: String,
    pub sub_package: Option<String>,
}

pub fn get_packages_for_completion(registry_handle: &str) -> Result<Vec<CompletionEntry>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt =
        conn.prepare("SELECT name, repo, description, sub_package FROM packages ORDER BY name")?;

    let rows = stmt.query_map([], |row| {
        Ok(CompletionEntry {
            name: row.get(0)?,
            repo: row.get(1)?,
            description: row.get(2).unwrap_or_default(),
            sub_package: row.get(3)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

pub fn index_package_files(conn: &Connection, package_id: i64, files: &[String]) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO package_files (package_id, path) VALUES (?1, ?2)")?;
    for file in files {
        stmt.execute(params![package_id, file])?;
    }
    Ok(())
}

pub fn delete_package(
    conn: &Connection,
    name: &str,
    sub_package: Option<&str>,
    repo: &str,
    scope: Option<types::Scope>,
) -> Result<()> {
    let scope_str = scope.map(|s| format!("{:?}", s).to_lowercase());
    conn.execute(
        "DELETE FROM packages WHERE name = ?1 AND (sub_package IS ?2) AND repo = ?3 AND (scope IS ?4 OR scope IS NULL)",
        params![name, sub_package, repo, scope_str],
    )?;
    Ok(())
}

pub fn clear_registry(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM packages", [])?;
    Ok(())
}

pub fn find_provides(registry_handle: &str, term: &str) -> Result<Vec<(types::Package, String)>> {
    let conn = open_connection(registry_handle)?;

    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, bins, license, sub_package 
         FROM packages 
         WHERE name = ?1 OR bins LIKE ?2",
    )?;

    let bins_like_query = format!("%\"{}\"%", term);
    let rows = stmt.query_map(params![term, bins_like_query], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
        let bins_raw: String = row.get(6)?;
        let bins: Vec<String> = serde_json::from_str(&bins_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package,
        };

        Ok(types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            bins: Some(bins),
            license: row.get(7)?,
            sub_package: row.get(8)?,
            maintainer: types::Maintainer::default(),
            ..Default::default()
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        let pkg = row?;
        results.push((pkg, format!("bin/{}", term)));
    }

    let mut stmt = conn.prepare(
        "SELECT p.name, p.repo, p.version, p.description, p.package_type, p.tags, p.bins, p.license, p.sub_package, pf.path 
         FROM packages p
         JOIN package_files pf ON p.id = pf.package_id
         WHERE pf.path LIKE ?1 OR pf.path LIKE ?2",
    )?;

    let path_like_query = format!("%/{}", term);
    let exact_path_query = term.to_string();

    let rows = stmt.query_map(params![path_like_query, exact_path_query], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
        let bins_raw: String = row.get(6)?;
        let bins: Vec<String> = serde_json::from_str(&bins_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package,
        };

        let pkg = types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            bins: Some(bins),
            license: row.get(7)?,
            sub_package: row.get(8)?,
            maintainer: types::Maintainer::default(),
            ..Default::default()
        };
        let path: String = row.get(9)?;
        Ok((pkg, path))
    })?;

    for row in rows {
        results.push(row?);
    }

    results.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    results.dedup_by(|a, b| a.0.name == b.0.name && a.1 == b.1);

    Ok(results)
}

pub fn search_packages(registry_handle: &str, term: &str) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, license, sub_package 
         FROM packages 
         WHERE id IN (SELECT rowid FROM packages_fts WHERE packages_fts MATCH ?1)
         OR name LIKE ?2",
    )?;

    let search_query = format!("{}*", term);
    let like_query = format!("%{}%", term);

    let rows = stmt.query_map(params![search_query, like_query], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package,
        };

        Ok(types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            sub_package: row.get(7)?,
            maintainer: types::Maintainer {
                name: String::new(),
                email: String::new(),
                website: None,
            },
            ..Default::default()
        })
    })?;

    let mut pkgs = Vec::new();
    for row in rows {
        pkgs.push(row?);
    }
    Ok(pkgs)
}

pub fn search_files(registry_handle: &str, term: &str) -> Result<Vec<(types::Package, String)>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT p.name, p.repo, p.version, p.description, p.package_type, p.tags, p.license, p.sub_package, pf.path 
         FROM packages p
         JOIN package_files pf ON p.id = pf.package_id
         WHERE pf.id IN (SELECT rowid FROM package_files_fts WHERE package_files_fts MATCH ?1)
         OR pf.path LIKE ?2",
    )?;

    let search_query = format!("*{}*", term.replace('/', " "));
    let like_query = format!("%{}%", term);

    let rows = stmt.query_map(params![search_query, like_query], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package,
        };

        let pkg = types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            sub_package: row.get(7)?,
            maintainer: types::Maintainer {
                name: String::new(),
                email: String::new(),
                website: None,
            },
            ..Default::default()
        };
        let path: String = row.get(8)?;
        Ok((pkg, path))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn list_all_packages(registry_handle: &str) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, license, sub_package, scope, registry, reason FROM packages ORDER BY name"
    )?;

    let rows = stmt.query_map([], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package,
        };

        let sub_package: Option<String> = row.get(7)?;
        let scope_raw: Option<String> = row.get(8)?;
        let registry: Option<String> = row.get(9)?;
        let reason_raw: Option<String> = row.get(10)?;

        let scope = match scope_raw.as_deref() {
            Some("system") => types::Scope::System,
            Some("project") => types::Scope::Project,
            _ => types::Scope::User,
        };

        let reason = reason_raw.map(|r| {
            if r == "direct" {
                types::InstallReason::Direct
            } else if r == "declarative" {
                types::InstallReason::Declarative
            } else if let Some(parent) = r.strip_prefix("dependency:") {
                types::InstallReason::Dependency {
                    parent: parent.to_string(),
                }
            } else {
                types::InstallReason::Direct
            }
        });

        let pkg = types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            scope,
            registry_handle: registry,
            sub_package,
            reason,
            maintainer: types::Maintainer {
                name: String::new(),
                email: String::new(),
                website: None,
            },
            ..Default::default()
        };

        Ok(pkg)
    })?;

    let mut pkgs = Vec::new();
    for row in rows {
        pkgs.push(row?);
    }
    Ok(pkgs)
}

pub fn get_all_package_names(registry_handle: &str) -> Result<Vec<String>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare("SELECT DISTINCT name FROM packages")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut names = Vec::new();
    for name in rows {
        names.push(name?);
    }
    Ok(names)
}

pub fn get_all_versions(registry_handle: &str, name: &str, repo: &str) -> Result<Vec<String>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare("SELECT version FROM packages WHERE name = ?1 AND repo = ?2")?;
    let rows = stmt.query_map(params![name, repo], |row| row.get(0))?;
    let mut versions = Vec::new();
    for v in rows.flatten() {
        versions.push(v);
    }
    Ok(versions)
}

pub fn get_package_dependencies(
    registry_handle: &str,
    name: &str,
    version: &str,
    sub_package: Option<&str>,
    repo: &str,
) -> Result<Option<String>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT dependencies FROM packages 
         WHERE name = ?1 AND version = ?2 AND (sub_package IS ?3) AND repo = ?4",
    )?;
    let mut rows = stmt.query(params![name, version, sub_package, repo])?;
    if let Some(row) = rows.next()? {
        let deps: Option<String> = row.get(0)?;
        Ok(deps)
    } else {
        Ok(None)
    }
}
