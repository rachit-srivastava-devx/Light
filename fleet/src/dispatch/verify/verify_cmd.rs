//! `fleet oracle|gate`: parse -> `verify::{run_all,GATES}` against real, injected
//! `ToolProbe`/`ProcessRunner` ports (`verify_ports.rs`) -> print -> map the aggregate
//! `Report::exit_code()` to `Ok`/`Err` so the process's own exit code reflects the verdict
//! (BLUEPRINT §2/§7). Previously this unconditionally returned `Ok(())` after printing, so a
//! failing report still exited 0 -- the defect this file exists to not regress.

use super::verify_ports::{RealRunner, resolve_gates_root};
use super::verify_repo::ensure_repo_mode;
use super::which_probe::WhichProbe;
use crate::dispatch::error::DispatchError;
use cli::args_ctx::{GateArgs, OracleArgs};
use print::verify_report::render as print_report;
use types::ExitCode;
use verify::{GitleaksFindingsProvider, Report, VerifyError};

#[cfg(test)]
#[path = "verify_cmd_tests.rs"]
mod tests;

/// Turns a `Report`'s own `exit_code()` into `Ok`/`Err` -- the seam that used to be missing:
/// `oracle`/`gate` printed the report and then unconditionally returned `Ok(())`, so a caller
/// doing `fleet gate || exit 1` could never observe a failing gate.
fn to_result(report: Report) -> Result<(), DispatchError> {
    match report.exit_code() {
        ExitCode::Ok => Ok(()),
        code => Err(DispatchError::VerifyFailed {
            failed: report.failed(),
            skipped: report.skipped(),
            total: report.results.len(),
            code,
            detail: format!(
                "{} gate(s) failed or skipped",
                report.failed() + report.skipped()
            ),
        }),
    }
}

fn verification_error(error: VerifyError) -> DispatchError {
    match error {
        VerifyError::ScannerScopeUnavailable(reason) => DispatchError::Refusal(format!(
            "verification: secret scanner scope unavailable: {reason}"
        )),
        other => DispatchError::EnvFault(format!("verification: {other}")),
    }
}

fn refused(state_dir: &std::path::Path, error: DispatchError) -> Result<(), DispatchError> {
    super::verify_receipt::refusal(state_dir, &error)?;
    Err(error)
}

pub fn oracle(state_dir: &std::path::Path, args: OracleArgs) -> Result<(), DispatchError> {
    let repo = match ensure_repo_mode(&args.repo, !args.no_git) {
        Ok(repo) => repo,
        Err(error) => return refused(state_dir, error),
    };
    let specs = match super::gate_config::resolve(&repo) {
        Ok(specs) => specs,
        Err(error) => return refused(state_dir, error),
    };
    let gates = match resolve_gates_root() {
        Ok(gates) => gates,
        Err(error) => return refused(state_dir, error.into()),
    };
    let runner = RealRunner::new(&repo);
    let candidate = match verify::candidate_for_repo_with_specs(&repo, &gates, &specs) {
        Ok(candidate) => candidate,
        Err(error) => {
            return refused(
                state_dir,
                DispatchError::EnvFault(format!("candidate: {error}")),
            );
        }
    };
    let provider = GitleaksFindingsProvider::new(&repo).with_git_required(!args.no_git);
    let evidence = match verify::verify_production(
        &candidate,
        &WhichProbe,
        &runner,
        &gates,
        &specs,
        &provider,
    ) {
        Ok(evidence) => evidence,
        Err(error) => return refused(state_dir, verification_error(error)),
    };
    let report = verify::report_from_evidence(&evidence);
    print_report(&report);
    let code = if evidence.findings.is_empty() {
        report.exit_code()
    } else {
        ExitCode::Invariant
    };
    super::verify_receipt::evidence(state_dir, &evidence, code, provider.last_scope().ok())?;
    if !evidence.findings.is_empty() {
        eprintln!(
            "secret findings: {}",
            verify::findings_summary(&evidence.findings)
        );
        return Err(DispatchError::SecretsFound {
            count: evidence.findings.len(),
        });
    }
    to_result(report)
}

pub fn gate(state_dir: &std::path::Path, args: GateArgs) -> Result<(), DispatchError> {
    let repo = match ensure_repo_mode(&args.repo, !args.no_git) {
        Ok(repo) => repo,
        Err(error) => return refused(state_dir, error),
    };
    let specs: Vec<_> = match super::gate_config::resolve(&repo) {
        Ok(specs) => specs,
        Err(error) => return refused(state_dir, error),
    }
    .into_iter()
    .filter(|g| args.id.as_deref().map(|id| id == g.id).unwrap_or(true))
    .collect();
    if let Some(id) = &args.id {
        if specs.is_empty() {
            return refused(state_dir, DispatchError::UnknownGate(id.clone()));
        }
    }
    let gates = match resolve_gates_root() {
        Ok(gates) => gates,
        Err(error) => return refused(state_dir, error.into()),
    };
    let runner = RealRunner::new(&repo);
    let candidate = match verify::candidate_for_repo_with_specs(&repo, &gates, &specs) {
        Ok(candidate) => candidate,
        Err(error) => {
            return refused(
                state_dir,
                DispatchError::EnvFault(format!("candidate: {error}")),
            );
        }
    };
    let provider = GitleaksFindingsProvider::new(&repo).with_git_required(!args.no_git);
    let evidence = match verify::verify_production(
        &candidate,
        &WhichProbe,
        &runner,
        &gates,
        &specs,
        &provider,
    ) {
        Ok(evidence) => evidence,
        Err(error) => return refused(state_dir, verification_error(error)),
    };
    let report = verify::report_from_evidence(&evidence);
    print_report(&report);
    let code = if evidence.findings.is_empty() {
        report.exit_code()
    } else {
        ExitCode::Invariant
    };
    super::verify_receipt::evidence(state_dir, &evidence, code, provider.last_scope().ok())?;
    if !evidence.findings.is_empty() {
        eprintln!(
            "secret findings: {}",
            verify::findings_summary(&evidence.findings)
        );
        return Err(DispatchError::SecretsFound {
            count: evidence.findings.len(),
        });
    }
    to_result(report)
}
