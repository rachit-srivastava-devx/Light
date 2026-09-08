//! Help-text augmentation for `Commands` variants, applied to the clap `Command` builder before
//! parsing so doc comments on the enum (which would push `root.rs` past its 80-line cap) are not
//! needed. First half of the 28 descriptions; see `help_text_ops.rs` for the rest.

use clap::Command;

pub fn with_descriptions(cmd: Command) -> Command {
    let cmd = cmd
        .mut_subcommand("meter", |c| c.about("Reserve (and optionally settle) a token budget for --lane."))
        .mut_subcommand("route", |c| c.about("Pick an adapter for a task via fleet-router's live capability state."))
        .mut_subcommand("roles", |c| c.about("List every known role with its bandwidth and owned gate."))
        .mut_subcommand("swarm", |c| c.about("Spawn a worker lane for --task in --repo under --role."))
        .mut_subcommand("sow", |c| c.about("Validate a --text SOW's structure and content for ambiguity."))
        .mut_subcommand("plan", |c| c.about("Print an acceptance-checks draft for --model."))
        .mut_subcommand("skills", |c| c.about("NOT IMPLEMENTED: fleet-worker's skills_registry module is private."))
        .mut_subcommand("role-check", |c| c.about("Check whether --role passes fleet-router's role gate."))
        .mut_subcommand("agents", |c| c.about("Resolve a hermetic agent provision for --agent-id in --repo."))
        .mut_subcommand("lifecycle", |c| c.about("Resume --task-id from Intake and advance it using --evidence."))
        .mut_subcommand("run", |c| c.about("Run the full durable pipeline for --task in --repo."))
        .mut_subcommand("oracle", |c| c.about("Run every real verify gate and exit on the aggregate verdict."))
        .mut_subcommand("adjudicate", |c| c.about("Judge an artifact with fleet-judge (needs --features llm7); abstain/failure exit non-zero."))
        .mut_subcommand("attest", |c| c.about("NOT IMPLEMENTED: fleet-types has the wire shape only, no builder fn."))
        .mut_subcommand("pr", |c| c.about("NOT IMPLEMENTED: fleet-merge has no pr-emit fn exposed yet."));
    super::help_text_ops::with_descriptions(cmd)
}
