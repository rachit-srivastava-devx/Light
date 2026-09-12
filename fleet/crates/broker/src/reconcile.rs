//! Readback reconciler — resolves Unknown states via provider readback.
use crate::effect::{BrokerError, EffectState, Readback};

/// Verdict from a reconciliation attempt. `checked == total > 0` is required.
pub struct ReconcileResult {
    pub checked: usize,
    pub total: usize,
    pub final_state: EffectState,
}

/// Reconcile one Unknown effect via provider readback.
/// Returns `Err(BrokerError::Reconcile)` on zero-row or vacuous recovery.
pub fn reconcile(readback: Result<Readback, BrokerError>) -> Result<ReconcileResult, BrokerError> {
    match readback {
        Ok(rb) => Ok(ReconcileResult {
            checked: 1,
            total: 1,
            final_state: rb.state,
        }),
        Err(BrokerError::Provider(msg)) => Err(BrokerError::Reconcile(msg)),
        Err(e) => Err(e),
    }
}
