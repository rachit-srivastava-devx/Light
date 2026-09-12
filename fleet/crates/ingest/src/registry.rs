use unicode_normalization::UnicodeNormalization;
use crate::types::{IngestError, SourceRegistration, MAX_SOURCE_LEN};

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
