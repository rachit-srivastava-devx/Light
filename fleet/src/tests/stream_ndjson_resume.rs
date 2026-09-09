//! Resume proof: two pipeline runs against the SAME `state_dir` (so the ledger accumulates both
//! runs' receipts) and the SAME `FLEET_STREAM_DIR` must leave the first run's rows written
//! exactly once -- the second flush's `FileCursorStore` cursor must skip everything it already
//! delivered, matching `crates/fleet-stream/tests/run_cursor_resume_real_file.rs`'s shape but
//! driven through the real binary end to end instead of `run_sink` directly.

mod support;
use std::collections::HashSet;
use std::fs;
use support::{pipeline_probe, scratch_repo_staged};

fn seqs(path: &std::path::Path) -> Vec<u64> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["seq"].as_u64().unwrap())
        .collect()
}

#[test]
fn a_second_run_appends_without_replaying_the_first() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let stream_dir = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());
    let events_path = stream_dir.path().join("events.ndjson");

    let first = pipeline_probe(state_dir.path(), repo.path(), "resume-task-1", Some(stream_dir.path()));
    assert!(first.status.success(), "run 1 failed: {}", String::from_utf8_lossy(&first.stderr));
    let after_first = seqs(&events_path);
    assert!(!after_first.is_empty());

    let second = pipeline_probe(state_dir.path(), repo.path(), "resume-task-2", Some(stream_dir.path()));
    assert!(second.status.success(), "run 2 failed: {}", String::from_utf8_lossy(&second.stderr));
    let after_second = seqs(&events_path);

    assert!(after_second.len() > after_first.len(), "second run must append new rows");
    assert_eq!(&after_second[..after_first.len()], &after_first[..], "first run's rows must be untouched");
    let unique: HashSet<u64> = after_second.iter().copied().collect();
    assert_eq!(unique.len(), after_second.len(), "no seq may be duplicated across the two flushes");
}
