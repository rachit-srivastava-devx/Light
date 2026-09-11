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
    /// Opt in to merging the lane's worktree branch back into this repo's `HEAD` if (and only
    /// if) it finishes `Done`. Off by default -- a `Refused` or `EnvironmentFault` lane is never
    /// merged. This WRITES to the target branch given by `--repo`; only pass it when you mean
    /// to update that branch.
    #[arg(long)]
    pub merge: bool,
    /// Chain `fleet run` (verify pipeline) against the same `--repo` and `--task` immediately
    /// after the lane finishes `Done`. Off by default. A lane that ends `Refused` or
    /// `EnvironmentFault` short-circuits: verify is NOT invoked and swarm's exit code is
    /// preserved. When the lane finishes `Done`, the final exit code becomes the run pipeline's
    /// outcome. Works with `--merge false`: verify grades the worktree, not the main branch.
    #[arg(long)]
    pub then_verify: bool,
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
