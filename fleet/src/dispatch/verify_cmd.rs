//! `fleet oracle|gate`: parse -> `fleet_verify::{run_all,GATES}` against real, injected
//! `ToolProbe`/`ProcessRunner` ports (`verify_ports.rs`) -> print -> map the aggregate
//! `Report::exit_code()` to `Ok`/`Err` so the process's own exit code reflects the verdict
//! (BLUEPRINT §2/§7). Previously this unconditionally returned `Ok(())` after printing, so a
//! failing report still exited 0 -- the defect this file exists to not regress.

use super::verify_ports::{resolve_gates_root, RealRunner};
use super::which_probe::WhichProbe;
use super::verify_repo::ensure_repo;
use crate::cli::args_ctx::{GateArgs, OracleArgs};
use crate::dispatch::error::DispatchError;
use crate::print::verify_report::render as print_report;
use fleet_types::ExitCode;
use fleet_verify::Report;

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
        }),
    }
}

pub fn oracle(args: OracleArgs) -> Result<(), DispatchError> {
    let repo = ensure_repo(&args.repo)?;
    let specs = super::gate_config::resolve(&repo)?;
    let gates = resolve_gates_root()?;
    let runner = RealRunner::new(repo);
    let report = fleet_verify::run_all(&specs, &WhichProbe, &runner, &gates);
    print_report(&report);
    to_result(report)
}

pub fn gate(args: GateArgs) -> Result<(), DispatchError> {
    let repo = ensure_repo(&args.repo)?;
    let specs: Vec<_> = super::gate_config::resolve(&repo)?
        .into_iter()
        .filter(|g| args.id.as_deref().map(|id| id == g.id).unwrap_or(true))
        .collect();
    if let Some(id) = &args.id {
        if specs.is_empty() {
            return Err(DispatchError::UnknownGate(id.clone()));
        }
    }
    let gates = resolve_gates_root()?;
    let runner = RealRunner::new(repo);
    let report = fleet_verify::run_all(&specs, &WhichProbe, &runner, &gates);
    print_report(&report);
    to_result(report)
}
