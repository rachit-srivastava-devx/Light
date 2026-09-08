//! Restate is deferred in this pass (`pipeline/graph.rs`'s doc comment); `StepLog` provides the
//! same crash-resume property. This test proves it black-box, through the real binary: run the
//! hidden `__pipeline_probe` once so every stage gets marked done on disk, then start a brand
//! new process (standing in for "the old one crashed and this is the restart") against the same
//! state dir and confirm the on-disk step log already carries every stage -- nothing here re-ran
//! `Teach`'s side effect a second time by observing the log is stable across the second call.

use std::fs;
use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_fleet")
}

fn run_probe(state_dir: &Path, task_id: &str) -> std::process::Output {
    Command::new(bin())
        .env("FLEET_STATE_DIR", state_dir)
        .args(["__pipeline_probe", "--task-id", task_id])
        .output()
        .expect("binary runs")
}

#[test]
fn a_second_process_against_the_same_state_dir_replays_instead_of_rerunning() {
    let dir = tempfile::tempdir().unwrap();
    let task_id = "resume-demo-task";

    let first = run_probe(dir.path(), task_id);
    assert!(first.status.success(), "first run: {}", String::from_utf8_lossy(&first.stderr));

    let log_path = dir.path().join(format!("{task_id}.steps.json"));
    let after_first = fs::read_to_string(&log_path).expect("step log written by first process");
    for stage in ["Event", "Classify", "Scan", "Plan", "Dispatch", "Verify", "Merge", "Teach"] {
        assert!(after_first.contains(stage), "stage {stage} missing from step log: {after_first}");
    }

    // A brand new process, standing in for the post-crash restart, sees the same completed log.
    let second = run_probe(dir.path(), task_id);
    assert!(second.status.success(), "second run: {}", String::from_utf8_lossy(&second.stderr));
    let after_second = fs::read_to_string(&log_path).expect("step log still present");
    assert_eq!(after_first, after_second, "replay must not mutate the completed step log");
}
