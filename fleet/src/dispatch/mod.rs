//! Top-level dispatch: match the parsed `Commands`, call the one `*_cmd` fn that owns it. This
//! is the only place a `Commands` variant is matched against a real handler.

mod adjudicate_cmd;
mod adjudicate_cmd_error;
mod adjudicate_render;
pub mod agent_cmd;
mod agent_cmd_error;
mod agent_cmd_run;
pub mod agents_cmd;
pub mod capacity_probe_cmd;
pub mod context_cmd;
mod doctor_json;
pub mod error;
mod error_exit;
pub mod ledger_cmd;
pub mod lifecycle_cmd;
mod memory;
pub mod meter_cmd;
pub mod ops_cmd;
pub mod plan_cmd;
pub mod planahead_cmd;
pub mod role_cmd;
pub mod route_cmd;
pub mod run_cmd;
mod sow_probes;
pub mod spawn_probe_cmd;
pub mod swarm_cmd;
pub mod verify_cmd;
/// `pub(crate)`, not `mod`: `pipeline::verify_stage` reuses the same real `WhichProbe`/
/// `RealRunner` ports this module's `verify_cmd` uses, instead of a second hand-rolled pair.
mod verify_report;
pub(crate) mod verify_ports;
mod verify_runner_bounded;
mod walk;
mod walk_error;
pub mod worker_cmd;

use crate::cli::Commands;
use crate::runtime::ConcurrencyCap;
use error::DispatchError;
use std::path::Path;

pub fn run(command: Commands, state_dir: &Path, cap: ConcurrencyCap) -> Result<(), DispatchError> {
    match command {
        Commands::Meter(a) => meter_cmd::meter(state_dir, a),
        Commands::Route(a) => route_cmd::route(a),
        Commands::Roles(a) => role_cmd::roles(a.json),
        Commands::Swarm(a) => swarm_cmd::swarm(a),
        Commands::Sow(a) => plan_cmd::sow(state_dir, a),
        Commands::Plan(a) => plan_cmd::plan(a),
        Commands::RoleCheck(a) => role_cmd::role_check(a),
        Commands::Agents(a) => agents_cmd::agents(a),
        Commands::Lifecycle(a) => lifecycle_cmd::lifecycle(state_dir, a),
        Commands::Run(a) => run_cmd::run(state_dir, a),
        Commands::Oracle => verify_cmd::oracle(),
        Commands::Gate(a) => verify_cmd::gate(a),
        Commands::Ledger(a) => ledger_cmd::ledger(state_dir, a),
        Commands::Rollback(a) => ledger_cmd::rollback(a),
        Commands::Graph(a) => context_cmd::graph(a),
        Commands::Impact(a) => context_cmd::impact(a),
        Commands::Mcp(a) => worker_cmd::mcp(a),
        Commands::Status { json } => ops_cmd::status(json, cap),
        Commands::Doctor(a) => ops_cmd::doctor(a.json),
        Commands::Version => ops_cmd::version(),
        Commands::Completions { shell } => ops_cmd::completions(shell),
        Commands::PipelineProbe { task_id, repo } => run_cmd::pipeline_probe(state_dir, repo, task_id),
        Commands::PlanAheadProbe(a) => planahead_cmd::probe(state_dir, a),
        Commands::Agent(a) => agent_cmd::agent(a).map_err(DispatchError::from),
        Commands::SpawnProbe(a) => spawn_probe_cmd::probe(a),
        Commands::CapacityProbe => capacity_probe_cmd::report(3, None),
        Commands::Adjudicate(a) => adjudicate_cmd::adjudicate(a.artifact, a.json),
        other @ (Commands::Console { .. }
        | Commands::Freeze(_)
        | Commands::Contract(_)
        | Commands::Pr(_)
        | Commands::Attest(_)
        | Commands::Skills { .. }) => Err(ops_cmd::not_yet_implemented(&other)),
    }
}
