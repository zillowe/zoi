use std::fs;
use tempfile::tempdir;
use zoi_system::generation::GenerationManager;

#[test]
fn test_generation_management() {
    let dir = tempdir().unwrap();
    let gen_root = dir.path().join("generations");
    fs::create_dir_all(&gen_root).unwrap();

    let manager = GenerationManager { root: gen_root };

    // Initial state
    let gens = manager.list_generations().unwrap();
    assert_eq!(gens.len(), 0);

    // Create first generation
    let id1 = manager
        .create_generation(vec!["@core/bash".to_string()])
        .unwrap();
    assert_eq!(id1, 1);

    let gens = manager.list_generations().unwrap();
    assert_eq!(gens.len(), 1);
    assert_eq!(gens[0].id, 1);
    assert_eq!(gens[0].packages[0], "@core/bash");

    // Create second generation
    let id2 = manager
        .create_generation(vec!["@core/bash".to_string(), "@main/vim".to_string()])
        .unwrap();
    assert_eq!(id2, 2);

    let gens = manager.list_generations().unwrap();
    assert_eq!(gens.len(), 2);
    assert_eq!(gens[1].id, 2);
}
