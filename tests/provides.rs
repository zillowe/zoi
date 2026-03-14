use rusqlite::params;
use tempfile::tempdir;

#[test]
fn test_find_provides_logic() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("local.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    conn.execute(
        "CREATE TABLE packages (
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
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE package_files (
            id INTEGER PRIMARY KEY,
            package_id INTEGER,
            path TEXT NOT NULL,
            FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO packages (name, repo, version, bins, package_type, tags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params!["git", "core", "2.40.0", "[\"git\"]", "package", "[]"],
    ).unwrap();
    let pkg_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO package_files (package_id, path) VALUES (?1, ?2)",
        params![pkg_id, "data/pkgstore/bin/git"],
    )
    .unwrap();

    let mut stmt = conn.prepare("SELECT p.name FROM packages p JOIN package_files pf ON p.id = pf.package_id WHERE pf.path LIKE ?1").unwrap();
    let mut rows = stmt.query(params!["%/git"]).unwrap();
    let name: String = rows.next().unwrap().unwrap().get(0).unwrap();
    assert_eq!(name, "git");
}
