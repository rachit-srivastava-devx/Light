//! Top-level dispatch: match the parsed `Commands`, call the one `*_cmd` fn that owns it.

#[path = "adjudicate/adjudicate_cmd.rs"]
mod adjudicate_cmd;
#[path = "adjudicate/adjudicate_cmd_error.rs"]
mod adjudicate_cmd_error;
#[path = "adjudicate/adjudicate_render.rs"]
mod adjudicate_render;
#[path = "agent/agent_cmd.rs"]
pub mod agent_cmd;
#[path = "agent/agent_cmd_error.rs"]
mod agent_cmd_error;
#[path = "agent/agent_cmd_run.rs"]
pub(crate) mod agent_cmd_run;
#[path = "agent/agents_cmd.rs"]
pub mod agents_cmd;
#[path = "run/capacity_probe_cmd.rs"]
pub mod capacity_probe_cmd;
#[path = "ops/context_cmd.rs"]
pub mod context_cmd;
#[path = "ops/doctor_json.rs"]
mod doctor_json;
#[path = "ops/doctor_optional.rs"]
mod doctor_optional;
#[path = "ops/error.rs"]
pub mod error;
#[path = "ops/error_exit.rs"]
mod error_exit;
#[path = "verify/gate_config.rs"]
mod gate_config;
#[path = "verify/gate_config_file.rs"]
mod gate_config_file;
#[path = "ops/ledger_cmd.rs"]
pub mod ledger_cmd;
#[path = "ops/lifecycle_cmd.rs"]
pub mod lifecycle_cmd;
pub mod meter_cmd;
#[path = "run/mutants_probe.rs"]
mod mutants_probe;
#[path = "ops/ops_cmd.rs"]
pub mod ops_cmd;
// `fleet version` + build identity, split out of `ops_cmd` for its 80-line gate
#[path = "ops/ops_version.rs"]
mod ops_version;
#[path = "run/plan_cmd.rs"]
pub mod plan_cmd;
#[path = "run/planahead_cmd.rs"]
pub mod planahead_cmd;
// `fleet pr`, split out of `ops_cmd` for its 80-line gate
#[path = "ops/pr_cmd.rs"]
mod pr_cmd;
#[path = "ops/role_cmd.rs"]
pub mod role_cmd;
#[path = "ops/route_cmd.rs"]
pub mod route_cmd;
#[path = "run/run_cmd.rs"]
pub mod run_cmd;
#[path = "runtime_snapshot.rs"]
mod runtime_snapshot;
// `fleet run-modules` arg parsing, split out of `mod.rs` for its 80-line gate
#[path = "run/run_modules_cmd.rs"]
mod run_modules_cmd;
#[path = "run/run_report.rs"]
mod run_report;
#[path = "run/sow_probes.rs"]
mod sow_probes;
#[path = "run/spawn_probe_cmd.rs"]
pub mod spawn_probe_cmd;
#[path = "run/swarm_cmd.rs"]
pub mod swarm_cmd;
#[path = "ops/tool_path.rs"]
pub(crate) mod tool_path;
#[path = "verify/verify_cmd.rs"]
pub mod verify_cmd;
/// `pub(crate)`: `pipeline::verify_stage` reuses this module's real `WhichProbe`/`RealRunner`.
#[path = "verify/verify_ports.rs"]
pub(crate) mod verify_ports;
#[path = "verify/verify_repo.rs"]
mod verify_repo;
#[path = "verify/verify_report.rs"]
mod verify_report;
#[path = "verify/verify_runner_bounded.rs"]
mod verify_runner_bounded;
#[path = "verify/verify_runner_io.rs"]
mod verify_runner_io;
#[path = "ops/walk.rs"]
mod walk;
#[path = "ops/walk_error.rs"]
mod walk_error;
#[path = "verify/which_probe.rs"]
pub(crate) mod which_probe;
#[path = "ops/worker_cmd.rs"]
pub mod worker_cmd;
use cli::Commands;
use runtime::ConcurrencyCap;
use error::DispatchError;
use std::path::Path;

pub async fn run(
    command: Commands,
    state_dir: &Path,
    cap: ConcurrencyCap,
) -> Result<(), DispatchError> {
    match command {
        Commands::Meter(a) => meter_cmd::meter(state_dir, a),
        Commands::Route(a) => route_cmd::route(a),
        Commands::Roles(a) => role_cmd::roles(a.json),
        Commands::Swarm(a) => swarm_cmd::swarm(state_dir, a),
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
        Commands::RunModules(a) => {
            run_cmd::run_modules(
                state_dir,
                Path::new(&a.repo),
                run_modules_cmd::parse_modules_from_args(&a)?,
            )
            .await
        }
        Commands::PipelineProbe { task_id, repo } => {
            run_cmd::pipeline_probe(state_dir, repo, task_id)
        }
        Commands::PlanAheadProbe(a) => planahead_cmd::probe(state_dir, a),
        Commands::Agent(a) => agent_cmd::agent(a).map_err(DispatchError::from),
        Commands::SpawnProbe(a) => spawn_probe_cmd::probe(a),
        Commands::CapacityProbe => capacity_probe_cmd::report(3, None),
        Commands::Adjudicate(a) => adjudicate_cmd::adjudicate(a.artifact, a.json),
        Commands::Pr(a) => pr_cmd::pr_emit(a),
        other @ (Commands::Console { .. }
        | Commands::Freeze(_)
        | Commands::Contract(_)
        | Commands::Attest(_)
        | Commands::Skills { .. }) => Err(ops_cmd::not_yet_implemented(&other)),
    }
}
