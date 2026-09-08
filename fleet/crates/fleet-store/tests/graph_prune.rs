//! `GraphStore::usage()`/`GraphStore::prune()`: whole-project limit enforcement, no-op on empty.

use fleet_store::graph::ReindexBatch;
use fleet_store::{GraphStore, RetentionPolicy};

mod support;
use support::{project, symbol};

#[test]
fn graph_prune_is_a_noop_on_an_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = GraphStore::open(&dir.path().join("graph.sqlite3")).unwrap();
    let report = store.prune(&RetentionPolicy { max_rows: Some(0), ..Default::default() }).unwrap();
    assert_eq!(report.rows_removed, 0);
}

#[test]
fn graph_prune_respects_max_rows_by_whole_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = GraphStore::open(&dir.path().join("graph.sqlite3")).unwrap();
    for i in 0..3 {
        let mut p = project(&format!("/root{i}"), &format!("digest{i}"));
        p.project_id = format!("proj-{i}");
        let batch =
            ReindexBatch { project: p, files: vec![], symbols: vec![symbol("s1")], edges: vec![], aliases: vec![] };
        store.replace_project(batch).unwrap();
    }
    assert_eq!(store.usage().unwrap().row_count, 3);
    let report = store.prune(&RetentionPolicy { max_rows: Some(1), ..Default::default() }).unwrap();
    assert_eq!(report.rows_removed, 2);
    assert_eq!(store.usage().unwrap().row_count, 1);
}
