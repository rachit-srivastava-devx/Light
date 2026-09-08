//! `Task<Accepted>::propose`'s happy path and the attestation-completeness gate.

mod common;

use common::{accepted_task, complete_elements, sample_request, MemoryLedger, RecordingEmitter};
use fleet_lifecycle::AttestationBundle;

#[test]
fn complete_bundle_advances_to_proposed_and_records_one_receipt() {
    let ledger = MemoryLedger::default();
    let task = accepted_task("propose-pass", &ledger);
    let bundle = AttestationBundle::new(complete_elements());
    let request = sample_request("fleet/propose-pass");
    let emitter = RecordingEmitter::default();
    let before = ledger.0.borrow().len();

    let (proposed, change) = task.propose(&bundle, &request, &emitter, &ledger).unwrap();

    assert_eq!(proposed.id().as_str(), "propose-pass");
    assert_eq!(change.url, "https://example.test/acme/repo/pull/1");
    assert_eq!(emitter.0.borrow().len(), 1, "the emitter must run exactly once");
    assert_eq!(ledger.0.borrow().len(), before + 1, "propose must append exactly one receipt");
}

#[test]
fn incomplete_bundle_refuses_before_the_emitter_runs() {
    let ledger = MemoryLedger::default();
    let task = accepted_task("propose-refuse", &ledger);
    let mut elements = complete_elements();
    elements.as_object_mut().unwrap().remove("adequacy");
    let bundle = AttestationBundle::new(elements);
    let request = sample_request("fleet/propose-refuse");
    let emitter = RecordingEmitter::default();
    let before = ledger.0.borrow().len();

    let refusal = task.propose(&bundle, &request, &emitter, &ledger).unwrap_err();

    assert_eq!(refusal.code(), "INCOMPLETE_ATTESTATION");
    assert!(emitter.0.borrow().is_empty(), "the gate must run before the side effect");
    assert_eq!(ledger.0.borrow().len(), before, "a refusal must not append a receipt");
}
