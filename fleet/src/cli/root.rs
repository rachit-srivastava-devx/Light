//! `Cli`/`Commands` -- the clap derive replacement for `main.rs:57-252`'s hand-rolled `match`.
//! Every variant name is a real subcommand string from the current dispatcher (BLUEPRINT §5's
//! citation table); none invented, none dropped.

use super::args_core::*;
use super::args_ctx::*;
use super::args_ops::*;
use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(name = "fleet", version, about = "fleet-cli composition root")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Meter(MeterArgs),
    Route(RouteArgs),
    Roles,
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
    Oracle,
    Adjudicate {
        artifact: String,
    },
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
    Completions {
        shell: Shell,
    },
    Doctor,
    Version,
    /// Hidden, test-only: drives the real pipeline end to end. Preserves the property that
    /// `main.rs`'s `__repl`/`__agent`/`__lanes_probe`/`__pr_emit_probe` scaffolding drove real
    /// production code through the real binary (BLUEPRINT §5), narrowed to the one probe this
    /// pass needs.
    #[command(name = "__pipeline_probe", hide = true)]
    PipelineProbe {
        #[arg(long)]
        task_id: String,
    },
}
