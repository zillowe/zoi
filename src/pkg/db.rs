use crate::pkg::types;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::PathBuf;

pub fn get_db_path(registry_handle: &str) -> Result<PathBuf> {
    let db_root = crate::pkg::resolve::get_db_root()?;
    Ok(db_root.join(format!("{}.db", registry_handle)))
}

pub fn open_connection(registry_handle: &str) -> Result<Connection> {
    let db_path = get_db_path(registry_handle)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    setup_schema(&conn)?;
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
            license TEXT,
            registry TEXT,
            scope TEXT,
            reason TEXT,
            UNIQUE(name, sub_package, repo, scope)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_repo ON packages(repo)",
        [],
    )?;

    let fts_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='packages_fts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !fts_exists {
        let _ = conn.execute(
            "CREATE VIRTUAL TABLE packages_fts USING fts5(name, description, tags, content='packages', content_rowid='id')",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_ai AFTER INSERT ON packages BEGIN
                INSERT INTO packages_fts(rowid, name, description, tags) VALUES (new.id, new.name, new.description, new.tags);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_ad AFTER DELETE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, description, tags) VALUES('delete', old.id, old.name, old.description, old.tags);
            END",
            [],
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_au AFTER UPDATE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, description, tags) VALUES('delete', old.id, old.name, old.description, old.tags);
                INSERT INTO packages_fts(rowid, name, description, tags) VALUES (new.id, new.name, new.description, new.tags);
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
) -> Result<()> {
    let tags_json = serde_json::to_string(&pkg.tags)?;
    let pkg_type = format!("{:?}", pkg.package_type).to_lowercase();
    let scope_str = scope.map(|s| format!("{:?}", s).to_lowercase());
    let reason_str = reason.map(|r| match r {
        types::InstallReason::Direct => "direct".to_string(),
        types::InstallReason::Dependency { parent } => format!("dependency:{}", parent),
    });

    conn.execute(
        "INSERT OR REPLACE INTO packages (name, sub_package, repo, version, description, package_type, tags, license, registry, scope, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            pkg.name,
            sub_package,
            pkg.repo,
            pkg.version,
            pkg.description,
            pkg_type,
            tags_json,
            pkg.license,
            registry,
            scope_str,
            reason_str,
        ],
    )?;
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

pub fn list_all_packages(registry_handle: &str) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, license, sub_package, scope, registry FROM packages ORDER BY name"
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

        let scope = match scope_raw.as_deref() {
            Some("system") => types::Scope::System,
            Some("project") => types::Scope::Project,
            _ => types::Scope::User,
        };

        let mut pkg = types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            scope,
            maintainer: types::Maintainer {
                name: String::new(),
                email: String::new(),
                website: None,
            },
            ..Default::default()
        };

        if let Some(sub) = sub_package {
            pkg.alt = Some(format!("sub:{}", sub));
        }
        if let Some(reg) = registry {
            pkg.readme = Some(reg);
        }

        Ok(pkg)
    })?;

    let mut pkgs = Vec::new();
    for row in rows {
        pkgs.push(row?);
    }
    Ok(pkgs)
}
