//! Arg groups for contract/gate/freeze/context/mcp subcommands. Same pragmatic-subset caveat as
//! `args_core.rs`.

use clap::Args;

#[derive(Args, Debug)]
pub struct ContractArgs {
    #[arg(long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct GateArgs {
    #[arg(long)]
    pub id: Option<String>,
    /// Defaults to the process's cwd, matching `graph`/`impact`. Threaded all the way to every
    /// spawned gate's `.current_dir()` -- see `verify_runner_bounded.rs` (S1 fix).
    #[arg(long, default_value = ".")]
    pub repo: String,
}

/// `fleet oracle --repo <path>`: previously accepted no `--repo` at all (S1), always verifying
/// the calling process's own cwd. Same default and same threading as `GateArgs::repo`.
#[derive(Args, Debug)]
pub struct OracleArgs {
    #[arg(long, default_value = ".")]
    pub repo: String,
}

#[derive(Args, Debug)]
pub struct FreezeArgs {
    #[arg(long)]
    pub path: String,
}

#[derive(Args, Debug)]
pub struct GraphArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ImpactArgs {
    /// Defaults to the process's cwd. Before this existed, `--repo` was REJECTED (clap exit 2)
    /// while the command silently analysed the cwd -- answering about a different repo than the
    /// user named. Every sibling command takes `--repo`; this one now does too.
    #[arg(long, default_value = ".")]
    pub repo: String,
    #[arg(long)]
    pub symbol: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    #[arg(long)]
    pub lease: String,
}

/// `fleet __planahead_probe`: drives `pipeline::planahead::run_plan_ahead` end to end through
/// the real binary, same shape as `__pipeline_probe`. `units` is comma-separated; `queue_capacity`
/// defaults to 2 so a probe with >=3 units observably exercises the backpressure bound.
#[derive(Args, Debug)]
pub struct PlanAheadProbeArgs {
    #[arg(long)]
    pub run_id: String,
    #[arg(long, value_delimiter = ',')]
    pub units: Vec<String>,
    #[arg(long, default_value_t = 2)]
    pub queue_capacity: usize,
}
