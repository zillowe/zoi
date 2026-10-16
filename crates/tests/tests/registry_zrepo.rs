//! Integration tests for the registry snapshot (`.zrepo`) transport and its
//! incremental zsync transfers.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use tempfile::tempdir;
use zoi_core::types::RepoConfig;
use zoi_sync::zrepo;

mod common;

/// Fills `root` with a registry tree holding `filler` package definitions and
/// one `ripgrep` definition whose body the tests vary between revisions.
fn make_registry_tree(
    root: &Path,
    pkg_lua_body: &str,
    filler: usize
) -> Result<()> {
    fs::create_dir_all(root.join("main/ripgrep"))?;
    fs::write(
        root.join("repo.yaml"),
        "version: \"2\"\nname: testreg\ndescription: A test registry\ngit: \
         []\nrepos: []\nzrepo:\n  - type: main\n    url: \
         http://placeholder/testreg.zrepo\n"
    )?;
    fs::write(
        root.join("packages.json"),
        "{\"version\":\"2\",\"packages\":{}}"
    )?;
    fs::write(root.join("main/ripgrep/ripgrep.pkg.lua"), pkg_lua_body)?;

    for index in 0..filler {
        let dir = root.join(format!("main/tool-{index}"));
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join(format!("tool-{index}.pkg.lua")),
            format!(
                "metadata({{name=\"tool-{index}\",repo=\"main\",version=\"1.0.\
                 {index}\"}})\n"
            )
        )?;
    }
    Ok(())
}

/// Serves `root` over HTTP on a background thread, counting the body bytes it
/// hands out so a test can tell an incremental transfer from a full one.
///
/// Supports `Range`, which is the request the snapshot transfer is built on.
fn serve(root: PathBuf) -> (String, Arc<AtomicU64>) {
    let served = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&served);

    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port");
    let address = listener.local_addr().expect("read the bound address");

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            handle_request(&mut stream, &root, &counter);
        }
    });

    (format!("http://{address}"), served)
}

/// Answers one HTTP request, honouring `Range` when one is asked for.
fn handle_request(
    stream: &mut std::net::TcpStream,
    root: &Path,
    counter: &AtomicU64
) {
    use std::io::{BufRead, BufReader, Write};

    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let mut range: Option<(usize, usize)> = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        let lower = header.to_lowercase();
        if let Some(at) = lower.find("range: bytes=") {
            let spec = header[at + 13..]
                .split_whitespace()
                .next()
                .unwrap_or("0-")
                .to_string();
            let mut parts = spec.split('-');
            let start = parts
                .next()
                .and_then(|n| n.trim().parse::<usize>().ok())
                .unwrap_or(0);
            let end = parts
                .next()
                .and_then(|n| n.trim().parse::<usize>().ok())
                .unwrap_or(usize::MAX);
            range = Some((start, end));
        }
    }

    let name = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .trim_start_matches('/')
        .to_string();
    let path = root.join(&name);

    let Ok(body) = fs::read(&path) else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    };

    let empty: &[u8] = &[];
    let (status, slice) = match range {
        // A range that runs past the end answers empty rather than panicking,
        // which is what a real server does and what the client has to survive.
        Some((start, end)) => {
            let end = end.min(body.len().saturating_sub(1));
            let slice = body.get(start..=end).unwrap_or(empty);
            ("206 Partial Content", slice)
        }
        None => ("200 OK", body.as_slice())
    };

    counter.fetch_add(slice.len() as u64, Ordering::Relaxed);
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: \
         bytes\r\n\r\n",
        slice.len()
    );
    if stream.write_all(head.as_bytes()).is_ok() {
        let mut remaining = slice;
        // Streamed so a large snapshot does not need a second copy.
        while !remaining.is_empty() {
            let chunk = remaining.len().min(64 * 1024);
            let Some(head) = remaining.get(..chunk) else {
                return;
            };
            if stream.write_all(head).is_err() {
                return;
            }
            remaining = remaining.get(chunk..).unwrap_or_default();
        }
    }
    let _ = stream.flush();
}

/// A `.zrepo` link pointing at a snapshot served by `base`.
fn served_link(base: &str) -> zoi_core::types::ZrepoLink {
    zoi_core::types::ZrepoLink {
        link_type: "main".to_string(),
        url: format!("{base}/testreg.zrepo"),
        control: None,
        pgp: None,
        hash: None,
        size: None
    }
}

#[test]
fn zrepo_sync_reuses_an_older_snapshot_and_transfers_only_what_changed() {
    let tmp = tempdir().expect("tempdir");
    let serve_root = tmp.path().join("serve");
    let tree = tmp.path().join("registry");
    fs::create_dir_all(&serve_root).expect("serve dir");

    // A registry with real bulk, so an incremental transfer has something to
    // save rather than trivially matching everything.
    make_registry_tree(
        &tree,
        "metadata({name=\"ripgrep\",version=\"14.1.0\"})",
        300
    )
    .expect("first tree");
    zrepo::create_zrepo(&tree, &serve_root.join("testreg.zrepo"), None)
        .expect("first snapshot");

    let (base, served) = serve(serve_root.clone());
    let link = served_link(&base);

    let db_root = tmp.path().join("db");
    let installed = tmp.path().join("registry-live");

    let hash = zrepo::sync_zrepo(
        &[&link],
        &db_root,
        "testreg",
        &installed,
        &[],
        None,
        false
    )
    .expect("first sync");
    assert!(!hash.is_empty(), "a snapshot hash should be recorded");
    assert!(
        installed.join("repo.yaml").exists(),
        "the registry tree should have been installed"
    );
    assert!(
        zrepo::base_artifact_path(&db_root, "testreg").exists(),
        "the snapshot should be cached as a seed for the next sync"
    );

    // Publish a new revision that changes exactly one package definition.
    make_registry_tree(
        &tree,
        "metadata({name=\"ripgrep\",version=\"14.1.1\"})",
        300
    )
    .expect("second tree");
    zrepo::create_zrepo(&tree, &serve_root.join("testreg.zrepo"), None)
        .expect("second snapshot");

    let snapshot_size = fs::metadata(serve_root.join("testreg.zrepo"))
        .expect("stat")
        .len();

    served.store(0, Ordering::Relaxed);
    zrepo::sync_zrepo(
        &[&link],
        &db_root,
        "testreg",
        &installed,
        &[],
        None,
        false
    )
    .expect("second sync");

    let definition =
        fs::read_to_string(installed.join("main/ripgrep/ripgrep.pkg.lua"))
            .expect("the new definition should be installed");
    assert!(
        definition.contains("14.1.1"),
        "the new revision should be installed, got {definition}"
    );

    // The point of the exercise: the second sync must not have re-downloaded
    // the registry. Serving less than a tenth of the snapshot is a wide margin
    // for a one-file change out of hundreds.
    let transferred = served.load(Ordering::Relaxed);
    assert!(
        transferred < snapshot_size / 10,
        "expected an incremental transfer, but {transferred} of \
         {snapshot_size} bytes were served"
    );
}

#[test]
fn zrepo_sync_falls_back_to_a_full_download_without_a_control_file() {
    let tmp = tempdir().expect("tempdir");
    let serve_root = tmp.path().join("serve");
    let tree = tmp.path().join("registry");
    fs::create_dir_all(&serve_root).expect("serve dir");

    make_registry_tree(
        &tree,
        "metadata({name=\"ripgrep\",version=\"14.1.0\"})",
        20
    )
    .expect("tree");
    zrepo::create_zrepo(&tree, &serve_root.join("testreg.zrepo"), None)
        .expect("snapshot");
    // Remove the control file: a registry that has not adopted zsync yet.
    fs::remove_file(serve_root.join("testreg.zrepo.zsync"))
        .expect("remove control file");

    let (base, _served) = serve(serve_root);
    let link = served_link(&base);
    let installed = tmp.path().join("registry-live");

    // Must succeed anyway: the control file is an optimisation, not a
    // requirement.
    zrepo::sync_zrepo(
        &[&link],
        &tmp.path().join("db"),
        "testreg",
        &installed,
        &[],
        None,
        false
    )
    .expect("sync should fall back to a full download");

    assert!(
        installed.join("repo.yaml").exists(),
        "the registry tree should still have been installed"
    );
}

fn repo_config_with_zrepo_links(yaml: &str) -> RepoConfig {
    serde_yaml::from_str(yaml).expect("repo.yaml should parse")
}

#[test]
fn zrepo_candidates_prefer_main_then_fall_back_to_mirrors() {
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: mirror
    url: https://eu.example.com/testreg.zrepo
  - type: mirror
    url: https://us.example.com/testreg.zrepo
  - type: main
    url: https://main.example.com/testreg.zrepo
"#
    );

    let candidates = zrepo::candidate_links(&repo_config);
    let urls: Vec<&str> =
        candidates.iter().map(|link| link.url.as_str()).collect();

    assert_eq!(
        urls,
        vec![
            "https://main.example.com/testreg.zrepo",
            "https://eu.example.com/testreg.zrepo",
            "https://us.example.com/testreg.zrepo",
        ],
        "the main tier must be tried first, mirrors only as a fallback"
    );
}

#[test]
fn zrepo_candidates_keep_a_registry_without_a_main_tier_usable() {
    // A registry that labels every entry `mirror` still syncs; the entries are
    // simply tried in the order they were declared.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: mirror
    url: https://a.example.com/testreg.zrepo
  - type: mirror
    url: https://b.example.com/testreg.zrepo
"#
    );

    let urls: Vec<String> = zrepo::candidate_links(&repo_config)
        .iter()
        .map(|link| link.url.clone())
        .collect();

    assert_eq!(
        urls,
        vec![
            "https://a.example.com/testreg.zrepo",
            "https://b.example.com/testreg.zrepo"
        ]
    );
}

#[test]
fn zrepo_control_url_defaults_to_the_conventional_suffix() {
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: main
    url: https://example.com/testreg.zrepo
  - type: mirror
    url: https://mirror.example.com/testreg.zrepo
    control: https://index.example.com/testreg.zsync
"#
    );

    let candidates = zrepo::candidate_links(&repo_config);
    let main = candidates.first().expect("main link should be present");
    let mirror = candidates.get(1).expect("mirror link should be present");

    assert_eq!(
        zrepo::control_url(main),
        "https://example.com/testreg.zrepo.zsync",
        "an entry without an explicit control resolves to <url>.zsync"
    );
    assert_eq!(
        zrepo::control_url(mirror),
        "https://index.example.com/testreg.zsync",
        "an explicit control URL must win"
    );
}

#[test]
fn zrepo_base_snapshot_lives_next_to_the_registry_index() {
    let db_root = PathBuf::from("/tmp/zoi-db");
    assert_eq!(
        zrepo::base_artifact_path(&db_root, "testreg"),
        db_root.join("testreg.zrepo")
    );
}

#[test]
fn git_only_repo_yaml_declares_no_zrepo_links() {
    // The existing registry format must keep working: a `repo.yaml` with only
    // `git:` links yields no `.zrepo` candidates, so sync falls through to the
    // Git transport.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: zoidberg
description: Official Zoi packages repository
git:
  - type: official
    url: https://gitlab.com/zillowe/zillwen/zusty/zoidberg
repos: []
"#
    );

    assert!(
        zrepo::candidate_links(&repo_config).is_empty(),
        "a git-only repo.yaml must not select the snapshot transport"
    );
}

#[test]
fn a_snapshot_only_registry_needs_no_git_section() {
    // A registry published as a snapshot instead of a Git repository has
    // nothing to put under `git:`, so an absent section must parse rather than
    // failing the whole manifest.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: snapshotonly
description: A registry with no Git repository
zrepo:
  - type: main
    url: https://example.com/snapshotonly.zrepo
"#
    );

    assert!(
        repo_config.git.is_empty(),
        "an omitted git section should default to empty"
    );
    let repos = &repo_config.repos;
    assert_eq!(repos.len(), 0, "expected no repo entries, got {repos:?}");
    assert_eq!(repo_config.name, "snapshotonly");
    assert_eq!(zrepo::candidate_links(&repo_config).len(), 1);
}

#[test]
fn a_registry_may_omit_every_optional_section() {
    // Only `name` and `description` are genuinely required, so a minimal
    // manifest must remain valid.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: minimal
description: The smallest valid registry
zrepo:
  - type: main
    url: https://example.com/minimal.zrepo
"#
    );

    let pkg_links = &repo_config.pkg;
    assert_eq!(
        pkg_links.len(),
        0,
        "expected no pkg links, got {pkg_links:?}"
    );
    let delta_links = &repo_config.delta;
    assert_eq!(
        delta_links.len(),
        0,
        "expected no delta links, got {delta_links:?}"
    );
    let pgp_keys = &repo_config.pgp;
    assert_eq!(pgp_keys.len(), 0, "expected no pgp keys, got {pgp_keys:?}");
    assert!(repo_config.db.is_none());
    assert!(repo_config.advisory_prefix.is_none());
}

#[test]
fn repo_yaml_url_detection_only_matches_yaml_documents() {
    // A plain repository URL must not be mistaken for a manifest, otherwise
    // it would be requested verbatim and return HTML.
    assert!(!zoi_sync::is_repo_yaml_url("https://gitlab.com/org/repo"));
    assert!(!zoi_sync::is_repo_yaml_url(
        "https://github.com/org/repo.git"
    ));
    assert!(!zoi_sync::is_repo_yaml_url("/home/user/registry"));
    assert!(zoi_sync::is_repo_yaml_url("https://example.com/repo.yaml"));
    assert!(zoi_sync::is_repo_yaml_url("https://example.com/repo.yml"));
    assert!(zoi_sync::is_repo_yaml_url(
        "https://example.com/registry/repo.yaml?ref=main"
    ));
}

#[test]
fn a_registry_added_by_url_records_its_resolved_handle() -> Result<()> {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    // What `zoi registry add <url>` persists after resolving repo.yaml: the
    // real handle and metadata, with the manifest URL kept as the identity.
    config::add_resolved_registry(Registry {
        handle: "testreg".to_string(),
        url: "https://example.com/repo.yaml".to_string(),
        name: Some("Testreg".to_string()),
        description: Some("A test registry".to_string()),
        advisory_prefix: Some("TEST".to_string()),
        authorities: None
    })?;

    let cfg = config::read_config()?;
    let added = cfg
        .added_registries
        .iter()
        .find(|r| r.handle == "testreg")
        .expect("the resolved handle should have been recorded");

    assert_eq!(added.name.as_deref(), Some("Testreg"));
    assert_eq!(added.description.as_deref(), Some("A test registry"));
    assert_eq!(added.advisory_prefix.as_deref(), Some("TEST"));
    assert_eq!(added.url, "https://example.com/repo.yaml");

    Ok(())
}

#[test]
fn a_registry_with_no_name_cannot_be_added() {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    // A manifest without a `name` has no usable handle, so it must be
    // rejected rather than stored as an unidentifiable entry.
    let err = config::add_resolved_registry(Registry {
        handle: String::new(),
        url: "https://example.com/repo.yaml".to_string(),
        name: None,
        description: None,
        advisory_prefix: None,
        authorities: None
    })
    .expect_err("a registry without a name must be rejected");

    assert!(
        err.to_string().contains("declares no 'name'"),
        "unexpected error: {err}"
    );
}

#[test]
fn a_registry_cannot_be_added_twice() -> Result<()> {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    let registry = Registry {
        handle: "testreg".to_string(),
        url: "https://example.com/repo.yaml".to_string(),
        name: Some("Testreg".to_string()),
        description: None,
        advisory_prefix: None,
        authorities: None
    };
    config::add_resolved_registry(registry.clone())?;

    // The same registry under a different URL spelling is still the same
    // handle, and must not be added twice.
    let mut other = registry.clone();
    other.url = "https://example.com/./repo.yaml".to_string();
    let err = config::add_resolved_registry(other)
        .expect_err("a duplicate registry must be rejected");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    // An identical URL is rejected too.
    let err = config::add_resolved_registry(registry)
        .expect_err("a duplicate registry must be rejected");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    Ok(())
}
