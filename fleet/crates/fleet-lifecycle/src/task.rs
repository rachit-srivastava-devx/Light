//! `Task<S>` and the shared accessors/private transition. Ported verbatim from
//! `fleet/keel/fleet/src/lifecycle.rs:188-194,516-549`.

use crate::receipt::{ReceiptLedger, TransitionReceipt};
use crate::states::State;
use crate::task_id::TaskId;
use fleet_types::GateRefusal;
use std::marker::PhantomData;

/// A task whose legal operations are determined entirely by `S`. Fields are private to this
/// crate: no caller, including `src/`, can construct or inspect a `Task<S>` except through
/// the typed edges or the sanctioned `resume` bridge.
#[derive(Debug)]
pub struct Task<S: State> {
    pub(crate) id: TaskId,
    pub(crate) retry_depth: u32,
    pub(crate) _s: PhantomData<S>,
}

impl<S: State> Task<S> {
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    pub fn retry_depth(&self) -> u32 {
        self.retry_depth
    }

    pub(crate) fn transition<N: State>(
        self,
        evidence: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<N>, GateRefusal> {
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            return Err(GateRefusal::new(
                "EMPTY_TRANSITION_EVIDENCE",
                "a lifecycle transition requires evidence",
            ));
        }
        ledger.append(TransitionReceipt {
            task_id: self.id.clone(),
            from: std::any::type_name::<S>(),
            to: std::any::type_name::<N>(),
            evidence,
        })?;
        Ok(Task { id: self.id, retry_depth: self.retry_depth, _s: PhantomData })
    }
}
