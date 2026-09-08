//! Full `Intake..Observed` walk (ported from `lifecycle.rs`'s `legal_path_consumes_each_
//! state_and_records_every_edge`) plus the empty-evidence and ledger-failure unit cases,
//! which need only the public API.

mod common;

use common::{complete_elements, sample_request, FailingLedger, MemoryLedger, RecordingEmitter};
use fleet_lifecycle::{HumanApproval, Task, TaskId};

#[test]
fn legal_path_consumes_each_state_and_records_every_edge() {
    let ledger = MemoryLedger::default();
    let task = Task::new(TaskId::new("task-1").unwrap());
    let task = task.specify("SOW", &ledger).unwrap();
    let task = task.review(HumanApproval::recorded("operator approval").unwrap(), &ledger).unwrap();
    assert_eq!(task.retry_depth(), 0);
    let task = task.decompose("atomic leaves", &ledger).unwrap();
    let task = task.contract("blind suite", &ledger).unwrap();
    let task = task.brief("assembled brief", &ledger).unwrap();
    let task = task.lease("worktree lease", &ledger).unwrap();
    let task = task.build("supervised spawn", &ledger).unwrap();
    let task = task.finish_build("non-empty diff", &ledger).unwrap();
    let task = task.begin_verification("different model", &ledger).unwrap();
    let task = task.verify("reproduced", &ledger).unwrap();
    let task = task.attest("seven elements", &ledger).unwrap();
    let task = task.accept(HumanApproval::recorded("operator acceptance").unwrap(), &ledger).unwrap();
    let bundle = fleet_lifecycle::AttestationBundle::new(complete_elements());
    let request = sample_request("fleet/task-1");
    let emitter = RecordingEmitter::default();
    let (task, _change) = task.propose(&bundle, &request, &emitter, &ledger).unwrap();
    assert_eq!(task.retry_depth(), 0);
    let task = task.observe("measured baseline", &ledger).unwrap();
    let task = task.reopen("control band breach", &ledger).unwrap();

    assert_eq!(task.id().as_str(), "task-1");
    assert_eq!(task.retry_depth(), 0);
    assert_eq!(ledger.0.borrow().len(), 15);
}

#[test]
fn empty_evidence_refuses_without_advancing_or_writing() {
    let ledger = MemoryLedger::default();
    let task = Task::new(TaskId::new("task-2").unwrap());
    let refusal = task.specify("  ", &ledger).unwrap_err();
    assert_eq!(refusal.code(), "EMPTY_TRANSITION_EVIDENCE");
    assert!(ledger.0.borrow().is_empty());
}

#[test]
fn ledger_append_failure_yields_no_new_task() {
    let ledger = FailingLedger;
    let task = Task::new(TaskId::new("task-3").unwrap());
    let refusal = task.specify("a real SOW", &ledger).unwrap_err();
    assert_eq!(refusal.code(), "LEDGER_DOWN");
}
