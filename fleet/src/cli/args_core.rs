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
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    #[arg(long)]
    pub role: Option<String>,
}

#[derive(Args, Debug)]
pub struct SwarmArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub task: String,
    #[arg(long)]
    pub role: String,
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
}

#[derive(Args, Debug)]
pub struct AgentsArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub agent_id: String,
}
