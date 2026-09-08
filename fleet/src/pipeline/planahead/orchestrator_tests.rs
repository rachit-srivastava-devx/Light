//! Proves (a) and (b) of the plan-ahead overlap contract. (c) (crash-resume) lives in
//! `crash_resume_tests.rs` -- split out to keep each file under the 80-line gate.

use super::run_plan_ahead;
use super::test_support::{gated_build_fn, plan_fn, wait_until, EventLog};

/// (a): planning of unit N+1 must start (and, with a capacity-1 queue, be enqueued) before unit
/// N's build finishes. Proven by real event ordering, not by timing: unit "a"'s build is held
/// open on a gate until the test has already observed "plan:b" land in the shared log.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn planning_of_next_unit_starts_before_current_units_build_completes() {
    let dir = tempfile::tempdir().unwrap();
    let log = EventLog::default();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let build = gated_build_fn(log.clone(), "a", started_tx, release_rx);

    let units = vec!["a".to_string(), "b".to_string()];
    let handle = tokio::spawn(run_plan_ahead(units, dir.path().to_path_buf(), "run-a", 1, plan_fn(log.clone()), build));
    started_rx.recv_timeout(std::time::Duration::from_secs(5)).expect("build(a) must start");
    assert!(wait_until(|| log.contains("plan:b")).await, "plan(b) never ran while build(a) was in flight");
    release_tx.send(()).unwrap();
    handle.await.unwrap().unwrap();

    let events = log.snapshot();
    let plan_b = events.iter().position(|e| e == "plan:b").unwrap();
    let build_end_a = events.iter().position(|e| e == "build_end:a").unwrap();
    assert!(plan_b < build_end_a, "plan(b) must be observed before build(a) finishes: {events:?}");
}

/// (b): the queue is bounded -- with capacity 1 and a stalled builder, the 4th unit's *enqueue*
/// must not happen (planning cannot run unboundedly far ahead) until the builder catches up.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn planning_blocks_on_a_full_queue_when_the_builder_lags() {
    let dir = tempfile::tempdir().unwrap();
    let log = EventLog::default();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let build = gated_build_fn(log.clone(), "a", started_tx, release_rx);

    let units = vec!["a", "b", "c", "d"].into_iter().map(String::from).collect();
    let handle = tokio::spawn(run_plan_ahead(units, dir.path().to_path_buf(), "run-b", 1, plan_fn(log.clone()), build));
    started_rx.recv_timeout(std::time::Duration::from_secs(5)).expect("build(a) must start");
    assert!(wait_until(|| log.contains("plan:c")).await, "plan(c) should still happen (in flight + 1 buffered)");
    assert!(!wait_until(|| log.contains("plan:d")).await, "plan(d) ran despite a full, stalled build queue");

    release_tx.send(()).unwrap();
    handle.await.unwrap().unwrap();
    assert!(log.contains("plan:d") && log.contains("build_end:d"));
}
