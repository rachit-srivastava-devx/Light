use offline::{
    emit_validated_lesson, evaluate, EvaluationRequest, HeldOutTask, OfflineError, OfflineScore,
    Outcome, Recommendation, TrialRunner, Variant,
};

struct PassingRunner;
impl TrialRunner for PassingRunner {
    fn run(&self, task: &HeldOutTask, v: Variant) -> Result<Outcome, OfflineError> {
        Ok(Outcome { task_id: task.id.clone(), variant: v, passed: true })
    }
}

struct FailingRunner;
impl TrialRunner for FailingRunner {
    fn run(&self, _: &HeldOutTask, _: Variant) -> Result<Outcome, OfflineError> { Err(OfflineError::Unavailable("boom".into())) }
}

fn task(id: &str) -> HeldOutTask {
    HeldOutTask { id: id.into(), input: "x".into(), input_digest: format!("d-{}", id) }
}

fn req(ts: Vec<HeldOutTask>, min: u64) -> EvaluationRequest {
    EvaluationRequest { candidate_id: "c".into(), tasks: ts, min_pairs: min, seed: 0 }
}
#[test]
fn exact_pass_counts_both_arms() {
    let score = evaluate(&PassingRunner, req(vec![task("a"), task("b")], 1)).unwrap();
    assert_eq!(score.baseline_pass, 2);
    assert_eq!(score.variant_pass, 2);
    assert_eq!(score.checked, 2);
    assert_eq!(score.total, 2);
}
#[test]
fn zero_min_pairs_refused() {
    let r = evaluate(&PassingRunner, req(vec![task("a")], 0));
    assert!(matches!(r, Err(OfflineError::InsufficientPairs { .. })));
}
#[test]
fn insufficient_pairs_exact_counts() {
    let r = evaluate(&PassingRunner, req(vec![task("a")], 5));
    assert!(matches!(r, Err(OfflineError::InsufficientPairs { need: 5, got: 1 })));
}
#[test]
fn runner_error_propagates_unavailable() {
    let r = evaluate(&FailingRunner, req(vec![task("a")], 1));
    assert!(matches!(r, Err(OfflineError::Unavailable(_))));
}
#[test]
fn pair_mismatch_same_id_different_digest_refused() {
    let tasks = vec![
        HeldOutTask { id: "same".into(), input: "x".into(), input_digest: "d1".into() },
        HeldOutTask { id: "same".into(), input: "y".into(), input_digest: "d2".into() },
    ];
    assert!(matches!(evaluate(&PassingRunner, req(tasks, 1)), Err(OfflineError::PairMismatch)));
}
#[test]
fn emit_validated_lesson_promote_returns_some() {
    let score = OfflineScore {
        candidate_id: "c".into(),
        baseline_pass: 2,
        variant_pass: 3,
        checked: 3,
        total: 3,
        recommendation: Recommendation::Promote,
    };
    let lesson = emit_validated_lesson(&score);
    assert!(lesson.is_some());
    assert_eq!(lesson.unwrap().candidate_id, "c");
}
#[test]
fn emit_validated_lesson_reject_returns_none() {
    let score = OfflineScore {
        candidate_id: "c".into(),
        baseline_pass: 3,
        variant_pass: 0,
        checked: 3,
        total: 3,
        recommendation: Recommendation::Reject,
    };
    assert!(emit_validated_lesson(&score).is_none());
}