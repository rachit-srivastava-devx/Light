//! The `edge!` macro and its 11 invocations -- the straight-through edges. Ported verbatim
//! from `fleet/keel/fleet/src/lifecycle.rs:240-264`.

use super::receipt::ReceiptLedger;
use super::states::{
    Attested, Briefed, Building, Built, Contracted, Decomposed, Intake, Leased, Observed, Proposed,
    Reviewed, Verified, Verifying,
};
use super::task::Task;
use ::types::GateRefusal;

macro_rules! edge {
    ($from:ident, $method:ident, $to:ident) => {
        impl Task<$from> {
            pub fn $method(
                self,
                evidence: impl Into<String>,
                ledger: &impl ReceiptLedger,
            ) -> Result<Task<$to>, GateRefusal> {
                self.transition(evidence, ledger)
            }
        }
    };
}

edge!(Reviewed, decompose, Decomposed);
edge!(Decomposed, contract, Contracted);
edge!(Contracted, brief, Briefed);
edge!(Briefed, lease, Leased);
edge!(Leased, build, Building);
edge!(Building, finish_build, Built);
edge!(Built, begin_verification, Verifying);
edge!(Verifying, verify, Verified);
edge!(Verified, attest, Attested);
edge!(Proposed, observe, Observed);
edge!(Observed, reopen, Intake);
