//! Second half of `with_descriptions` (see `help_text.rs`) -- split out to keep each file ≤80
//! lines, per the hard per-file line-count rule this repo enforces.

use clap::Command;

pub fn with_descriptions(cmd: Command) -> Command {
    cmd.mut_subcommand("status", |c| c.about("Print the measured concurrency cap this process runs under."))
        .mut_subcommand("rollback", |c| c.about("Remove --worktree from --repo, refusing if it is not a real worktree."))
        .mut_subcommand("ledger", |c| c.about("Print the ledger row count, or verify its hash chain with --verify."))
        .mut_subcommand("contract", |c| c.about("NOT IMPLEMENTED: no crate in the roster names Contract ownership."))
        .mut_subcommand("gate", |c| c.about("Run one gate by --id (or all with no --id) and exit on its verdict."))
        .mut_subcommand("freeze", |c| c.about("NOT IMPLEMENTED: no crate in the roster names Freeze ownership."))
        .mut_subcommand("console", |c| c.about("NOT IMPLEMENTED: fleet-stream's console/dashboard sink is unwired."))
        .mut_subcommand("graph", |c| c.about("Walk --repo (skipping build/VCS dirs) and print its symbol graph size."))
        .mut_subcommand("impact", |c| c.about("Count how many symbols in the current repo match --symbol."))
        .mut_subcommand("mcp", |c| c.about("NOT IMPLEMENTED: fleet-worker's sandbox manifest fn is not public."))
        .mut_subcommand("completions", |c| c.about("Print a shell completion script for the given shell."))
        .mut_subcommand("doctor", |c| c.about("Check cargo/git are on PATH and print the capacity preflight verdict."))
        .mut_subcommand("version", |c| c.about("Print the fleet-cli package version."))
}
