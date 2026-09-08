//! The `refusal_edge!` macro and its 5 invocations. Ported verbatim from
//! `fleet/keel/fleet/src/lifecycle.rs:266-284`.

use crate::receipt::ReceiptLedger;
use crate::states::{Accepted, Attested, Building, Built, Refused, Verifying};
use crate::task::Task;
use fleet_types::GateRefusal;

macro_rules! refusal_edge {
    ($from:ident) => {
        impl Task<$from> {
            pub fn refuse(
                self,
                reason: impl Into<String>,
                ledger: &impl ReceiptLedger,
            ) -> Result<Task<Refused>, GateRefusal> {
                self.transition(reason, ledger)
            }
        }
    };
}

refusal_edge!(Building);
refusal_edge!(Built);
refusal_edge!(Verifying);
refusal_edge!(Attested);
refusal_edge!(Accepted);
