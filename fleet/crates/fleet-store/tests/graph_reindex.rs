//! `replace_project` upsert-on-matching-root_path idempotence + rollback-on-error.

use fleet_store::graph::{EdgeRecord, ReindexBatch};
use fleet_store::GraphStore;

mod support;
use support::{project, symbol};

#[test]
fn graph_replace_project_upserts_on_matching_root_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = GraphStore::open(&dir.path().join("graph.sqlite3")).unwrap();

    store
        .replace_project(ReindexBatch {
            project: project("/repo", "digest-1"),
            files: vec![],
            symbols: vec![symbol("s1")],
            edges: vec![],
            aliases: vec![],
        })
        .unwrap();
    let first_project = store.project(std::path::Path::new("/repo")).unwrap().unwrap();

    store
        .replace_project(ReindexBatch {
            project: project("/repo", "digest-2"),
            files: vec![],
            symbols: vec![symbol("s2")],
            edges: vec![],
            aliases: vec![],
        })
        .unwrap();
    let second_project = store.project(std::path::Path::new("/repo")).unwrap().unwrap();
    let symbols = store.symbols(&second_project.project_id).unwrap();

    assert_eq!(first_project.project_id, second_project.project_id);
    assert_eq!(second_project.tree_digest, "digest-2");
    assert_eq!(symbols.iter().map(|s| s.symbol_id.as_str()).collect::<Vec<_>>(), vec!["s2"]);
}

#[test]
fn a_failing_insert_rolls_back_the_whole_batch() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = GraphStore::open(&dir.path().join("graph.sqlite3")).unwrap();

    store
        .replace_project(ReindexBatch {
            project: project("/repo", "digest-1"),
            files: vec![],
            symbols: vec![symbol("s1")],
            edges: vec![],
            aliases: vec![],
        })
        .unwrap();

    // Duplicate symbol_id within the same batch violates the (project_id, symbol_id) primary key.
    let err = store.replace_project(ReindexBatch {
        project: project("/repo", "digest-2"),
        files: vec![],
        symbols: vec![symbol("s2"), symbol("s2")],
        edges: vec![EdgeRecord { caller_id: "s2".into(), callee_id: "s2".into() }],
        aliases: vec![],
    });
    assert!(err.is_err());

    let project = store.project(std::path::Path::new("/repo")).unwrap().unwrap();
    let symbols = store.symbols(&project.project_id).unwrap();
    assert_eq!(project.tree_digest, "digest-1", "the prior project row must survive an aborted batch");
    assert_eq!(symbols.iter().map(|s| s.symbol_id.as_str()).collect::<Vec<_>>(), vec!["s1"]);
}
