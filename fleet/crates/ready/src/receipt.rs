use serde::{Deserialize, Serialize};

use crate::types::{ReadyInput, ReadyVerdict};

/// Canonical receipt for an admission decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub plan_digest: String,
    pub resource_profile: String,
    pub status: String,
    pub checked: u64,
    pub total: u64,
}

impl Receipt {
    pub fn from_verdict(input: &ReadyInput, verdict: &ReadyVerdict) -> Self {
        Receipt {
            plan_digest: input.plan_digest.clone(),
            resource_profile: input.resource_profile.clone(),
            status: format!("{:?}", verdict.status),
            checked: verdict.checked,
            total: verdict.total,
        }
    }
}

/// Validate the denomination: checked > 0 and checked == total.
/// Returns Err(()) when the denominator is zero or inconsistent.
pub fn validate_denominator(input: &ReadyInput) -> Result<(), ()> {
    if input.checked == 0 || input.total == 0 || input.checked != input.total {
        return Err(());
    }
    Ok(())
}
