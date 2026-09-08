//! `ReceiptLedger`/`ChangeEmitter` test doubles shared across the integration suites.

use fleet_lifecycle::{ChangeEmitter, ProposalRequest, ProposedChange, ReceiptLedger, TransitionReceipt};
use fleet_types::GateRefusal;
use std::cell::RefCell;

#[derive(Default)]
pub struct MemoryLedger(pub RefCell<Vec<TransitionReceipt>>);

impl ReceiptLedger for MemoryLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), GateRefusal> {
        self.0.borrow_mut().push(receipt);
        Ok(())
    }
}

#[derive(Default)]
pub struct FailingLedger;

impl ReceiptLedger for FailingLedger {
    fn append(&self, _receipt: TransitionReceipt) -> Result<(), GateRefusal> {
        Err(GateRefusal::new("LEDGER_DOWN", "unit test ledger always refuses"))
    }
}

#[derive(Default)]
pub struct RecordingEmitter(pub RefCell<Vec<ProposalRequest>>);

impl ChangeEmitter for RecordingEmitter {
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, GateRefusal> {
        self.0.borrow_mut().push(request.clone());
        Ok(ProposedChange {
            url: "https://example.test/acme/repo/pull/1".to_string(),
            head: request.head.clone(),
            commit: "0".repeat(40),
            changed_files: 1,
        })
    }
}

#[derive(Default)]
pub struct FailingEmitter(pub RefCell<Vec<ProposalRequest>>);

impl ChangeEmitter for FailingEmitter {
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, GateRefusal> {
        self.0.borrow_mut().push(request.clone());
        Err(GateRefusal::new("EMIT_DOWN", "unit test emitter always refuses"))
    }
}
