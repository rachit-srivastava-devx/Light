//! Arg groups for lifecycle/verify/ledger/merge subcommands. Same pragmatic-subset caveat as
//! `args_core.rs`.

use clap::Args;

#[derive(Args, Debug)]
pub struct LifecycleArgs {
    #[arg(long)]
    pub task_id: String,
    #[arg(long)]
    pub evidence: String,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    #[arg(long)]
    pub repo: String,
    #[arg(long)]
    pub task: String,
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
}
