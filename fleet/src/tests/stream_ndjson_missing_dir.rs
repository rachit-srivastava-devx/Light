//! `FLEET_STREAM_DIR` pointing at a directory that does not exist yet must still produce NDJSON.
//! `FileSink` opens its file with `create(true)`, which creates the FILE but not its parent
//! DIRECTORY -- so `fleet run` used to mirror nothing at all, with no file, no cursor, and an
//! unattributed `os error 2` note. Drives the real binary, not `flush` directly: the property is
//! "a user who set the env var gets receipts", not "a private fn returns Ok".

mod support;
use std::fs;
use support::{pipeline_probe, scratch_repo_staged};

#[test]
fn a_stream_dir_that_does_not_exist_yet_is_created_and_written() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());

    // Two levels below an existing parent -- `create_dir_all`, not `create_dir`.
    let stream_dir = parent.path().join("nested").join("stream");
    assert!(!stream_dir.exists(), "precondition: the stream dir must not exist yet");

    let out = pipeline_probe(state_dir.path(), repo.path(), "missing-dir-task", Some(&stream_dir));
    assert!(out.status.success(), "probe failed: {}", String::from_utf8_lossy(&out.stderr));

    let events_path = stream_dir.join("events.ndjson");
    assert!(events_path.exists(), "NDJSON must appear even when FLEET_STREAM_DIR did not exist");
    let text = fs::read_to_string(&events_path).unwrap();
    let kinds: Vec<String> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("NDJSON line parses"))
        .map(|r| r["event"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(kinds.iter().any(|k| k == "run_start"), "missing run_start in {kinds:?}");
    assert!(stream_dir.join("cursors").is_dir(), "the cursor store must land there too");
}

/// The failure path must never be silent: a `FLEET_STREAM_DIR` that cannot be created (here, a
/// path whose "parent" is a regular file) names the variable and the path on stderr.
#[test]
fn an_uncreatable_stream_dir_is_reported_with_its_path() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let blocker = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());

    let file = blocker.path().join("not-a-dir");
    fs::write(&file, "i am a file").unwrap();
    let stream_dir = file.join("stream");

    let out = pipeline_probe(state_dir.path(), repo.path(), "blocked-dir-task", Some(&stream_dir));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("FLEET_STREAM_DIR="), "the note must name the env var, got: {err}");
    assert!(err.contains(&stream_dir.display().to_string()), "must name the path, got: {err}");
    // Observability must not be able to fail the run it observes.
    assert!(out.status.success(), "a stream-dir fault must not fail the pipeline: {err}");
}
