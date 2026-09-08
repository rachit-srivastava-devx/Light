//! `fleet status|doctor|version|completions`: composition-root's own job (BLUEPRINT §2 --
//! printing/generation, no crate delegation needed) plus `fleet console|freeze|contract|pr|
//! attest|adjudicate|skills`, which are flagged `NotYetImplemented` where their owning crate
//! (per BLUEPRINT §2's non-goals table) has no public entry point this pass could reach without
//! inventing business logic here.

use crate::cli::{Cli, Commands};
use crate::dispatch::error::DispatchError;
use crate::print::human;
use crate::runtime::ConcurrencyCap;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

/// `status`'s payload as an actual object -- previously `--json` serialized the bare `usize`
/// from `cap.get()`, so `fleet status --json` emitted the scalar `3` instead of JSON with a
/// field name, unparseable by any caller expecting an object (D3 in the E2E findings).
#[derive(serde::Serialize)]
struct StatusReport {
    concurrency_cap: usize,
}

/// Takes the cap `main.rs` already computed from a REAL measurement. It used to recompute its own
/// with `ConcurrencyCap::from_env(usize::MAX, 3)` -- `usize::MAX` meaning "ignore RAM entirely" --
/// so `status` reported 3 while the measured preflight cap was 2. Reporting an unmeasured number
/// next to a measured gate is how a check becomes cosmetic; the cap is now passed in, never re-derived.
pub fn status(json: bool, cap: ConcurrencyCap) -> Result<(), DispatchError> {
    let report = StatusReport { concurrency_cap: cap.get() };
    if json {
        crate::print::json::print_pretty(&report);
    } else {
        human::line("concurrency_cap", report.concurrency_cap);
    }
    Ok(())
}

fn which(tool: &str) -> bool {
    std::process::Command::new("which").arg(tool).output().map(|o| o.status.success()).unwrap_or(false)
}

/// "no crashing again": folds the capacity preflight into `doctor`. `--json` builds the same
/// facts as a `DoctorReport` object (`doctor_json.rs`) instead of printing human lines.
pub fn doctor(json: bool) -> Result<(), DispatchError> {
    let (cargo, git) = (which("cargo"), which("git"));
    if json {
        crate::print::json::print_pretty(&super::doctor_json::build(cargo, git));
        return Ok(());
    }
    human::line("cargo", if cargo { "found" } else { "missing" });
    human::line("git", if git { "found" } else { "missing" });
    crate::dispatch::capacity_probe_cmd::report(3, None)
}

pub fn version() -> Result<(), DispatchError> {
    human::line("fleet-cli", env!("CARGO_PKG_VERSION"));
    Ok(())
}

pub fn completions(shell: Shell) -> Result<(), DispatchError> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}

/// One named refusal per subcommand this pass could not reach a real owning-crate entry point
/// for -- see BLUEPRINT §2's non-goals table for who owns each (fleet-stream/console,
/// fleet-worker/skills-registry, fleet-merge/pr-emit, fleet-verify/adjudication-table).
pub fn not_yet_implemented(command: &Commands) -> DispatchError {
    let reason = match command {
        Commands::Console { .. } => "fleet-stream's console/dashboard sink wiring",
        Commands::Freeze(_) => "no crate in the roster names Freeze ownership yet",
        Commands::Contract(_) => "no crate in the roster names Contract ownership yet",
        Commands::Pr(_) => "fleet-merge has no pr-emit fn exposed yet (worktree/merge only)",
        Commands::Attest(_) => "fleet-types has the wire shape only, no builder-flow fn yet",
        Commands::Skills { .. } => "fleet-worker's skills_registry module is private",
        _ => "not wired in this pass",
    };
    DispatchError::NotYetImplemented(reason)
}
