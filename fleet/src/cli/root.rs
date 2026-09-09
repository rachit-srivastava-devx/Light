//! `Cli`/`Commands` -- the clap derive replacement for `main.rs`'s hand-rolled `match`. Every
//! variant name is a real subcommand string from the current dispatcher (BLUEPRINT §5).

use super::args_agent::*;
use super::args_core::*;
use super::args_ctx::*;
use super::args_ops::*;
use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(name = "fleet", version, about = "fleet-cli composition root")]
pub struct Cli {
    /// Disable ANSI colour on the human path regardless of TTY (see `print::style`).
    #[arg(long, global = true)]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Meter(MeterArgs),
    Route(RouteArgs),
    Roles(JsonOnly),
    Swarm(SwarmArgs),
    Sow(SowArgs),
    Plan(PlanArgs),
    Skills {
        #[arg(long)]
        check: bool,
    },
    RoleCheck(RoleCheckArgs),
    Agents(AgentsArgs),
    Lifecycle(LifecycleArgs),
    Run(RunArgs),
    Oracle(OracleArgs),
    Adjudicate(AdjudicateArgs),
    Attest(AttestArgs),
    Pr(PrArgs),
    Status {
        #[arg(long)]
        json: bool,
    },
    Rollback(RollbackArgs),
    Ledger(LedgerArgs),
    Contract(ContractArgs),
    Gate(GateArgs),
    Freeze(FreezeArgs),
    Console {
        #[arg(long)]
        task: Option<String>,
    },
    Graph(GraphArgs),
    Impact(ImpactArgs),
    Mcp(McpArgs),
    Completions { shell: Shell },
    Doctor(JsonOnly),
    Version(JsonOnly),
    /// Hidden, test-only: drives the real pipeline end to end through the real binary.
    #[command(name = "__pipeline_probe", hide = true)]
    PipelineProbe {
        #[arg(long)]
        task_id: String,
        #[arg(long, default_value = ".")]
        repo: String,
    },
    #[command(name = "__planahead_probe", hide = true)]
    PlanAheadProbe(PlanAheadProbeArgs),
    /// Hidden, real: the child-side target `fleet-worker::child_command` spawns as `<exe>
    /// __agent <kind> <worktree> <task> [model]`. See `dispatch/agent_cmd.rs`.
    #[command(name = "__agent", hide = true)]
    Agent(AgentArgs),
    /// Hidden, test-only: drives `fleet_worker::spawn`/`join`'s real parent path end to end.
    #[command(name = "__spawn_probe", hide = true)]
    SpawnProbe(SpawnProbeArgs),
    /// Hidden: capacity reading + preflight verdict, no spawn (dispatch/capacity_probe_cmd.rs).
    #[command(name = "__capacity_probe", hide = true)]
    CapacityProbe,
}
