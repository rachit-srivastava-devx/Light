use crate::grant::{ApprovalError, ApprovalGrant, ApprovalRequest};
use crate::port::ApprovalStore;

fn require_nonempty(field: &str, value: &str) -> Result<(), ApprovalError> {
    if value.trim().is_empty() {
        Err(ApprovalError::Empty(field.to_string()))
    } else {
        Ok(())
    }
}

/// Validate an `ApprovalRequest` against `now`. Rejects empty fields,
/// expired timestamps, and a missing actor.
fn validate_grant(req: &ApprovalRequest, actor: &str, now: u64) -> Result<(), ApprovalError> {
    require_nonempty("task_id", &req.task_id)?;
    require_nonempty("action", &req.action)?;
    require_nonempty("resource", &req.resource)?;
    require_nonempty("scope_hash", &req.scope_hash)?;
    require_nonempty("base_commit", &req.base_commit)?;
    require_nonempty("artifact_id", &req.artifact_id)?;
    require_nonempty("actor", actor)?;
    if req.expires_at <= now {
        return Err(ApprovalError::Expired);
    }
    Ok(())
}

/// Mint a scoped, expiring capability and durably persist it.
/// No grant is created unless the store write succeeds.
pub fn approve(
    store: &mut impl ApprovalStore,
    req: ApprovalRequest,
    actor: String,
    now: u64,
) -> Result<ApprovalGrant, ApprovalError> {
    validate_grant(&req, &actor, now)?;
    let grant = ApprovalGrant {
        approval_id: format!("grant-{}-{}", req.task_id, now),
        request: req,
        actor,
        issued_at: now,
    };
    store.put(&grant)?;
    Ok(grant)
}

/// Consume a previously minted grant by id, checking expiry at `now`.
/// Delegates replay prevention to the store's atomic consume.
pub fn consume(
    store: &mut impl ApprovalStore,
    id: &str,
    now: u64,
) -> Result<ApprovalGrant, ApprovalError> {
    let grant = store.consume(id)?;
    if grant.request.expires_at <= now {
        return Err(ApprovalError::Expired);
    }
    Ok(grant)
}
