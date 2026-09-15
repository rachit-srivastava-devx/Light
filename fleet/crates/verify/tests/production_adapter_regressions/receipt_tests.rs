use super::*;

#[test]
fn secret_scan_receipt_digest_binds_denominator_and_redaction() {
    let candidate = candidate(&[override_spec()]);
    let evidence = assemble_gate_evidence(
        &candidate,
        vec![verify::CanonicalGateResult {
            id: "unit tests".into(),
            exit_code: 0,
            stdout_digest: "blake3:stdout".into(),
            stderr_digest: "blake3:stderr".into(),
            input_digest: "denominator:1/1".into(),
            checked: 1,
            total: 1,
            passed: true,
            failure_message: None,
        }],
        vec![SecretFinding {
            rule_id: "generic-api-key".into(),
            severity: "CRITICAL".into(),
            file: "config.env".into(),
            redacted: true,
        }],
    );
    let baseline = secret_scan_integrity_digest(&evidence, 4, 4);
    assert_ne!(baseline, secret_scan_integrity_digest(&evidence, 3, 4));
    assert_ne!(baseline, secret_scan_integrity_digest(&evidence, 4, 5));
}
