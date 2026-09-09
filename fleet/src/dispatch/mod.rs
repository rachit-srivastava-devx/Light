//! Top-level dispatch: match the parsed `Commands`, call the one `*_cmd` fn that owns it.

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
pub(crate) mod memory;
pub mod meter_cmd;
pub mod ops_cmd;
mod ops_version; // `fleet version` + build identity, split out of `ops_cmd` for its 80-line gate
pub mod plan_cmd;
pub mod planahead_cmd;
pub mod role_cmd;
pub mod route_cmd;
pub mod run_cmd;
mod mutants_probe; mod sow_probes; pub(crate) mod which_probe;
pub mod spawn_probe_cmd;
pub mod swarm_cmd;
pub mod verify_cmd;
mod verify_report;
mod verify_repo;
/// `pub(crate)`: `pipeline::verify_stage` reuses this module's real `WhichProbe`/`RealRunner`.
pub(crate) mod verify_ports;
mod verify_runner_bounded;
mod verify_runner_io;
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
        Commands::Oracle(a) => verify_cmd::oracle(a),
        Commands::Gate(a) => verify_cmd::gate(a),
        Commands::Ledger(a) => ledger_cmd::ledger(state_dir, a),
        Commands::Rollback(a) => ledger_cmd::rollback(a),
        Commands::Graph(a) => context_cmd::graph(a),
        Commands::Impact(a) => context_cmd::impact(a),
        Commands::Mcp(a) => worker_cmd::mcp(a),
        Commands::Status { json } => ops_cmd::status(json, cap),
        Commands::Doctor(a) => ops_cmd::doctor(a.json),
        Commands::Version(a) => ops_cmd::version(a.json),
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
