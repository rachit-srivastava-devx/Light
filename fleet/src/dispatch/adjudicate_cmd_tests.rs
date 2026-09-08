//! Drives `run_with_model` with a fake `JudgeModel` -- zero network, so the pure judge core
//! stays reachable from the default `cargo test` suite even though the real `llm7` adapter is
//! feature-gated off by default (see `adjudicate_cmd.rs`'s doc comment).

use super::*;
use fleet_judge::{ModelError, RawVerdict};

struct FakeModel(Result<RawVerdict, ModelError>);
impl JudgeModel for FakeModel {
    fn call(&self, _criteria: &Criteria, _candidate: &Candidate) -> Result<RawVerdict, ModelError> {
        self.0.clone()
    }
}

fn decided() -> RawVerdict {
    RawVerdict {
        label: Some("pass".into()),
        confidence_pct: Some(90),
        because: Some("meets the stated requirement".into()),
        abstain_why: None,
    }
}

#[test]
fn decided_verdict_is_ok() {
    let model = FakeModel(Ok(decided()));
    assert!(run_with_model("some artifact text", false, &model).is_ok());
}

/// Proves `--json`'s output is a parseable named-field object without depending on the live,
/// externally rate-limited `llm7` endpoint (see the task report's proof #7): run with
/// `--nocapture` and pipe stdout through `python3 -m json.tool`.
#[test]
fn decided_verdict_json_output_is_pretty_json() {
    let model = FakeModel(Ok(decided()));
    println!("--- json below ---");
    assert!(run_with_model("some artifact text", true, &model).is_ok());
}

#[test]
fn abstain_verdict_is_an_error_not_a_pass() {
    let raw = RawVerdict { abstain_why: Some("not enough context".into()), ..Default::default() };
    let model = FakeModel(Ok(raw));
    let err = run_with_model("some artifact text", false, &model).unwrap_err();
    assert!(matches!(err, AdjudicateCmdError::Abstained(_)), "{err:?}");
}

#[test]
fn model_failure_is_a_judge_error_not_a_fabricated_verdict() {
    let model = FakeModel(Err(ModelError::new("network unreachable")));
    let err = run_with_model("some artifact text", false, &model).unwrap_err();
    assert!(matches!(err, AdjudicateCmdError::Judge(_)), "{err:?}");
}
