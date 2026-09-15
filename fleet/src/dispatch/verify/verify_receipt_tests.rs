use super::*;
use std::path::Path;

fn rows(dir: &Path) -> Vec<types::Receipt> {
    ledger(dir).rows(false).expect("receipt rows")
}

#[test]
fn refusal_is_durable_before_cli_returns() {
    let dir = tempfile::tempdir().expect("state dir");
    let error = DispatchError::Refusal("bad verification input".into());
    refusal(dir.path(), &error).expect("refusal receipt");
    let rows = rows(dir.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event, ReceiptEvent::Refusal);
    assert_eq!(rows[0].exit_code, Some(ExitCode::Refusal));
    assert_eq!(rows[0].body["outcome"], "refused");
}

#[test]
fn canonical_evidence_is_durable_with_typed_exit() {
    let dir = tempfile::tempdir().expect("state dir");
    let report = GateEvidence {
        gate_results: vec![],
        checked: 0,
        total: 0,
        evidence_digest: "blake3:evidence".into(),
        integrity_digest: "blake3:integrity".into(),
        status: verify::Status::Failed,
        passed: false,
        failures: vec!["no gate inputs were checked".into()],
        findings: vec![],
    };
    evidence(dir.path(), &report, ExitCode::Invariant, None).expect("evidence receipt");
    let rows = rows(dir.path());
    assert_eq!(rows[0].event, ReceiptEvent::GateVerdict);
    assert_eq!(rows[0].exit_code, Some(ExitCode::Invariant));
    assert_eq!(rows[0].body["integrity_digest"], "blake3:integrity");
}
