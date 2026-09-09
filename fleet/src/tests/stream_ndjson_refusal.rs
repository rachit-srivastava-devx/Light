//! A real stage failure (`Merge` refusing an empty git stage) must leave a `refusal` receipt in
//! the NDJSON trail, not just the process's own non-zero exit -- the durable log is the point.

mod support;
use std::fs;
use support::{pipeline_probe, scratch_repo};

#[test]
fn a_failing_stage_leaves_a_refusal_row_in_the_stream() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let stream_dir = tempfile::tempdir().unwrap();
    // Committed, nothing staged: `check_stage_nonempty` refuses `Merge`.
    scratch_repo(repo.path());

    let out = pipeline_probe(state_dir.path(), repo.path(), "refusal-task", Some(stream_dir.path()));
    assert!(!out.status.success(), "probe should fail when the stage is empty");

    let events_path = stream_dir.path().join("events.ndjson");
    let rows: Vec<serde_json::Value> = fs::read_to_string(&events_path)
        .unwrap_or_else(|e| panic!("NDJSON not written even though the run refused: {e}"))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let refusal = rows.iter().find(|r| r["event"] == "refusal").expect("a refusal row must exist");
    assert_eq!(refusal["body"]["stage"], "merge");
}
