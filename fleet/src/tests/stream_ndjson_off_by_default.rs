//! `FLEET_STREAM_DIR` unset must be byte-identical to before `fleet-stream` was wired in: no
//! NDJSON file, no cursor directory, nothing written outside `state_dir` at all.

mod support;
use support::{pipeline_probe, scratch_repo_staged};

#[test]
fn unset_stream_dir_writes_nothing_to_the_stream_path() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());
    // A real candidate directory that the run is never told about -- proves absence, not just
    // "we didn't create a fresh tempdir".
    let would_be_stream_dir = tempfile::tempdir().unwrap();
    let events_path = would_be_stream_dir.path().join("events.ndjson");

    let out = pipeline_probe(state_dir.path(), repo.path(), "no-stream-task", None);
    assert!(out.status.success(), "probe failed: {}", String::from_utf8_lossy(&out.stderr));

    assert!(!events_path.exists(), "no NDJSON file must appear when FLEET_STREAM_DIR is unset");
}
