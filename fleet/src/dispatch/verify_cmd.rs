//! `fleet oracle|gate`: parse -> `fleet_verify::{run_all,GATES}` against real, injected
//! `ToolProbe`/`ProcessRunner` ports (which-on-PATH + `std::process::Command`) -> print. The
//! gate table and pass/fail classification are entirely `fleet-verify`'s; this only supplies
//! the two IO ports that crate declares and does not implement itself (BLUEPRINT §2/§7).

use crate::cli::args_ctx::GateArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_verify::{ProcessOutput, ProcessRunner, ProbeTool, Report, ToolProbe, Verdict};
use std::process::Command;

struct WhichProbe;
impl ToolProbe for WhichProbe {
    fn available(&self, tool: ProbeTool) -> bool {
        let name = match tool {
            ProbeTool::Cargo => "cargo",
            ProbeTool::CargoMutants => "cargo-mutants",
            ProbeTool::CargoFmt => "cargo-fmt",
            ProbeTool::CargoClippy => "cargo-clippy",
            ProbeTool::CargoDeny => "cargo-deny",
            ProbeTool::CargoAudit => "cargo-audit",
            ProbeTool::CargoLlvmCov => "cargo-llvm-cov",
            ProbeTool::Named(n) => n,
        };
        Command::new("which").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
    }
}

struct RealRunner;
impl ProcessRunner for RealRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        let Some((bin, rest)) = command.split_first() else {
            return ProcessOutput { exit_code: -1, stdout: String::new(), stderr: "empty command".into() };
        };
        match Command::new(bin).args(rest).output() {
            Ok(out) => ProcessOutput {
                exit_code: out.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(e) => ProcessOutput { exit_code: -1, stdout: String::new(), stderr: e.to_string() },
        }
    }
}

fn print_report(report: &Report) {
    for r in &report.results {
        let verdict = match &r.verdict {
            Verdict::Pass(_) => "pass".to_string(),
            Verdict::Fail { reason, .. } => format!("fail: {reason:?}"),
            Verdict::Skip { reason, .. } => format!("skip: {reason}"),
        };
        human::line(r.id, verdict);
    }
}

pub fn oracle() -> Result<(), DispatchError> {
    let report = fleet_verify::run_all(fleet_verify::GATES, &WhichProbe, &RealRunner);
    print_report(&report);
    Ok(())
}

pub fn gate(args: GateArgs) -> Result<(), DispatchError> {
    let specs: Vec<_> = fleet_verify::GATES
        .iter()
        .filter(|g| args.id.as_deref().map(|id| id == g.id).unwrap_or(true))
        .copied()
        .collect();
    let report = fleet_verify::run_all(&specs, &WhichProbe, &RealRunner);
    print_report(&report);
    Ok(())
}
