//! Arg groups for the routing/scheduling/intake family of subcommands. Field surface is a
//! pragmatic subset of the real flags `main.rs` parses today (repo/task/role/model), not a
//! byte-for-byte port of every historical flag -- flagged in the return, not silently claimed
//! complete (BLUEPRINT §5 covers the full historical flag set per subcommand).

use clap::Args;

#[derive(Args, Debug)]
pub struct MeterArgs {
    #[arg(long)]
    pub lane: String,
    #[arg(long)]
    pub cost_est: u64,
    #[arg(long)]
    pub settle: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    #[arg(long)]
    pub role: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct SwarmArgs {
    #[arg(long)]
    pub repo: String,
    /// The lane's task id, and (unless `--prompt` is also given) the free-text instructions
    /// sent to the worker. Passing `--task` alone is enough to run a lane.
    #[arg(long)]
    pub task: String,
    #[arg(long)]
    pub role: String,
    /// Optional: free-text worker instructions, overriding `--task`'s text for that purpose
    /// only (the task id stays `--task`'s value either way). Defaults to `--task`'s value.
    #[arg(long, default_value = "")]
    pub prompt: String,
}

#[derive(Args, Debug)]
pub struct SowArgs {
    #[arg(long)]
    pub text: String,
    #[arg(long)]
    pub intent_hash: String,
}

#[derive(Args, Debug)]
pub struct PlanArgs {
    #[arg(long, default_value = "fleet-cli")]
    pub model: String,
}

#[derive(Args, Debug)]
pub struct RoleCheckArgs {
    #[arg(long)]
    pub role: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct AgentsArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub agent_id: String,
    #[arg(long)]
    pub json: bool,
}
