//! Broker — the only parent-authorized path for external effects.
pub mod authorize;
pub mod effect;
pub mod provider;
pub mod reconcile;

pub use effect::{
    BrokerError, EffectState, EffectStore, ExternalEffect, Grant, ProviderAck, Readback, Receipt,
};
pub use provider::Provider;

/// Execute one external effect through the broker state machine.
///
/// Preconditions: consumed grant, unique key, valid expiry, store available.
/// Postcondition: every outcome produces a receipt; `Acked` requires provider ack; no panic.
pub fn execute(
    p: &impl Provider,
    store: &mut impl EffectStore,
    e: ExternalEffect,
) -> Result<Receipt, BrokerError> {
    let key = authorize::idempotency_key(&e);
    // Idempotency: return existing Acked state without re-dispatching.
    if let Some(state) = store.load(&key) {
        if state == EffectState::Acked {
            return Ok(Receipt {
                key,
                state: EffectState::Acked,
                checked: 1,
                total: 1,
            });
        }
    }
    // Grant validation — must succeed before any store write or provider call.
    authorize::validate_grant(&e)?;
    // Persist PREPARED, then DISPATCHING, before network I/O.
    store.save(&key, EffectState::Prepared)?;
    store.save(&key, EffectState::Dispatching)?;
    match p.dispatch(&e) {
        Ok(_ack) => {
            store.save(&key, EffectState::Acked)?;
            Ok(Receipt {
                key,
                state: EffectState::Acked,
                checked: 1,
                total: 1,
            })
        }
        Err(BrokerError::Provider(_)) => {
            store.save(&key, EffectState::Unknown)?;
            Ok(Receipt {
                key,
                state: EffectState::Unknown,
                checked: 1,
                total: 1,
            })
        }
        Err(err) => Err(err),
    }
}
