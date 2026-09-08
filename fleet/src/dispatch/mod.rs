//! Top-level dispatch: match the parsed `Commands`, call the one `*_cmd` fn that owns it. This
//! is the only place a `Commands` variant is matched against a real handler.

pub mod agents_cmd;
pub mod context_cmd;
pub mod error;
pub mod ledger_cmd;
pub mod lifecycle_cmd;
pub mod meter_cmd;
pub mod ops_cmd;
pub mod plan_cmd;
pub mod route_cmd;
pub mod run_cmd;
pub mod swarm_cmd;
pub mod verify_cmd;
pub mod worker_cmd;

use crate::cli::Commands;
use error::DispatchError;
use std::path::Path;

pub fn run(command: Commands, state_dir: &Path) -> Result<(), DispatchError> {
    match command {
        Commands::Meter(a) => meter_cmd::meter(state_dir, a),
        Commands::Route(a) => route_cmd::route(a),
        Commands::Roles => route_cmd::roles(),
        Commands::Swarm(a) => swarm_cmd::swarm(a),
        Commands::Sow(a) => plan_cmd::sow(a),
        Commands::Plan(a) => plan_cmd::plan(a),
        Commands::RoleCheck(a) => route_cmd::role_check(a),
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
        Commands::Status { json } => ops_cmd::status(json),
        Commands::Doctor => ops_cmd::doctor(),
        Commands::Version => ops_cmd::version(),
        Commands::Completions { shell } => ops_cmd::completions(shell),
        Commands::PipelineProbe { task_id } => run_cmd::pipeline_probe(state_dir, task_id),
        other @ (Commands::Console { .. }
        | Commands::Freeze(_)
        | Commands::Contract(_)
        | Commands::Pr(_)
        | Commands::Attest(_)
        | Commands::Adjudicate { .. }
        | Commands::Skills { .. }) => Err(ops_cmd::not_yet_implemented(&other)),
    }
}
