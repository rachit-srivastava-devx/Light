use crate::dispatch::error::DispatchError;
use serde_json::json;
use std::path::Path;
use store::Ledger;
use store::ledger::LedgerPaths;
use types::{ExitCode, ReceiptEvent};
use verify::GateEvidence;

fn ledger(state_dir: &Path) -> Ledger {
    Ledger::open(LedgerPaths {
        chain: state_dir.join("ledger.chain"),
        lock: state_dir.join("ledger.lock"),
    })
}

fn append(
    state_dir: &Path,
    event: ReceiptEvent,
    body: serde_json::Value,
    code: ExitCode,
) -> Result<(), DispatchError> {
    ledger(state_dir)
        .append(event, body, "fleet-cli-verify".into(), None, Some(code))
        .map(|_| ())
        .map_err(|error| DispatchError::EnvFault(format!("verification receipt: {error}")))
}

pub(super) fn evidence(
    state_dir: &Path,
    evidence: &GateEvidence,
    code: ExitCode,
    scope: Option<verify::SecretScanScope>,
) -> Result<(), DispatchError> {
    let mut body = serde_json::to_value(evidence).map_err(|error| {
        DispatchError::EnvFault(format!("verification receipt encode: {error}"))
    })?;
    if let Some(scope) = scope {
        body["secret_scan"] = json!({
            "checked": scope.checked(),
            "total": scope.total(),
            "git_backed": scope.git_backed(),
            "git_requested": scope.requested_git(),
            "no_head_fallback": scope.no_head_fallback()
        });
    }
    append(state_dir, ReceiptEvent::GateVerdict, body, code)
}

pub(super) fn refusal(state_dir: &Path, error: &DispatchError) -> Result<(), DispatchError> {
    append(
        state_dir,
        ReceiptEvent::Refusal,
        json!({
            "outcome": "refused",
            "failure": error.to_string(),
            "checked": 0,
            "total": 0
        }),
        error.exit_code(),
    )
}

#[cfg(test)]
#[path = "verify_receipt_tests.rs"]
mod tests;
