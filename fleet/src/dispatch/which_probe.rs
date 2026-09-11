//! `WhichProbe`: the real `ToolProbe` impl, split out of `verify_ports.rs` to keep that file
//! under the 80-line cap.

use super::mutants_probe;
use super::tool_path;
use fleet_verify::{ProbeTool, ToolProbe};

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

    /// Names the tool AND everywhere it was looked for. A bare "unavailable" sent a user with
    /// cargo installed under `~/.cargo/bin` (rustup's default, absent from a non-login shell's
    /// `PATH`) off to reinstall a tool they already had -- see `tool_path`.
    fn unavailable_reason(&self, tool: ProbeTool) -> String {
        match tool {
            ProbeTool::CargoMutants => mutants_probe::reason(mutants_probe::probe()),
            other => format!("not found: no `{}` in {}", name_on_path(other), tool_path::searched()),
        }
    }
}

fn on_path(name: &str) -> bool {
    tool_path::found(name)
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
