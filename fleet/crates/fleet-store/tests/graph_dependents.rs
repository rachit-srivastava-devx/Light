//! `dependents()`: recursive closure bounded by depth; `depth == 0` rejected.

use fleet_store::graph::{EdgeRecord, GraphError, ProjectRecord, ReindexBatch, SymbolRecord};
use fleet_store::GraphStore;

fn sym(id: &str) -> SymbolRecord {
    SymbolRecord { symbol_id: id.into(), path: "a.rs".into(), name: id.into(), arity: 0, kind: "fn".into(), line: 1 }
}

fn seeded() -> (tempfile::TempDir, GraphStore, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = GraphStore::open(&dir.path().join("graph.sqlite3")).unwrap();
    // A 4-hop call chain: d3 -> d2 -> d1 -> target -> callee.
    store
        .replace_project(ReindexBatch {
            project: ProjectRecord {
                project_id: "proj-1".into(),
                root_path: "/repo".into(),
                tree_digest: "d".into(),
                indexed_commit: "c".into(),
                indexed_file_count: 1,
                floor: 1,
                files_skipped: 0,
                languages: vec![],
            },
            files: vec![],
            symbols: vec![sym("callee"), sym("target"), sym("d1"), sym("d2"), sym("d3")],
            edges: vec![
                EdgeRecord { caller_id: "d3".into(), callee_id: "d2".into() },
                EdgeRecord { caller_id: "d2".into(), callee_id: "d1".into() },
                EdgeRecord { caller_id: "d1".into(), callee_id: "target".into() },
                EdgeRecord { caller_id: "target".into(), callee_id: "callee".into() },
            ],
            aliases: vec![],
        })
        .unwrap();
    (dir, store, "proj-1".into())
}

#[test]
fn graph_dependents_rejects_zero_depth() {
    let (_dir, store, project_id) = seeded();
    let err = store.dependents(&project_id, &["target".to_string()], 0).unwrap_err();
    assert!(matches!(err, GraphError::BadDepth(0)));
}

#[test]
fn recursive_closure_stops_at_depth() {
    let (_dir, store, project_id) = seeded();
    let hits = store.dependents(&project_id, &["target".to_string()], 2).unwrap();
    let mut ids: Vec<&str> = hits.iter().map(|h| h.symbol_id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["d1", "d2"]);
    let depth_of = |id: &str| hits.iter().find(|h| h.symbol_id == id).unwrap().depth;
    assert_eq!(depth_of("d1"), 1);
    assert_eq!(depth_of("d2"), 2);
}
