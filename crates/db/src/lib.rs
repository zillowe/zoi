//! Database management for Zoi registry metadata.
//!
//! This crate provides functionality to manage the SQLite-based registry
//! metadata, including package information, file indexing, and security
//! advisories.

use std::path::PathBuf;

use anyhow::Result;
use rusqlite::{Connection, params};
use zoi_core::types;
use zoi_resolver::resolve::{get_db_root, get_host_db_root};

/// Gets the path to the database for a specific registry.
///
/// # Errors
///
/// Returns an error if the database root directory cannot be determined.
pub fn get_db_path(registry_handle: &str) -> Result<PathBuf> {
    let target_root = get_db_root()?;
    let target_path = target_root.join(format!("{registry_handle}.db"));

    if !target_path.exists() && zoi_core::sysroot::get_sysroot().is_some() {
        // Fallback to host metadata for bootstrapping
        let host_root = get_host_db_root()?;
        let host_path = host_root.join(format!("{registry_handle}.db"));

        if host_path.exists() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Copy host registry DB to target so records are kept in target
            let _ = std::fs::copy(&host_path, &target_path);
        }
    }

    Ok(target_path)
}

/// Opens a connection to the registry database and ensures the schema is set
/// up.
///
/// # Errors
///
/// Returns an error if the database cannot be opened or if the schema setup
/// fails.
pub fn open_connection(registry_handle: &str) -> Result<Connection> {
    let conn = open_connection_no_setup(registry_handle)?;
    setup_schema(&conn)?;
    Ok(conn)
}

/// Opens a raw `SQLite` connection and configures high-performance pragmas.
///
/// Performance Tuning:
/// - `WAL` (Write-Ahead Logging): Allows concurrent readers and a single
///   writer.
/// - `NORMAL` Synchronous: Balanced safety and speed for registry metadata.
///
/// # Errors
///
/// Returns an error if the database cannot be opened or if PRAGMA configuration
/// fails.
pub fn open_connection_no_setup(registry_handle: &str) -> Result<Connection> {
    let db_path = get_db_path(registry_handle)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
    "
    )?;

    Ok(conn)
}

/// Initializes or migrates the Zoi registry metadata schema.
///
/// Schema Highlights:
/// - `packages`: The main metadata store with FTS5 search indexing.
/// - `package_files`: Index of every file path provided by every package (used
///   by `zoi provides`).
/// - `package_advisories`: Security vulnerability database.
///
/// FTS5 Search: Uses `SQLite`'s virtual tables for sub-millisecond full-text
/// search across thousands of package descriptions and tags.
fn setup_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sub_package TEXT,
            repo TEXT NOT NULL,
            version TEXT,
            epoch INTEGER DEFAULT 0,
            description TEXT,
            package_type TEXT,
            tags TEXT,
            bins TEXT,
            license TEXT,
            registry TEXT,
            scope TEXT,
            reason TEXT,
            dependencies TEXT,
            revision TEXT,
            archive_size INTEGER,
            installed_size INTEGER,
            UNIQUE(name, sub_package, repo, scope, registry)
        )",
        []
    )?;

    let has_epoch: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='epoch'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_epoch {
        let _ = conn.execute(
            "ALTER TABLE packages ADD COLUMN epoch INTEGER DEFAULT 0",
            []
        );
    }

    let has_revision: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='revision'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_revision {
        let _ =
            conn.execute("ALTER TABLE packages ADD COLUMN revision TEXT", []);
    }

    let has_deps: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='dependencies'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_deps {
        let _ = conn
            .execute("ALTER TABLE packages ADD COLUMN dependencies TEXT", []);
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='bins'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        let _ = conn.execute("ALTER TABLE packages ADD COLUMN bins TEXT", []);
    }

    let has_archive_size: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='archive_size'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_archive_size {
        let _ = conn.execute(
            "ALTER TABLE packages ADD COLUMN archive_size INTEGER",
            []
        );
        let _ = conn.execute(
            "ALTER TABLE packages ADD COLUMN installed_size INTEGER",
            []
        );
    }

    let has_archive_hash: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('packages') WHERE \
             name='archive_hash'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_archive_hash {
        let _ = conn
            .execute("ALTER TABLE packages ADD COLUMN archive_hash TEXT", []);
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name)",
        []
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_packages_repo ON packages(repo)",
        []
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS package_files (
            id INTEGER PRIMARY KEY,
            package_id INTEGER,
            path TEXT NOT NULL,
            FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
        )",
        []
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS package_advisories (
            id TEXT PRIMARY KEY,
            package TEXT NOT NULL,
            sub_package TEXT,
            summary TEXT NOT NULL,
            severity TEXT NOT NULL,
            cvss TEXT,
            affected_range TEXT NOT NULL,
            fixed_in TEXT,
            description TEXT NOT NULL,
            references_json TEXT,
            repo TEXT,
            registry TEXT
        )",
        []
    )?;

    let has_sub_pkg_adv: bool = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('package_advisories') \
             WHERE name='sub_package'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !has_sub_pkg_adv {
        let _ = conn.execute(
            "ALTER TABLE package_advisories ADD COLUMN sub_package TEXT",
            []
        );
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_package_advisories_package ON \
         package_advisories(package, sub_package)",
        []
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_package_files_path ON \
         package_files(path)",
        []
    )?;

    let mut fts_needs_rebuild = false;
    let fts_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND \
             name='packages_fts'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if fts_exists {
        let has_bins_fts: bool = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('packages_fts') WHERE \
                 name='bins'",
                [],
                |row| row.get(0)
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
            "CREATE VIRTUAL TABLE packages_fts USING fts5(name, description, \
             tags, bins, content='packages', content_rowid='id')",
            []
        );

        let _ = conn.execute(
            "INSERT INTO packages_fts(rowid, name, description, tags, bins) 
             SELECT id, name, description, tags, bins FROM packages",
            []
        );

        let _ = conn.execute(
            "CREATE TRIGGER packages_ai AFTER INSERT ON packages BEGIN
                INSERT INTO packages_fts(rowid, name, description, tags, bins) \
             VALUES (new.id, new.name, new.description, new.tags, new.bins);
            END",
            []
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_ad AFTER DELETE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, \
             description, tags, bins) VALUES('delete', old.id, old.name, \
             old.description, old.tags, old.bins);
            END",
            []
        );
        let _ = conn.execute(
            "CREATE TRIGGER packages_au AFTER UPDATE ON packages BEGIN
                INSERT INTO packages_fts(packages_fts, rowid, name, \
             description, tags, bins) VALUES('delete', old.id, old.name, \
             old.description, old.tags, old.bins);
                INSERT INTO packages_fts(rowid, name, description, tags, bins) \
             VALUES (new.id, new.name, new.description, new.tags, new.bins);
            END",
            []
        );
    }

    let files_fts_exists: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND \
             name='package_files_fts'",
            [],
            |row| row.get(0)
        )
        .unwrap_or(0)
        > 0;

    if !files_fts_exists {
        let _ = conn.execute(
            "CREATE VIRTUAL TABLE package_files_fts USING fts5(path, \
             content='package_files', content_rowid='id')",
            []
        );

        let _ = conn.execute(
            "INSERT INTO package_files_fts(rowid, path) SELECT id, path FROM \
             package_files",
            []
        );

        let _ = conn.execute(
            "CREATE TRIGGER package_files_ai AFTER INSERT ON package_files \
             BEGIN
                INSERT INTO package_files_fts(rowid, path) VALUES (new.id, \
             new.path);
            END",
            []
        );
        let _ = conn.execute(
            "CREATE TRIGGER package_files_ad AFTER DELETE ON package_files \
             BEGIN
                INSERT INTO package_files_fts(package_files_fts, rowid, path) \
             VALUES('delete', old.id, old.path);
            END",
            []
        );
        let _ = conn.execute(
            "CREATE TRIGGER package_files_au AFTER UPDATE ON package_files \
             BEGIN
                INSERT INTO package_files_fts(package_files_fts, rowid, path) \
             VALUES('delete', old.id, old.path);
                INSERT INTO package_files_fts(rowid, path) VALUES (new.id, \
             new.path);
            END",
            []
        );
    }

    Ok(())
}

/// Updates or inserts a package in the database.
///
/// # Errors
///
/// Returns an error if serialization fails or if the database execution fails.
pub fn update_package(
    conn: &Connection,
    pkg: &types::Package,
    registry: &str,
    scope: Option<types::Scope>,
    sub_package: Option<&str>,
    reason: Option<&types::InstallReason>
) -> Result<i64> {
    let tags_json = serde_json::to_string(&pkg.tags)?;
    let bins_json =
        serde_json::to_string(&pkg.bins.as_ref().unwrap_or(&vec![]))
            .unwrap_or_default();
    let pkg_type = format!("{:?}", pkg.package_type).to_lowercase();
    let scope_str = scope.map(|s| format!("{s:?}").to_lowercase());
    let reason_str = reason.map(|r| match r {
        types::InstallReason::Direct => "direct".to_string(),
        types::InstallReason::Dependency { parent } => {
            format!("dependency:{parent}")
        }
    });

    let deps_json = if let Some(deps) = &pkg.dependencies {
        serde_json::to_string(deps).unwrap_or_default()
    } else {
        String::new()
    };

    conn.execute(
        "INSERT INTO packages (name, sub_package, repo, version, epoch, \
         description, package_type, tags, bins, license, registry, scope, \
         reason, dependencies, revision)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
         ?15)
         ON CONFLICT(name, sub_package, repo, scope, registry) DO UPDATE SET
            version = excluded.version,
            epoch = excluded.epoch,
            description = excluded.description,
            package_type = excluded.package_type,
            tags = excluded.tags,
            bins = excluded.bins,
            license = excluded.license,
            reason = COALESCE(excluded.reason, packages.reason),
            dependencies = excluded.dependencies,
            revision = excluded.revision",
        params![
            pkg.name,
            sub_package,
            pkg.repo,
            pkg.version,
            pkg.epoch,
            pkg.description,
            pkg_type,
            tags_json,
            bins_json,
            pkg.license,
            registry,
            scope_str,
            reason_str,
            deps_json,
            pkg.revision,
        ]
    )?;

    let row_id = conn.query_row(
        "SELECT id FROM packages WHERE name = ?1 AND (sub_package IS ?2) AND \
         repo = ?3 AND (scope IS ?4 OR (scope IS NULL AND ?4 IS NULL)) AND \
         (registry IS ?5)",
        params![pkg.name, sub_package, pkg.repo, scope_str, registry],
        |row| row.get(0)
    )?;

    Ok(row_id)
}

/// Retrieves the internal database ID for a package.
///
/// # Errors
///
/// Returns an error if the query fails or if the package is not found.
pub fn get_package_id(
    conn: &Connection,
    name: &str,
    sub_package: Option<&str>,
    repo: &str,
    registry: &str
) -> Result<i64> {
    let id = conn.query_row(
        "SELECT id FROM packages WHERE name = ?1 AND (sub_package IS ?2) AND \
         repo = ?3 AND registry = ?4",
        params![name, sub_package, repo, registry],
        |row| row.get(0)
    )?;
    Ok(id)
}

/// Updates the archive and installed sizes for a package.
///
/// # Errors
///
/// Returns an error if the database update fails.
pub fn set_package_sizes(
    conn: &Connection,
    package_id: i64,
    archive_size: u64,
    installed_size: u64
) -> Result<()> {
    conn.execute(
        "UPDATE packages SET archive_size = ?1, installed_size = ?2 WHERE id \
         = ?3",
        params![archive_size as i64, installed_size as i64, package_id]
    )?;
    Ok(())
}

/// Updates the archive hash for a package.
///
/// # Errors
///
/// Returns an error if the database update fails.
pub fn set_package_hash(
    conn: &Connection,
    package_id: i64,
    hash: &str
) -> Result<()> {
    conn.execute(
        "UPDATE packages SET archive_hash = ?1 WHERE id = ?2",
        params![hash, package_id]
    )?;
    Ok(())
}

/// Retrieves the archive hash for a package from the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened.
pub fn get_package_hash_from_db(
    registry_handle: &str,
    name: &str,
    sub_package: Option<&str>,
    repo: &str
) -> Result<Option<String>> {
    let conn = open_connection(registry_handle)?;
    let Ok(pkg_id) =
        get_package_id(&conn, name, sub_package, repo, registry_handle)
    else {
        return Ok(None);
    };
    let hash: Option<String> = conn
        .query_row(
            "SELECT archive_hash FROM packages WHERE id = ?1 AND archive_hash \
             IS NOT NULL",
            params![pkg_id],
            |row| row.get(0)
        )
        .ok();
    Ok(hash)
}

/// Retrieves the archive and installed sizes for a package from the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_package_sizes_from_db(
    registry_handle: &str,
    name: &str,
    sub_package: Option<&str>
) -> Result<Option<(u64, u64)>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT archive_size, installed_size FROM packages WHERE name = ?1 \
         AND (sub_package IS ?2) AND archive_size IS NOT NULL LIMIT 1"
    )?;
    let mut rows = stmt.query(params![name, sub_package])?;
    if let Some(row) = rows.next()? {
        let archive: Option<i64> = row.get(0)?;
        let installed: Option<i64> = row.get(1)?;
        match (archive, installed) {
            (Some(a), Some(i)) => {
                Ok(Some((a.cast_unsigned(), i.cast_unsigned())))
            }
            _ => Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// Retrieves the list of files associated with a package from the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_package_files_from_db(
    registry_handle: &str,
    name: &str,
    sub_package: Option<&str>,
    repo: &str
) -> Result<Option<Vec<String>>> {
    let conn = open_connection(registry_handle)?;
    let Ok(pkg_id) =
        get_package_id(&conn, name, sub_package, repo, registry_handle)
    else {
        return Ok(None);
    };
    let mut stmt =
        conn.prepare("SELECT path FROM package_files WHERE package_id = ?1")?;
    let rows = stmt.query_map(params![pkg_id], |row| row.get(0))?;
    let mut files = Vec::new();
    for row in rows {
        files.push(row?);
    }
    if files.is_empty() {
        Ok(None)
    } else {
        Ok(Some(files))
    }
}

/// A single entry for shell completion.
#[derive(Debug)]
pub struct CompletionEntry {
    /// The name of the package.
    pub name: String,
    /// The repository where the package is located.
    pub repo: String,
    /// A short description of the package.
    pub description: String,
    /// The name of the sub-package, if applicable.
    pub sub_package: Option<String>
}

/// Retrieves a list of packages suitable for shell completion.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_packages_for_completion(
    registry_handle: &str
) -> Result<Vec<CompletionEntry>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, description, sub_package FROM packages ORDER BY \
         name"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CompletionEntry {
            name: row.get(0)?,
            repo: row.get(1)?,
            description: row.get(2).unwrap_or_default(),
            sub_package: row.get(3)?
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// Updates or inserts a security advisory in the database.
///
/// # Errors
///
/// Returns an error if serialization fails or if the database execution fails.
pub fn update_advisory(
    conn: &Connection,
    advisory: &types::Advisory,
    repo: &str,
    registry: &str
) -> Result<()> {
    let references_json =
        serde_json::to_string(&advisory.references).unwrap_or_default();
    let severity_str = format!("{:?}", advisory.severity).to_lowercase();

    conn.execute(
        "INSERT INTO package_advisories (id, package, sub_package, summary, \
         severity, cvss, affected_range, fixed_in, description, \
         references_json, repo, registry)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            package = excluded.package,
            sub_package = excluded.sub_package,
            summary = excluded.summary,
            severity = excluded.severity,
            cvss = excluded.cvss,
            affected_range = excluded.affected_range,
            fixed_in = excluded.fixed_in,
            description = excluded.description,
            references_json = excluded.references_json,
            repo = excluded.repo,
            registry = excluded.registry",
        params![
            advisory.id,
            advisory.package,
            advisory.sub_package,
            advisory.summary,
            severity_str,
            advisory.cvss,
            advisory.affected_range,
            advisory.fixed_in,
            advisory.description,
            references_json,
            repo,
            registry,
        ]
    )?;
    Ok(())
}

/// Lists all security advisories in the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn list_all_advisories(
    registry_handle: &str
) -> Result<Vec<(types::Advisory, String)>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT id, package, sub_package, summary, severity, cvss, \
         affected_range, fixed_in, description, references_json, repo FROM \
         package_advisories"
    )?;

    let rows = stmt.query_map([], |row| {
        let severity_raw: String = row.get(4)?;
        let severity = match severity_raw.as_str() {
            "medium" => types::Severity::Medium,
            "high" => types::Severity::High,
            "critical" => types::Severity::Critical,
            _ => types::Severity::Low
        };

        let references_raw: String = row.get(9)?;
        let references: Option<Vec<String>> =
            serde_json::from_str(&references_raw).ok();

        Ok((
            types::Advisory {
                id: row.get(0)?,
                package: row.get(1)?,
                sub_package: row.get(2)?,
                summary: row.get(3)?,
                severity,
                cvss: row.get(5)?,
                affected_range: row.get(6)?,
                fixed_in: row.get(7)?,
                description: row.get(8)?,
                references
            },
            row.get::<_, String>(10)?
        ))
    })?;

    let mut advisories = Vec::new();
    for row in rows {
        advisories.push(row?);
    }
    Ok(advisories)
}

/// Retrieves all security advisories for a specific package.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_advisories_for_package(
    registry_handle: &str,
    package_name: &str,
    sub_package: Option<&str>
) -> Result<Vec<types::Advisory>> {
    let conn = open_connection(registry_handle)?;

    let (query, params_vec): (String, Vec<rusqlite::types::Value>) =
        match sub_package {
            Some(sub) => (
                "SELECT id, package, sub_package, summary, severity, cvss, \
                 affected_range, fixed_in, description, references_json 
             FROM package_advisories 
             WHERE package = ?1 AND (sub_package IS ?2 OR sub_package IS NULL)"
                    .to_string(),
                vec![package_name.to_string().into(), sub.to_string().into()]
            ),
            None => (
                "SELECT id, package, sub_package, summary, severity, cvss, \
                 affected_range, fixed_in, description, references_json 
             FROM package_advisories 
             WHERE package = ?1 AND sub_package IS NULL"
                    .to_string(),
                vec![package_name.to_string().into()]
            )
        };

    let mut stmt = conn.prepare(&query)?;

    let rows =
        stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            let severity_raw: String = row.get(4)?;
            let severity = match severity_raw.as_str() {
                "medium" => types::Severity::Medium,
                "high" => types::Severity::High,
                "critical" => types::Severity::Critical,
                _ => types::Severity::Low
            };

            let references_raw: String = row.get(9)?;
            let references: Option<Vec<String>> =
                serde_json::from_str(&references_raw).ok();

            Ok(types::Advisory {
                id: row.get(0)?,
                package: row.get(1)?,
                sub_package: row.get(2)?,
                summary: row.get(3)?,
                severity,
                cvss: row.get(5)?,
                affected_range: row.get(6)?,
                fixed_in: row.get(7)?,
                description: row.get(8)?,
                references
            })
        })?;

    let mut advisories = Vec::new();
    for row in rows {
        advisories.push(row?);
    }
    Ok(advisories)
}

/// Indexes the files provided by a package.
///
/// # Errors
///
/// Returns an error if the database execution fails.
pub fn index_package_files(
    conn: &Connection,
    package_id: i64,
    files: &[String]
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO package_files (package_id, path) VALUES (?1, ?2)"
    )?;
    for file in files {
        stmt.execute(params![package_id, file])?;
    }
    Ok(())
}

/// Clears the file index for a specific package.
///
/// # Errors
///
/// Returns an error if the database execution fails.
pub fn clear_package_files(conn: &Connection, package_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM package_files WHERE package_id = ?1",
        params![package_id]
    )?;
    Ok(())
}

/// Checks if a file path is owned by any package other than the specified one.
///
/// # Errors
///
/// Returns an error if the query fails.
pub fn has_other_owners(
    conn: &Connection,
    path: &str,
    current_package_id: i64
) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM package_files WHERE path = ?1 AND package_id != \
         ?2",
        params![path, current_package_id],
        |row| row.get(0)
    )?;
    Ok(count > 0)
}

/// Deletes a package from the database.
///
/// # Errors
///
/// Returns an error if the database execution fails.
pub fn delete_package(
    conn: &Connection,
    name: &str,
    sub_package: Option<&str>,
    repo: &str,
    scope: Option<types::Scope>
) -> Result<()> {
    let scope_str = scope.map(|s| format!("{s:?}").to_lowercase());
    conn.execute(
        "DELETE FROM packages WHERE name = ?1 AND (sub_package IS ?2) AND \
         repo = ?3 AND (scope IS ?4 OR scope IS NULL)",
        params![name, sub_package, repo, scope_str]
    )?;
    Ok(())
}

/// Clears all package and advisory data from the database.
///
/// # Errors
///
/// Returns an error if the database execution fails.
pub fn clear_registry(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM packages", [])?;
    conn.execute("DELETE FROM package_advisories", [])?;
    Ok(())
}

/// Finds packages that provide a specific command or file.
///
/// # Errors
///
/// Returns an error if the database connection or query fails.
pub fn find_provides(
    registry_handle: &str,
    term: &str
) -> Result<Vec<(types::Package, String)>> {
    let conn = open_connection(registry_handle)?;

    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, bins, \
         license, sub_package, revision, epoch 
         FROM packages 
         WHERE name = ?1"
    )?;

    let rows = stmt.query_map(params![term], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_raw).unwrap_or_default();
        let bins_raw: String = row.get::<_, String>(6).unwrap_or_default();
        let bins: Vec<String> =
            serde_json::from_str(&bins_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;
        let revision: String = row.get(9).unwrap_or_else(|_| "1".to_string());
        let epoch: u32 = row.get(10).unwrap_or(0);

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package
        };

        Ok((
            types::Package {
                name: row.get(0)?,
                repo: row.get(1)?,
                version: row.get(2)?,
                epoch,
                revision,
                description: row.get(3)?,
                package_type,
                tags,
                bins: Some(bins.clone()),
                license: row.get(7)?,
                sub_package: row.get(8)?,
                maintainer: types::Maintainer::default(),
                ..Default::default()
            },
            bins
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (pkg, bins) = row?;
        if bins.is_empty() {
            results.push((pkg, format!("bin/{term}")));
        } else {
            for bin in &bins {
                results.push((pkg.clone(), format!("bin/{bin}")));
            }
        }
    }

    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, bins, \
         license, sub_package, revision, epoch 
         FROM packages 
         WHERE bins IS NOT NULL"
    )?;

    let rows = stmt.query_map([], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_raw).unwrap_or_default();
        let bins_raw: String = row.get(6)?;
        let bins: Vec<String> =
            serde_json::from_str(&bins_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;
        let revision: String = row.get(9).unwrap_or_else(|_| "1".to_string());
        let epoch: u32 = row.get(10).unwrap_or(0);

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package
        };

        Ok(types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            epoch,
            revision,
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

    for row in rows {
        let pkg = row?;
        if let Some(bins) = &pkg.bins {
            for bin in bins {
                if bin == term || bin.contains(term) {
                    results.push((pkg.clone(), format!("bin/{bin}")));
                }
            }
        }
    }

    let mut stmt = conn.prepare(
        "SELECT p.name, p.repo, p.version, p.description, p.package_type, \
         p.tags, p.bins, p.license, p.sub_package, pf.path, p.revision, \
         p.epoch 
         FROM packages p
         JOIN package_files pf ON p.id = pf.package_id
         WHERE pf.path LIKE ?1 OR pf.path LIKE ?2"
    )?;

    let path_like_query = format!("%/{term}");
    let exact_path_query = term.to_string();

    let rows =
        stmt.query_map(params![path_like_query, exact_path_query], |row| {
            let tags_raw: String = row.get(5)?;
            let tags: Vec<String> =
                serde_json::from_str(&tags_raw).unwrap_or_default();
            let bins_raw: String = row.get(6)?;
            let bins: Vec<String> =
                serde_json::from_str(&bins_raw).unwrap_or_default();
            let type_raw: String = row.get(4)?;
            let revision: String =
                row.get(10).unwrap_or_else(|_| "1".to_string());
            let epoch: u32 = row.get(11).unwrap_or(0);

            let package_type = match type_raw.as_str() {
                "collection" => types::PackageType::Collection,
                "app" => types::PackageType::App,
                "extension" => types::PackageType::Extension,
                _ => types::PackageType::Package
            };

            let pkg = types::Package {
                name: row.get(0)?,
                repo: row.get(1)?,
                version: row.get(2)?,
                epoch,
                revision,
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
        let (pkg, mut path): (types::Package, String) = row?;
        if let Some(stripped) = path.strip_prefix("data/pkgstore/") {
            path = format!("/{stripped}");
        } else if let Some(stripped) = path.strip_prefix("data/home/") {
            path = format!("~/{stripped}");
        }

        results.push((pkg, path));
    }

    results.sort_by(|a, b| {
        a.0.name
            .cmp(&b.0.name)
            .then(a.0.repo.cmp(&b.0.repo))
            .then(a.1.cmp(&b.1))
    });
    results.dedup_by(|a, b| {
        a.0.name == b.0.name && a.0.repo == b.0.repo && a.1 == b.1
    });

    Ok(results)
}

/// Searches for packages matching a search term.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn search_packages(
    registry_handle: &str,
    term: &str
) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, \
         license, sub_package, revision, epoch 
         FROM packages 
         WHERE id IN (SELECT rowid FROM packages_fts WHERE packages_fts MATCH \
         ?1)
         OR name LIKE ?2"
    )?;

    let search_query = format!("{term}*");
    let like_query = format!("%{term}%");

    let rows = stmt.query_map(params![search_query, like_query], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;
        let revision: String = row.get(8).unwrap_or_else(|_| "1".to_string());
        let epoch: u32 = row.get(9).unwrap_or(0);

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package
        };

        Ok(types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            epoch,
            revision,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            sub_package: row.get(7)?,
            maintainer: types::Maintainer {
                name: String::new(),
                email: String::new(),
                website: None
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

/// Searches for files matching a search term.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn search_files(
    registry_handle: &str,
    term: &str
) -> Result<Vec<(types::Package, String)>> {
    let conn = open_connection(registry_handle)?;
    let like_query = format!("%{term}%");

    let has_fts = conn
        .prepare("SELECT 1 FROM package_files_fts LIMIT 0")
        .is_ok();

    macro_rules! map_file_row {
        ($row:expr) => {{
            let tags_raw: String = $row.get(5)?;
            let tags: Vec<String> =
                serde_json::from_str(&tags_raw).unwrap_or_default();
            let type_raw: String = $row.get(4)?;
            let revision: String =
                $row.get(9).unwrap_or_else(|_| "1".to_string());
            let epoch: u32 = $row.get(10).unwrap_or(0);
            let package_type = match type_raw.as_str() {
                "collection" => types::PackageType::Collection,
                "app" => types::PackageType::App,
                "extension" => types::PackageType::Extension,
                _ => types::PackageType::Package
            };
            let pkg = types::Package {
                name: $row.get(0)?,
                repo: $row.get(1)?,
                version: $row.get(2)?,
                epoch,
                revision,
                description: $row.get(3)?,
                package_type,
                tags,
                license: $row.get(6)?,
                sub_package: $row.get(7)?,
                maintainer: types::Maintainer {
                    name: String::new(),
                    email: String::new(),
                    website: None
                },
                ..Default::default()
            };
            let path: String = $row.get(8)?;
            Ok::<_, rusqlite::Error>((pkg, path))
        }};
    }

    if has_fts {
        let search_query = term
            .replace('/', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("* ");
        let mut stmt = conn.prepare(
            "SELECT p.name, p.repo, p.version, p.description, p.package_type, \
             p.tags, p.license, p.sub_package, pf.path, p.revision, p.epoch
             FROM packages p
             JOIN package_files pf ON p.id = pf.package_id
             WHERE pf.id IN (SELECT rowid FROM package_files_fts WHERE \
             package_files_fts MATCH ?1)
             OR pf.path LIKE ?2"
        )?;
        let rows = stmt
            .query_map(params![search_query, like_query], |row| {
                map_file_row!(row)
            })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    } else {
        let mut stmt = conn.prepare(
            "SELECT p.name, p.repo, p.version, p.description, p.package_type, \
             p.tags, p.license, p.sub_package, pf.path, p.revision, p.epoch
             FROM packages p
             JOIN package_files pf ON p.id = pf.package_id
             WHERE pf.path LIKE ?1"
        )?;
        let rows =
            stmt.query_map(params![like_query], |row| map_file_row!(row))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

/// Lists all packages in the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn list_all_packages(registry_handle: &str) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, \
         license, sub_package, scope, registry, reason, revision, epoch FROM \
         packages ORDER BY name"
    )?;

    let rows = stmt.query_map([], |row| {
        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;

        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package
        };

        let sub_package: Option<String> = row.get(7)?;
        let scope_raw: Option<String> = row.get(8)?;
        let registry: Option<String> = row.get(9)?;
        let reason_raw: Option<String> = row.get(10)?;
        let revision: String = row.get(11).unwrap_or_else(|_| "1".to_string());
        let epoch: u32 = row.get(12).unwrap_or(0);

        let scope = match scope_raw.as_deref() {
            Some("system") => types::Scope::System,
            Some("project") => types::Scope::Project,
            _ => types::Scope::User
        };

        let reason = reason_raw.map(|r| {
            if r == "direct" {
                types::InstallReason::Direct
            } else if let Some(parent) = r.strip_prefix("dependency:") {
                types::InstallReason::Dependency {
                    parent: parent.to_string()
                }
            } else {
                types::InstallReason::Direct
            }
        });

        let pkg = types::Package {
            name: row.get(0)?,
            repo: row.get(1)?,
            version: row.get(2)?,
            epoch,
            revision,
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
                website: None
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

/// Retrieves all versions of a specific package from the database.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_all_versions(
    registry_handle: &str,
    name: &str,
    repo: &str
) -> Result<Vec<String>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT version FROM packages WHERE name = ?1 AND repo = ?2"
    )?;
    let rows = stmt.query_map(params![name, repo], |row| row.get(0))?;
    let mut versions = Vec::new();
    for v in rows.flatten() {
        versions.push(v);
    }
    Ok(versions)
}

/// Retrieves the dependencies of a specific package version.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_package_dependencies(
    registry_handle: &str,
    name: &str,
    version: &str,
    sub_package: Option<&str>,
    repo: &str
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

/// Finds all installed packages that depend on a specific package.
///
/// # Errors
///
/// Returns an error if the database connection cannot be opened or the query
/// fails.
pub fn get_dependents(
    registry_handle: &str,
    target_package_name: &str,
    target_sub_package: Option<&str>
) -> Result<Vec<types::Package>> {
    let conn = open_connection(registry_handle)?;
    let mut stmt = conn.prepare(
        "SELECT name, repo, version, description, package_type, tags, \
         license, sub_package, scope, registry, reason, revision, epoch, \
         dependencies FROM packages"
    )?;

    let rows = stmt.query_map([], |row| {
        let dependencies_raw: Option<String> = row.get(13)?;
        let name: String = row.get(0)?;

        let mut depends_on_target = false;
        if let Some(deps_json) = dependencies_raw
            && deps_json.contains(target_package_name)
        {
            // Heuristic check first for performance
            if let Ok(deps) =
                serde_json::from_str::<types::Dependencies>(&deps_json)
            {
                let resolved = deps.resolve(
                    &[],
                    &[],
                    row.get::<_, Option<String>>(7)?.as_deref(),
                    true,
                    None
                );
                let all_deps = [
                    resolved.runtime,
                    resolved.test,
                    resolved
                        .build
                        .into_iter()
                        .flat_map(|b| b.packages)
                        .collect()
                ]
                .concat();

                for dep_str in all_deps {
                    if let Ok(dep) = zoi_deps::parse_dependency_string(&dep_str)
                        && dep.manager == "zoi"
                        && let Ok(req) =
                            zoi_resolver::resolve::parse_source_string(
                                dep.package
                            )
                        && req.name == target_package_name
                        && req.sub_package.as_deref() == target_sub_package
                    {
                        depends_on_target = true;
                        break;
                    }
                }
            }
        }

        if !depends_on_target {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        let tags_raw: String = row.get(5)?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_raw).unwrap_or_default();
        let type_raw: String = row.get(4)?;
        let package_type = match type_raw.as_str() {
            "collection" => types::PackageType::Collection,
            "app" => types::PackageType::App,
            "extension" => types::PackageType::Extension,
            _ => types::PackageType::Package
        };

        let scope_raw: Option<String> = row.get(8)?;
        let scope = match scope_raw.as_deref() {
            Some("system") => types::Scope::System,
            Some("project") => types::Scope::Project,
            _ => types::Scope::User
        };

        Ok(types::Package {
            name,
            repo: row.get(1)?,
            version: row.get(2)?,
            description: row.get(3)?,
            package_type,
            tags,
            license: row.get(6)?,
            sub_package: row.get(7)?,
            scope,
            registry_handle: row.get(9)?,
            revision: row.get(11).unwrap_or_else(|_| "1".to_string()),
            epoch: row.get(12).unwrap_or(0),
            ..Default::default()
        })
    })?;

    let mut pkgs = Vec::new();
    for pkg in rows.flatten() {
        pkgs.push(pkg);
    }
    Ok(pkgs)
}
