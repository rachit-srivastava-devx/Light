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

pub fn status(json: bool) -> Result<(), DispatchError> {
    let cap = ConcurrencyCap::from_env(usize::MAX, 3);
    if json {
        crate::print::json::print_pretty(&cap.get());
    } else {
        human::line("concurrency_cap", cap.get());
    }
    Ok(())
}

pub fn doctor() -> Result<(), DispatchError> {
    for tool in ["cargo", "git"] {
        let ok = std::process::Command::new("which").arg(tool).output().map(|o| o.status.success()).unwrap_or(false);
        human::line(tool, if ok { "found" } else { "missing" });
    }
    Ok(())
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
        Commands::Adjudicate { .. } => "fleet-verify has no adjudication-table fn exposed yet",
        Commands::Skills { .. } => "fleet-worker's skills_registry module is private",
        _ => "not wired in this pass",
    };
    DispatchError::NotYetImplemented(reason)
}
