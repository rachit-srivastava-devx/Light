//! `Verify`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Runs the real
//! committed gate table (`verify::GATES`) against the same real `WhichProbe`/`RealRunner`
//! ports `fleet oracle`/`fleet gate` already use (`dispatch::verify_ports`) -- not the
//! always-unavailable probe + always-`exit 0` runner this stage used to hardcode, which made the
//! one stage whose job is catching failure structurally unable to ever fail.

use super::event::PipelineError;
use super::ledger_events;
use super::records::{label, GateRecord};
use crate::dispatch::verify_ports::{resolve_gates_root, RealRunner};
use crate::dispatch::which_probe::WhichProbe;
use print::human_stream::emit;
use print::render_event::Event;
use print::style::Style;
use print::verify_report::line_for;
use std::path::Path;
use types::{ExitCode, ReceiptEvent};
use verify::{GateSpec, GitleaksFindingsProvider, SecretFinding, Verdict, VerifyError};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SecretScanOutcome {
    Passed,
    Findings,
    Refused,
}

#[derive(Debug, serde::Serialize)]
struct SecretScanReceipt {
    outcome: SecretScanOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    git_backed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    git_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_head_fallback: Option<bool>,
    findings: Vec<SecretFinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrity_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<SecretScanFailure>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SecretScanFailureKind {
    ScannerUnavailable,
    ScannerFailed,
    ScannerTimeout,
    ScannerOutputInvalid,
    ScopeUnavailable,
    VerificationInvariant,
}

#[derive(Debug, serde::Serialize)]
struct SecretScanFailure {
    kind: SecretScanFailureKind,
    message: String,
}

/// `repo` is the same `--repo` the pipeline was invoked with (`StageCtx::repo`) -- the S1 fix:
/// this stage used to run every gate against the `fleet` process's own cwd instead of the repo
/// the caller actually named.
pub fn verify(
    gates: &[GateSpec],
    repo: &Path,
    state_dir: &Path,
    out: &mut Vec<GateRecord>,
    git_backed: bool,
) -> Result<(), PipelineError> {
    let gates_root = resolve_gates_root().map_err(|e| PipelineError::VerifyTyped {
        detail: format!("gates root: {e}"),
        code: ExitCode::Env,
    })?;
    let runner = RealRunner::new(repo);
    let candidate =
        verify::candidate_for_repo_with_specs(repo, &gates_root, gates).map_err(|error| {
            PipelineError::VerifyTyped {
                detail: format!("candidate: {error}"),
                code: ExitCode::Invariant,
            }
        })?;
    let provider = GitleaksFindingsProvider::new(repo).with_git_required(git_backed);
    let evidence = match verify::verify_production(
        &candidate,
        &WhichProbe,
        &runner,
        &gates_root,
        gates,
        &provider,
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            let scope = provider.last_scope().ok();
            let (kind, exit_code) = secret_scan_failure_kind(&error);
            let receipt = SecretScanReceipt {
                outcome: SecretScanOutcome::Refused,
                checked: scope.map(|value| value.checked()),
                total: scope.map(|value| value.total()),
                git_backed: scope.map(|value| value.git_backed()),
                git_requested: scope.map(|value| value.requested_git()),
                no_head_fallback: scope.map(|value| value.no_head_fallback()),
                findings: Vec::new(),
                integrity_digest: None,
                failure: Some(SecretScanFailure {
                    kind,
                    message: error.to_string(),
                }),
            };
            append_secret_receipt(state_dir, ReceiptEvent::Refusal, &receipt, exit_code)?;
            return Err(PipelineError::VerifyTyped {
                detail: format!("verification: {error}"),
                code: exit_code,
            });
        }
    };
    let secret_scope = match provider.last_scope() {
        Ok(scope) => scope,
        Err(error) => {
            let receipt = SecretScanReceipt {
                outcome: SecretScanOutcome::Refused,
                checked: None,
                total: None,
                git_backed: None,
                git_requested: None,
                no_head_fallback: None,
                findings: Vec::new(),
                integrity_digest: None,
                failure: Some(SecretScanFailure {
                    kind: SecretScanFailureKind::ScopeUnavailable,
                    message: error.to_string(),
                }),
            };
            let (_, exit_code) = secret_scan_failure_kind(&error);
            append_secret_receipt(state_dir, ReceiptEvent::Refusal, &receipt, exit_code)?;
            return Err(PipelineError::VerifyTyped {
                detail: format!("secret scan scope: {error}"),
                code: exit_code,
            });
        }
    };
    let report = verify::report_from_evidence(&evidence);
    let mut failed: Vec<String> = Vec::new();
    for r in &report.results {
        report_gate(state_dir, r, out);
        if let Verdict::Fail { reason, .. } = &r.verdict {
            failed.push(format!("{}: {reason:?}", r.id));
        }
    }
    let findings = verify::normalize_findings(evidence.findings.clone());
    let has_findings = !findings.is_empty();
    let receipt = SecretScanReceipt {
        outcome: if has_findings {
            SecretScanOutcome::Findings
        } else {
            SecretScanOutcome::Passed
        },
        checked: Some(secret_scope.checked()),
        total: Some(secret_scope.total()),
        git_backed: Some(secret_scope.git_backed()),
        git_requested: Some(secret_scope.requested_git()),
        no_head_fallback: Some(secret_scope.no_head_fallback()),
        findings,
        integrity_digest: Some(verify::secret_scan_integrity_digest(
            &evidence,
            secret_scope.checked(),
            secret_scope.total(),
        )),
        failure: has_findings.then(|| SecretScanFailure {
            kind: SecretScanFailureKind::ScannerFailed,
            message: format!("{} secret finding(s) blocked gate", evidence.findings.len()),
        }),
    };
    append_secret_receipt(
        state_dir,
        ReceiptEvent::GateVerdict,
        &receipt,
        if has_findings {
            ExitCode::Invariant
        } else {
            ExitCode::Ok
        },
    )?;
    if !evidence.findings.is_empty() {
        return Err(PipelineError::VerifyTyped {
            detail: format!(
                "secret scan found {} redacted finding(s)",
                evidence.findings.len()
            ),
            code: ExitCode::Invariant,
        });
    }
    if !failed.is_empty() {
        return Err(PipelineError::VerifyTyped {
            detail: format!("gate(s) failed: {}", failed.join("; ")),
            code: ExitCode::Invariant,
        });
    }
    Ok(())
}

fn append_secret_receipt(
    state_dir: &Path,
    event: ReceiptEvent,
    receipt: &SecretScanReceipt,
    exit_code: ExitCode,
) -> Result<(), PipelineError> {
    let body = serde_json::to_value(receipt)
        .map_err(|error| PipelineError::Verify(format!("secret receipt encode: {error}")))?;
    ledger_events::append_as_with_exit(
        state_dir,
        event,
        body,
        "fleet-cli-pipeline",
        Some(exit_code),
    )
    .map_err(|error| PipelineError::Verify(format!("secret receipt append: {error}")))
}

fn secret_scan_failure_kind(error: &VerifyError) -> (SecretScanFailureKind, ExitCode) {
    match error {
        VerifyError::NoGates => (
            SecretScanFailureKind::VerificationInvariant,
            ExitCode::Invariant,
        ),
        VerifyError::ScannerUnavailable(_) => {
            (SecretScanFailureKind::ScannerUnavailable, ExitCode::Env)
        }
        VerifyError::ScannerFailed { .. } => (SecretScanFailureKind::ScannerFailed, ExitCode::Env),
        VerifyError::ScannerTimeout => (SecretScanFailureKind::ScannerTimeout, ExitCode::Env),
        VerifyError::ScannerParse(_) => (
            SecretScanFailureKind::ScannerOutputInvalid,
            ExitCode::Invariant,
        ),
        VerifyError::ScannerScopeUnavailable(_) => {
            (SecretScanFailureKind::ScopeUnavailable, ExitCode::Refusal)
        }
        VerifyError::CoverageParseError(_) => (
            SecretScanFailureKind::ScannerOutputInvalid,
            ExitCode::Invariant,
        ),
        VerifyError::InvalidCandidate(_) | VerifyError::InvalidGate(_) => (
            SecretScanFailureKind::VerificationInvariant,
            ExitCode::Invariant,
        ),
        VerifyError::SecretFound => (SecretScanFailureKind::ScannerFailed, ExitCode::Invariant),
    }
}

/// One gate's real, structured verdict, pushed to both consumers of the same `Event`: the human
/// render path (previously silent per-gate during `fleet run` -- only the stage's own pass/fail
/// showed) and the durable ledger `fleet-stream` tails.
fn report_gate(state_dir: &Path, r: &verify::GateResult, out: &mut Vec<GateRecord>) {
    let event = line_for(r);
    emit(&event, &Style::detect());
    let Event::GateVerdict {
        id,
        outcome,
        checked,
        total,
        detail,
    } = &event
    else {
        return;
    };
    let body = serde_json::json!({
        "id": id, "outcome": format!("{outcome:?}"), "checked": checked, "total": total, "detail": detail,
    });
    ledger_events::observe(state_dir, ReceiptEvent::GateVerdict, body);
    out.push(GateRecord {
        id: id.clone(),
        verdict: label(*outcome),
        checked: *checked,
        total: *total,
        detail: detail.clone(),
        env_fault: matches!(
            r.verdict,
            Verdict::Skip {
                was_required: true,
                ..
            }
        ),
    });
}

#[cfg(test)]
#[path = "verify_receipt_tests.rs"]
mod tests;
