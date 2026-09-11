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
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub task: String,
    /// Emit the machine-readable `PipelineOutcome` JSON on stdout instead of the human
    /// structured summary on stderr. Byte-identical to the pre-existing always-JSON behaviour.
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

/// Open a GitHub PR against `--repo`, attaching the last passing run's receipt for `--task`
/// from the ledger to the PR body. `--dry-run` prints the resolved `gh pr create` invocation
/// and composed body instead of shelling out -- the review-before-push path.
#[derive(Args, Debug)]
pub struct PrArgs {
    /// The target repo (a git worktree). Its origin remote is parsed for `owner/name`.
    #[arg(long)]
    pub repo: String,
    /// The task id whose last passing receipt is attached.
    #[arg(long)]
    pub task: String,
    /// PR title.
    #[arg(long)]
    pub title: String,
    /// PR base branch. Defaults to the repo's default branch (`gh repo view`).
    #[arg(long)]
    pub base: Option<String>,
    /// PR head branch. Defaults to the current branch of `--repo` (`git symbolic-ref`).
    #[arg(long)]
    pub head: Option<String>,
    /// Optional extra body. If given, the receipt block is appended below it.
    #[arg(long)]
    pub body: Option<String>,
    /// Print the resolved `gh` command + body and exit; do not shell out.
    #[arg(long)]
    pub dry_run: bool,
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
