//! `Task<Accepted>::propose`'s digest-mismatch and empty-diff gates, plus a failed emitter
//! leaving no receipt behind.

mod common;

use common::{accepted_task, complete_elements, sample_request, FailingEmitter, MemoryLedger, RecordingEmitter};
use fleet_lifecycle::{AttestationBundle, ProposalRequest};

#[test]
fn digest_mismatch_refuses_before_the_emitter_runs() {
    let ledger = MemoryLedger::default();
    let task = accepted_task("propose-digest", &ledger);
    let bundle = AttestationBundle::new(complete_elements());
    let mut request = sample_request("fleet/propose-digest");
    request.artifact_id = "f".repeat(64);
    let emitter = RecordingEmitter::default();

    let refusal = task.propose(&bundle, &request, &emitter, &ledger).unwrap_err();

    assert_eq!(refusal.code(), "ARTIFACT_DIGEST_MISMATCH");
    assert!(emitter.0.borrow().is_empty());
}

#[test]
fn empty_diff_refuses_after_digest_check_passes() {
    let ledger = MemoryLedger::default();
    let task = accepted_task("propose-empty", &ledger);
    let bundle = AttestationBundle::new(complete_elements());
    let request = ProposalRequest {
        diff: vec![],
        artifact_id: blake3::hash(&[]).to_hex().to_string(),
        ..sample_request("fleet/propose-empty")
    };
    let emitter = RecordingEmitter::default();

    let refusal = task.propose(&bundle, &request, &emitter, &ledger).unwrap_err();

    assert_eq!(refusal.code(), "EMPTY_DIFF");
    assert!(emitter.0.borrow().is_empty());
}

#[test]
fn emitter_failure_leaves_no_receipt() {
    let ledger = MemoryLedger::default();
    let task = accepted_task("propose-emit-fails", &ledger);
    let bundle = AttestationBundle::new(complete_elements());
    let request = sample_request("fleet/propose-emit-fails");
    let emitter = FailingEmitter::default();
    let before = ledger.0.borrow().len();

    let refusal = task.propose(&bundle, &request, &emitter, &ledger).unwrap_err();

    assert_eq!(refusal.code(), "EMIT_DOWN");
    assert_eq!(ledger.0.borrow().len(), before, "a failed emit must not append a receipt");
}
