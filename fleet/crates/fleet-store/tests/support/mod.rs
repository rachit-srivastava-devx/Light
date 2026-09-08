//! Shared test helpers. `dead_code` is allowed because each integration-test binary that includes
//! this module uses only the subset of helpers it needs.
#![allow(dead_code)]

use fleet_store::graph::{ProjectRecord, SymbolRecord};
use fleet_store::ledger::LedgerPaths;
use fleet_store::memory::{MemoryRow, Severity};
use fleet_store::Ledger;
use fleet_types::ReceiptEvent;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn paths(dir: &tempfile::TempDir) -> LedgerPaths {
    LedgerPaths { chain: dir.path().join("chain.jsonl"), lock: dir.path().join("chain.lock") }
}

pub fn lines_of(path: &Path) -> Vec<Value> {
    fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

pub fn write_lines(path: &Path, rows: &[Value]) {
    let text = rows.iter().map(|r| serde_json::to_string(r).unwrap()).collect::<Vec<_>>().join("\n");
    fs::write(path, text + "\n").unwrap();
}

pub fn seeded(dir: &tempfile::TempDir, n: usize) -> LedgerPaths {
    let p = paths(dir);
    let ledger = Ledger::open(LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });
    for i in 0..n {
        ledger.append(ReceiptEvent::RunStart, json!({ "i": i }), "a".into(), None, None).unwrap();
    }
    p
}

pub fn vec0_path() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var("FLEET_STORE_TEST_VEC0").ok()?);
    path.exists().then_some(path)
}

pub fn mem_row(id: &str, title: &str, body: &str) -> MemoryRow {
    MemoryRow {
        id: id.into(),
        title: title.into(),
        body: body.into(),
        source: "test".into(),
        keywords: vec![],
        evidence: String::new(),
        severity: Severity::Important,
    }
}

pub fn project(root: &str, digest: &str) -> ProjectRecord {
    ProjectRecord {
        project_id: "proj-1".into(),
        root_path: root.into(),
        tree_digest: digest.into(),
        indexed_commit: "abc123".into(),
        indexed_file_count: 1,
        floor: 1,
        files_skipped: 0,
        languages: vec!["rust".into()],
    }
}

pub fn symbol(id: &str) -> SymbolRecord {
    SymbolRecord { symbol_id: id.into(), path: "a.rs".into(), name: "f".into(), arity: 0, kind: "fn".into(), line: 1 }
}
