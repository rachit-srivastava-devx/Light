//! Compile-time lifecycle for a fleet task. A state is represented by the type parameter of
//! `Task<S>`. Every transition consumes the old task, calls the caller-supplied
//! `ReceiptLedger`, and returns a fresh `Task` typed to the new state -- or a `GateRefusal`,
//! with the old task (and the ledger) left untouched. This crate performs no IO:
//! `ReceiptLedger` and `ChangeEmitter` are ports the caller implements; this module only
//! decides what transitions are legal.

mod advance;
mod attestation;
mod attestation_checks;
mod edges_attested;
mod edges_intake;
mod edges_propose;
mod edges_refusal;
mod edges_specified;
mod edges_straight;
mod human_approval;
mod proposal_types;
mod receipt;
mod resume;
mod states;
mod task;
mod task_id;

pub use advance::advance_any;
pub use attestation::AttestationBundle;
pub use human_approval::HumanApproval;
pub use proposal_types::{ChangeEmitter, ProposalRequest, ProposedChange};
pub use receipt::{ReceiptLedger, TransitionReceipt};
pub use resume::{resume, AnyTask};
pub use states::{
    Accepted, Attested, Briefed, Building, Built, Contracted, Decomposed, Intake, Leased,
    Observed, Proposed, Refused, Reviewed, Specified, State, Verified, Verifying,
};
pub use task::Task;
pub use task_id::TaskId;
