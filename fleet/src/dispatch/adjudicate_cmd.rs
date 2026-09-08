//! `fleet adjudicate`: `fleet_judge::judge` over one real `JudgeModel` -> print (`adjudicate_render.rs`).
//! `select_model` picks the real keyless adapter behind the `llm7` cargo feature (passthrough in
//! `src/Cargo.toml`); with it OFF this fails with `AdjudicateCmdError::NoJudgeConfigured` --
//! never a fabricated verdict, never a silent default judge. `run_with_model` is the seam a test
//! drives with a fake `JudgeModel`, with zero network, so the pure judge core stays reachable
//! from the default test suite even though the real adapter is feature-gated off by default.

use crate::dispatch::adjudicate_cmd_error::AdjudicateCmdError;
use crate::dispatch::adjudicate_render::render;
use crate::dispatch::error::DispatchError;
use fleet_judge::{judge, Candidate, Criteria, JudgeModel, Verdict};
use std::fs;

fn criteria() -> Criteria {
    Criteria {
        instructions: "Judge whether the given artifact satisfies its stated requirements. \
            Choose `pass` only if it clearly does, `fail` if it clearly does not; abstain if \
            you cannot tell from the text alone."
            .to_string(),
        labels: vec!["pass".to_string(), "fail".to_string()],
    }
}

#[cfg(feature = "llm7")]
fn select_model() -> Result<Box<dyn JudgeModel>, AdjudicateCmdError> {
    Ok(Box::new(fleet_judge::llm7::Llm7Judge::new()))
}

#[cfg(not(feature = "llm7"))]
fn select_model() -> Result<Box<dyn JudgeModel>, AdjudicateCmdError> {
    Err(AdjudicateCmdError::NoJudgeConfigured)
}

pub fn adjudicate(artifact: String, json: bool) -> Result<(), DispatchError> {
    let text = fs::read_to_string(&artifact)
        .map_err(|source| AdjudicateCmdError::ArtifactUnreadable { path: artifact.clone(), source })?;
    // `dispatch::run` is always called from inside `main`'s multi-thread tokio runtime
    // (`tokio_rt.block_on(...)`, see `main.rs`); `Llm7Judge` owns a `reqwest::blocking::Client`,
    // which builds its own internal runtime and panics if that runtime is built OR dropped from
    // inside another runtime's worker thread without `block_in_place` -- so both the model's
    // construction and its eventual drop must happen inside the same `block_in_place` closure,
    // not just the call that uses it.
    tokio::task::block_in_place(|| {
        let model = select_model()?;
        run_with_model(&text, json, model.as_ref())
    })
    .map_err(DispatchError::from)
}

/// Drives `judge` over an injected model and renders the result -- the seam tests use with a
/// fake `JudgeModel` (see `adjudicate_cmd_tests.rs`).
fn run_with_model(text: &str, json: bool, model: &dyn JudgeModel) -> Result<(), AdjudicateCmdError> {
    let verdict = judge(&criteria(), &Candidate { input: text.to_string() }, model)?;
    render(&verdict, json);
    match verdict {
        Verdict::Decided { .. } => Ok(()),
        Verdict::Abstain { why } => Err(AdjudicateCmdError::Abstained(why)),
    }
}

#[cfg(test)]
#[path = "adjudicate_cmd_tests.rs"]
mod tests;
