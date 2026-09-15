use super::{
    append_secret_receipt, SecretScanFailure, SecretScanFailureKind, SecretScanOutcome,
    SecretScanReceipt,
};
use store::ledger::LedgerPaths;
use store::Ledger;
use types::{ExitCode, ReceiptEvent};
use verify::SecretFinding;

fn ledger(dir: &std::path::Path) -> Ledger {
    Ledger::open(LedgerPaths {
        chain: dir.join("ledger.chain"),
        lock: dir.join("ledger.lock"),
    })
}

#[test]
fn secret_scan_receipt_is_durable_typed_and_redacted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let receipt = SecretScanReceipt {
        outcome: SecretScanOutcome::Findings,
        checked: Some(7),
        total: Some(7),
        git_backed: Some(true),
        git_requested: Some(true),
        no_head_fallback: Some(false),
        findings: vec![SecretFinding {
            rule_id: "generic-api-key".into(),
            severity: "CRITICAL".into(),
            file: "config.env".into(),
            redacted: true,
        }],
        integrity_digest: Some("blake3:receipt-digest".into()),
        failure: Some(SecretScanFailure {
            kind: SecretScanFailureKind::ScannerFailed,
            message: "1 secret finding(s) blocked gate".into(),
        }),
    };
    append_secret_receipt(
        dir.path(),
        ReceiptEvent::GateVerdict,
        &receipt,
        ExitCode::Invariant,
    )
    .expect("receipt append");

    let rows = ledger(dir.path()).rows(false).expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event, ReceiptEvent::GateVerdict);
    assert_eq!(rows[0].exit_code, Some(ExitCode::Invariant));
    assert_eq!(rows[0].body["checked"], 7);
    assert_eq!(rows[0].body["total"], 7);
    assert_eq!(rows[0].body["findings"][0]["redacted"], true);
    assert_eq!(rows[0].body["integrity_digest"], "blake3:receipt-digest");
}

#[test]
fn refusal_receipt_has_typed_reason_and_no_fabricated_denominator() {
    let dir = tempfile::tempdir().expect("tempdir");
    let receipt = SecretScanReceipt {
        outcome: SecretScanOutcome::Refused,
        checked: None,
        total: None,
        git_backed: None,
        git_requested: None,
        no_head_fallback: None,
        findings: vec![],
        integrity_digest: None,
        failure: Some(SecretScanFailure {
            kind: SecretScanFailureKind::ScopeUnavailable,
            message: "scope unavailable".into(),
        }),
    };
    append_secret_receipt(
        dir.path(),
        ReceiptEvent::Refusal,
        &receipt,
        ExitCode::Refusal,
    )
    .expect("receipt append");

    let rows = ledger(dir.path()).rows(false).expect("rows");
    assert_eq!(rows[0].event, ReceiptEvent::Refusal);
    assert_eq!(rows[0].exit_code, Some(ExitCode::Refusal));
    assert!(rows[0].body.get("checked").is_none());
    assert!(rows[0].body.get("total").is_none());
    assert_eq!(rows[0].body["failure"]["kind"], "scope_unavailable");
}
