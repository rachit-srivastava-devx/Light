use crate::types::GateEvidence;

/// Derive the receipt authority for the secret-scan verdict.  The scan denominator is measured
/// by the provider rather than by the gate runner, so it is deliberately included here instead
/// of being treated as presentation metadata.  The domain separator prevents this value from
/// being confused with the candidate/evidence digest itself.
pub fn secret_scan_integrity_digest(evidence: &GateEvidence, checked: u64, total: u64) -> String {
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, b"fleet.verify.secret-scan-receipt.v1");
    field(&mut hasher, evidence.integrity_digest.as_bytes());
    field(&mut hasher, &checked.to_le_bytes());
    field(&mut hasher, &total.to_le_bytes());
    field(&mut hasher, &(evidence.findings.len() as u64).to_le_bytes());
    for finding in &evidence.findings {
        field(&mut hasher, finding.rule_id.as_bytes());
        field(&mut hasher, finding.severity.as_bytes());
        field(&mut hasher, finding.file.as_bytes());
        field(&mut hasher, &[u8::from(finding.redacted)]);
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

fn field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}
