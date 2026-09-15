//! `PipelineOutcome`/`PipelineError` -- the seam where every crate's own typed error is wrapped
//! into one error type so `run_pipeline` has a single `Result` to propagate (BLUEPRINT §3).

use super::records::{GateRecord, StageRecord};
use super::stage::PipelineStage;
use types::ExitCode;

/// `Scan`/`Plan` are still never constructed: `stages::{scan,plan}` remain pure wiring (BLUEPRINT
/// §2 non-goals -- no decision logic lives in `fleet-cli`) and have no real failure input fed to
/// them yet (the concrete adapters/probes that could fail are each owned by another crate and
/// deliberately not fabricated here, see `stages.rs`'s doc comment). `Event`/`Verify`/`Merge` ARE
/// constructed now: `event` fails on a real ledger-append error, `verify` on a real failing gate,
/// `merge` on real git/`integrate` state. Kept in the enum because BLUEPRINT §3 names them as
/// the seam's shape; `allow`d rather than deleted so wiring `Scan`/`Plan`'s real failure input
/// later is a one-line change, not a new variant.
#[allow(dead_code)]
#[derive(Debug)]
pub enum PipelineError {
    Event(String),
    Scan(String),
    Plan(String),
    Dispatch(route::Refusal),
    Verify(String),
    VerifyTyped {
        detail: String,
        code: ExitCode,
    },
    Merge(integrate::MergeRefusal),
    /// The step-log / durable-journal shim itself faulted (distinct from a stage's own
    /// business error). Named `Runtime` to match BLUEPRINT §3's `PipelineError::Runtime`.
    Runtime(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PipelineError {}

#[derive(Debug, serde::Serialize)]
pub struct PipelineOutcome {
    pub task: types::TaskId,
    pub final_stage: PipelineStage,
    #[serde(skip)]
    pub result: Result<(), PipelineError>,
    /// `Classify`'s real routing decision over the task text, or `None` if that stage never ran
    /// (crash-resumed past it) this invocation -- `stages` says which of the two it was.
    pub classification: Option<Box<route::Decision>>,
    /// Every stage, in order, with its outcome and real duration. The `--json` counterpart of the
    /// `> stage x` / `PASS stage x (1.23s)` lines the human path streams to stderr.
    pub stages: Vec<StageRecord>,
    /// Every gate `Verify` ran this invocation, with its verdict and published denominator.
    /// Empty when `Verify` did not run (refused earlier, or resumed past).
    pub gates: Vec<GateRecord>,
    /// The refusal reason, verbatim -- the same text the human path prints after `REFUSED`.
    pub refusal: Option<String>,
    /// `"git"` or `"no-git"`, driven by `fleet run --no-git` (absent that flag, always `"git"`).
    /// Lets a `--json` consumer tell "Merge ran" from "Merge was structurally unavailable" --
    /// a run that skipped merge must not read as a run that merged.
    pub repo_mode: &'static str,
}

/// What a stage handed back beyond pass/fail. Only `Classify` carries data today; every other
/// stage's success is fully described by `Ok(())`.
pub enum StageOutput {
    None,
    Classified(Box<route::Decision>),
    /// The stage was structurally unavailable this run (only `Merge`, only under `--no-git`) and
    /// was never attempted -- distinct from `None`'s "ran and succeeded".
    Skipped,
}
