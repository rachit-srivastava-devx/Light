use crate::types::{IngestError, SourceRegistration, MAX_DELIVERY_ID_LEN, MAX_SOURCE_LEN};
use unicode_normalization::UnicodeNormalization;

/// NFC-normalize, trim, cap length, then verify against the registered namespace.
/// Returns the normalized form on success — this is what all downstream sees.
pub fn check_source(source: &str, reg: &SourceRegistration) -> Result<String, IngestError> {
    let normalized: String = source.nfc().collect();
    let trimmed = normalized.trim();

    if trimmed.is_empty() || trimmed.len() > MAX_SOURCE_LEN {
        return Err(IngestError::UnknownSource);
    }
    if trimmed != reg.namespace.as_str() {
        return Err(IngestError::UnknownSource);
    }
    Ok(trimmed.to_string())
}

/// Delivery identities become event IDs and deduplication keys; keep them bounded and opaque.
pub fn check_delivery_id(delivery_id: &str) -> Result<String, IngestError> {
    let trimmed = delivery_id.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_DELIVERY_ID_LEN
        || trimmed.chars().any(char::is_control)
    {
        return Err(IngestError::InvalidDeliveryId);
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests;
