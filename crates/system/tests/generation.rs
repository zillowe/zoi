//! Integration coverage for system generation records.
use std::fs;

use tempfile::tempdir;
use zoi_system::generation::GenerationManager;

#[test]
fn test_generation_management() {
    let dir = tempdir().expect("temp dir");
    let gen_root = dir.path().join("generations");
    fs::create_dir_all(&gen_root).expect("create store");

    let manager = GenerationManager { root: gen_root };

    // Initial state
    let gens = manager.list_generations().expect("list");
    assert_eq!(gens.len(), 0);

    // Create first generation
    let id1 = manager
        .create_generation(vec!["@core/bash".to_string()])
        .expect("create first generation");
    assert_eq!(id1, 1);

    let gens = manager.list_generations().expect("list");
    assert_eq!(gens.len(), 1);
    let first = gens.first().expect("one generation");
    assert_eq!(first.id, 1);
    assert_eq!(first.packages.first().expect("one package"), "@core/bash");

    // Create second generation
    let id2 = manager
        .create_generation(vec![
            "@core/bash".to_string(),
            "@main/vim".to_string(),
        ])
        .expect("create second generation");
    assert_eq!(id2, 2);

    let gens = manager.list_generations().expect("list");
    assert_eq!(gens.len(), 2);
    let second = gens.get(1).expect("two generations");
    assert_eq!(second.id, 2);
}
