//! `fleet status|doctor|version|completions`: composition-root's own job (BLUEPRINT §2 --
//! printing/generation, no crate delegation needed) plus `fleet console|freeze|contract|pr|
//! attest|adjudicate|skills`, flagged `NotYetImplemented` where their owning crate (BLUEPRINT
//! §2's non-goals table) exposes no entry point reachable without inventing business logic here.

use crate::cli::{Cli, Commands};
use crate::dispatch::error::DispatchError;
use crate::print::human;
use crate::runtime::ConcurrencyCap;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

/// `status`'s payload as an actual object -- previously `--json` serialized the bare `usize` from
/// `cap.get()`, so `fleet status --json` emitted the scalar `3`, unparseable by any caller
/// expecting an object (D3 in the E2E findings).
#[derive(serde::Serialize)]
struct StatusReport {
    concurrency_cap: usize,
}

/// Takes the cap `main.rs` already computed from a REAL measurement. It used to recompute its own
/// with `ConcurrencyCap::from_env(usize::MAX, 3)` -- "ignore RAM entirely" -- so `status` reported
/// 3 while the measured preflight cap was 2. Reporting an unmeasured number next to a measured
/// gate is how a check becomes cosmetic; the cap is passed in now, never re-derived.
pub fn status(json: bool, cap: ConcurrencyCap) -> Result<(), DispatchError> {
    let report = StatusReport { concurrency_cap: cap.get() };
    if json {
        crate::print::json::print_pretty(&report);
    } else {
        human::line("concurrency_cap", report.concurrency_cap);
    }
    Ok(())
}

/// "no crashing again": folds the capacity preflight into `doctor`. `--json` builds the same
/// facts as a `DoctorReport` (`doctor_json.rs`) instead of printing human lines.
pub fn doctor(json: bool) -> Result<(), DispatchError> {
    use super::doctor_json::{build, probe};
    let ((cargo, cargo_where), (git, git_where)) = (probe("cargo"), probe("git"));
    let optional = super::doctor_optional::collect();
    if json {
        crate::print::json::print_pretty(&build(cargo, cargo_where, git, git_where, &optional));
        return Ok(());
    }
    let id = crate::build_info::IDENTITY;
    human::line("cargo", if cargo { format!("found ({cargo_where})") } else { cargo_where });
    human::line("git", if git { format!("found ({git_where})") } else { git_where });
    human::line("commit_sha", id.commit_sha);
    human::line("tree_state", id.tree_state);
    human::line("build_time", id.build_time);
    crate::dispatch::capacity_probe_cmd::report(3, None)?;
    super::doctor_optional::print(&optional);
    Ok(())
}

pub use super::ops_version::version;

pub fn completions(shell: Shell) -> Result<(), DispatchError> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}

/// One named refusal per subcommand with no reachable owning-crate entry point -- see BLUEPRINT
/// §2's non-goals table for who owns each (fleet-stream/console, fleet-worker/skills-registry,
/// fleet-merge/pr-emit, fleet-verify/adjudication-table).
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
