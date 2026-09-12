use crate::{IntegrationReceipt, MergeRequest};

/// Build an `IntegrationReceipt` from a completed merge.
///
/// MUST be called before any worktree cleanup so the receipt is durable
/// before any side effects are discarded.
pub fn write_receipt(req: &MergeRequest, before: &str, after: &str) -> IntegrationReceipt {
    IntegrationReceipt {
        before: before.to_string(),
        after: after.to_string(),
        lane: req.lane_head.clone(),
        candidate_digest: req.candidate_digest.clone(),
        checked: req.checked,
        total: req.total,
    }
}
