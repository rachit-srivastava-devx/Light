//! `WhichProbe`: the real `ToolProbe` impl, split out of `verify_ports.rs` to keep that file
//! under the 80-line cap.

use super::mutants_probe;
use fleet_verify::{ProbeTool, ToolProbe};
use std::process::Command;

pub struct WhichProbe;
impl ToolProbe for WhichProbe {
    /// D27: `mutants` must stay opt-in -- a merge once dropped this guard and the gate ran
    /// unasked because `cargo-mutants` happened to be on `$PATH`. So `CargoMutants` is
    /// "available" only with `FLEET_MUTANTS=1` set (and actually on `PATH`); else
    /// `Verdict::Skip`, a visible `SKIP` line -- `unavailable_reason` below says which of the two
    /// it was, instead of collapsing both into one misleading "unavailable".
    fn available(&self, tool: ProbeTool) -> bool {
        match tool {
            ProbeTool::CargoMutants => matches!(mutants_probe::probe(), mutants_probe::Availability::Yes),
            other => on_path(name_on_path(other)),
        }
    }

    fn unavailable_reason(&self, tool: ProbeTool) -> String {
        match tool {
            ProbeTool::CargoMutants => mutants_probe::reason(mutants_probe::probe()),
            _ => "unavailable".to_string(),
        }
    }
}

fn on_path(name: &str) -> bool {
    Command::new("which").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
}

fn name_on_path(tool: ProbeTool) -> &'static str {
    match tool {
        ProbeTool::Cargo => "cargo",
        ProbeTool::CargoFmt => "cargo-fmt",
        ProbeTool::CargoClippy => "cargo-clippy",
        ProbeTool::CargoDeny => "cargo-deny",
        ProbeTool::CargoAudit => "cargo-audit",
        ProbeTool::CargoLlvmCov => "cargo-llvm-cov",
        ProbeTool::CargoMutants => "cargo-mutants",
        ProbeTool::Named(n) => n,
    }
}
