//! Grant validation and deterministic idempotency key derivation.
use crate::effect::{BrokerError, ExternalEffect};
use std::time::{SystemTime, UNIX_EPOCH};

/// Validates that the grant is consumed, matches action/resource/hash, and is not expired.
pub fn validate_grant(e: &ExternalEffect) -> Result<(), BrokerError> {
    let g = &e.grant;
    if !g.consumed {
        return Err(BrokerError::Grant("grant not consumed".into()));
    }
    if g.action != e.action {
        return Err(BrokerError::Grant(format!(
            "grant action '{}' != effect action '{}'",
            g.action, e.action
        )));
    }
    if g.resource != e.resource {
        return Err(BrokerError::Grant(format!(
            "grant resource '{}' != effect resource '{}'",
            g.resource, e.resource
        )));
    }
    if g.payload_hash != e.payload_hash {
        return Err(BrokerError::Grant("payload hash mismatch".into()));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if g.expires_at <= now {
        return Err(BrokerError::Grant("grant expired".into()));
    }
    Ok(())
}

/// Returns the caller-provided idempotency key. Deterministic: same effect → same key.
pub fn idempotency_key(e: &ExternalEffect) -> String {
    e.idempotency_key.clone()
}
