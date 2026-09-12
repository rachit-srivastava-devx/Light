//! Delivery reconciliation: receipt building, unknown classification, failure handling.
use crate::{DeliveryReceipt, DeliveryStatus, NotifyError};

/// Input for building a delivery receipt. Zero denominator is refused.
pub struct NotifyInput {
    pub effect_id: String,
    pub delivery_attempts: u64,
    pub total_recipients: u64,
}

/// Build a delivery receipt. Returns `ZeroDenominator` when `total_recipients == 0`.
pub fn build_delivery_receipt(input: NotifyInput) -> Result<DeliveryReceipt, NotifyError> {
    if input.total_recipients == 0 {
        return Err(NotifyError::ZeroDenominator);
    }
    Ok(DeliveryReceipt {
        effect_id: input.effect_id,
        status: DeliveryStatus::Unknown,
        attempts: input.delivery_attempts,
        checked: input.delivery_attempts.min(input.total_recipients),
        total: input.total_recipients,
    })
}

/// Classify a transport error as `Unknown`; never panics, never fabricates `Delivered`.
pub fn handle_delivery_failure(effect_id: &str, _err: &NotifyError) -> DeliveryReceipt {
    DeliveryReceipt {
        effect_id: effect_id.to_string(),
        status: DeliveryStatus::Unknown,
        attempts: 1,
        checked: 0,
        total: 1,
    }
}
