//! Arg groups for lifecycle/verify/ledger/merge subcommands. Same pragmatic-subset caveat as
//! `args_core.rs`.

use clap::Args;

/// Shared by the bare, arg-less subcommands (`roles`, `doctor`) that only need `--json`, so
/// adding it costs `root.rs` a `(JsonOnly)` on one existing line, not a whole struct body.
#[derive(Args, Debug)]
pub struct JsonOnly {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct LifecycleArgs {
    #[arg(long)]
    pub task_id: String,
    #[arg(long)]
    pub evidence: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// One or more repo paths. `--repo a` (single, back-compat) still works; `--repo a --repo b`
    /// (repeat) and `--repo a b` (variadic per occurrence) both parse to a two-element `Vec`.
    /// One task, N repos, aggregated verdict -- the Frido workspace's three sibling repos
    /// (posx-frido-{store,backend,admin}) is the shape driving this.
    #[arg(long = "repo", num_args = 1.., action = clap::ArgAction::Append, required = true)]
    pub repos: Vec<String>,
    #[arg(long)]
    pub task: String,
    /// Emit the machine-readable `PipelineOutcome` JSON on stdout instead of the human
    /// structured summary on stderr. Byte-identical to the pre-existing always-JSON behaviour.
    /// With N repos: N successive JSON objects (one per repo), in the given order.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct AdjudicateArgs {
    pub artifact: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct AttestArgs {
    #[arg(long)]
    pub artifact: String,
}

#[derive(Args, Debug)]
pub struct PrArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub branch: String,
}

#[derive(Args, Debug)]
pub struct RollbackArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub worktree: String,
}

#[derive(Args, Debug)]
pub struct LedgerArgs {
    #[arg(long)]
    pub verify: bool,
    #[arg(long)]
    pub json: bool,
}
