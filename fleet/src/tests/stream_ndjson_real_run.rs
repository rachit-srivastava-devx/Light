//! `FLEET_STREAM_DIR` set: a real pipeline run's ledger receipts land as NDJSON, containing the
//! real `run_start`/`run_end` records `event_stage`/`run_ledger` append -- not a fixture.

mod support;
use std::fs;
use support::{pipeline_probe, scratch_repo_staged};

fn events(path: &std::path::Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad NDJSON line {l:?}: {e}")))
        .collect()
}

#[test]
fn set_stream_dir_emits_real_stage_records_as_ndjson() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let stream_dir = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());

    let out = pipeline_probe(state_dir.path(), repo.path(), "stream-demo-task", Some(stream_dir.path()));
    assert!(out.status.success(), "probe failed: {}", String::from_utf8_lossy(&out.stderr));

    let events_path = stream_dir.path().join("events.ndjson");
    assert!(events_path.exists(), "NDJSON file must appear when FLEET_STREAM_DIR is set");
    let rows = events(&events_path);
    assert!(!rows.is_empty(), "NDJSON file must contain rows");

    let kinds: Vec<&str> = rows.iter().map(|r| r["event"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"run_start"), "missing run_start in {kinds:?}");
    assert!(kinds.contains(&"run_end"), "missing run_end in {kinds:?}");
    let run_start = rows.iter().find(|r| r["event"] == "run_start").unwrap();
    assert_eq!(run_start["body"]["task_id"], "stream-demo-task");
    assert!(rows.iter().all(|r| r["hash"].as_str().unwrap().starts_with("blake3:")));
}
