//! `fleet run --json` must not be poorer than the human stderr path. It used to emit exactly
//! `{task, final_stage, classification}` -- no stage outcomes, no gate verdicts, no refusal
//! reason -- so the machine-readable path was strictly LESS informative than the one printed for
//! a human. Also the end-to-end proof of `.fleet/gates.toml`: every gate here runs a configured
//! `bash` command, never `cargo test`, which is what makes this repo (not a cargo workspace at
//! all) gateable in the first place.

mod support;
use serde_json::Value;
use support::gates::{gates_toml, run_json};
use support::scratch_repo_staged;

#[test]
fn run_json_carries_per_stage_outcomes_gate_verdicts_and_the_classification() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());
    gates_toml(repo.path());

    let v: Value = run_json(state_dir.path(), repo.path(), "json-parity-task");

    // Pre-existing keys still there -- nothing that parses this today breaks.
    assert_eq!(v["task"], "json-parity-task");
    assert!(v["final_stage"].is_string(), "final_stage missing: {v}");
    assert!(v["classification"].is_object(), "classify PASSed, so it must be reported: {v}");

    let stages = v["stages"].as_array().expect("stages array").clone();
    let names: Vec<&str> = stages.iter().map(|s| s["stage"].as_str().unwrap()).collect();
    assert_eq!(names, ["Event", "Classify", "Scan", "Plan", "Dispatch", "Verify", "Merge", "Teach"]);
    for s in &stages {
        assert_eq!(s["outcome"], "pass", "stage {} did not pass: {s}", s["stage"]);
        assert!(s["elapsed_ms"].is_u64(), "a stage that ran must publish a duration: {s}");
    }

    let gates = v["gates"].as_array().expect("gates array").clone();
    let ids: Vec<&str> = gates.iter().map(|g| g["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 8, "every registry gate must be reported, got {ids:?}");
    assert!(ids.contains(&"unit tests"), "got {ids:?}");
    let unit = gates.iter().find(|g| g["id"] == "unit tests").unwrap();
    assert_eq!(unit["verdict"], "pass", "the configured npm-style command must run: {unit}");
    assert_eq!(unit["checked"], 3, "the gate's published denominator must survive: {unit}");
    assert_eq!(unit["total"], 3);
    assert_eq!(unit["env_fault"], false);
    assert!(v["refusal"].is_null(), "a clean run has no refusal: {v}");
}

/// The failing shape: the refusal reason and the failing gate must both be machine-readable.
#[test]
fn a_refused_run_names_the_failing_gate_and_the_refusal_in_json() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());
    gates_toml(repo.path());
    // Break exactly one gate: exit 1, no denominator at all.
    let path = repo.path().join(".fleet").join("gates.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replace(
        "echo '7 detectors match the manifest (denominator: 7)'",
        "exit 1",
    );
    std::fs::write(&path, text).unwrap();

    let v: Value = run_json(state_dir.path(), repo.path(), "json-refusal-task");

    assert_eq!(v["final_stage"], "Verify");
    let refusal = v["refusal"].as_str().expect("a refused run must publish its reason");
    assert!(refusal.contains("detectors"), "refusal must name the gate: {refusal}");
    let gates = v["gates"].as_array().unwrap();
    let broken = gates.iter().find(|g| g["id"] == "detectors").unwrap();
    assert_eq!(broken["verdict"], "fail");
    assert_eq!(broken["detail"], "NonZeroExit(1)");
    let verify = v["stages"].as_array().unwrap().iter().find(|s| s["stage"] == "Verify").unwrap();
    assert_eq!(verify["outcome"], "fail", "{verify}");
}
