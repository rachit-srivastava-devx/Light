use super::*;

#[test]
fn production_verification_surfaces_secret_findings_redacted() {
    let spec = override_spec();
    let specs = vec![spec];
    let gates = GatesRoot::materialize().expect("embedded gates");
    let findings = RecordingFindings {
        scans: Arc::new(Mutex::new(0)),
        findings: vec![SecretFinding {
            rule_id: "generic-api-key".into(),
            severity: "CRITICAL".into(),
            file: "config/secrets.env".into(),
            redacted: false,
        }],
    };
    let evidence = verify_production(
        &candidate(&specs),
        &EverythingAvailable,
        &RecordingRunner {
            stdout: "test result: ok. 1 passed; 0 failed; 0 ignored".into(),
            commands: Arc::new(Mutex::new(Vec::new())),
        },
        &gates,
        &specs,
        &findings,
    )
    .expect("production verification");
    assert!(!evidence.passed);
    assert_eq!(evidence.findings.len(), 1);
    assert!(evidence.findings[0].redacted);
    assert!(
        evidence
            .failures
            .iter()
            .any(|failure| failure.contains("secret"))
    );
}
