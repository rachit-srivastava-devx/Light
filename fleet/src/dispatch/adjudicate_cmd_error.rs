//! `AdjudicateCmdError` -- everything `fleet adjudicate` can fail on, mapped to a distinct
//! `ExitCode`. Mirrors `AgentCmdError`'s split (`agent_cmd_error.rs`): the detailed enum lives
//! here so `error.rs` only needs one `#[from]` wrapper line, keeping it under the ≤80-line rule.

use fleet_judge::JudgeError;
use fleet_types::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum AdjudicateCmdError {
    /// The `llm7` cargo feature is off, so no `JudgeModel` is wired -- never a fabricated
    /// verdict, never a silent default judge. Unreachable when built `--features llm7`.
    #[allow(dead_code)]
    #[error(
        "no judge model is configured: build with `--features llm7` to enable the real \
         keyless adapter (`fleet-judge`'s `Llm7Judge`), or wire your own `JudgeModel`"
    )]
    NoJudgeConfigured,
    /// The artifact path could not be read.
    #[error("could not read artifact {path:?}: {source}")]
    ArtifactUnreadable { path: String, source: std::io::Error },
    /// The judge core itself refused/faulted (`fleet_judge::judge`'s own typed errors).
    #[error(transparent)]
    Judge(#[from] JudgeError),
    /// The judge model produced an honest `Verdict::Abstain` -- not a pass, not an error, but
    /// still not a verdict this command can act on, so it is surfaced and the process exits
    /// non-zero.
    #[error("judge abstained: {0}")]
    Abstained(String),
}

impl AdjudicateCmdError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            AdjudicateCmdError::NoJudgeConfigured => ExitCode::Env,
            AdjudicateCmdError::ArtifactUnreadable { .. } => ExitCode::Env,
            AdjudicateCmdError::Judge(_) => ExitCode::Invariant,
            // A gate deliberately declining to say "yes" is the expected, auditable "no" --
            // the same class of outcome `ExitCode::Refusal` exists for elsewhere in this repo.
            AdjudicateCmdError::Abstained(_) => ExitCode::Refusal,
        }
    }
}
