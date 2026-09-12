use offline::{
    evaluate, EvaluationRequest, HeldOutTask, OfflineError, Outcome, Recommendation, TrialRunner,
    Variant,
};

// --- test runners ---

struct PassingRunner;

impl TrialRunner for PassingRunner {
    fn run(&self, task: &HeldOutTask, variant: Variant) -> Result<Outcome, OfflineError> {
        Ok(Outcome {
            task_id: task.id.clone(),
            variant,
            passed: true,
        })
    }
}

/// Baseline always passes; candidate arm always fails.
struct RejectingRunner;

impl TrialRunner for RejectingRunner {
    fn run(&self, task: &HeldOutTask, variant: Variant) -> Result<Outcome, OfflineError> {
        Ok(Outcome {
            task_id: task.id.clone(),
            variant,
            passed: matches!(variant, Variant::Baseline),
        })
    }
}

// --- helpers ---

fn tasks(n: usize) -> Vec<HeldOutTask> {
    (0..n)
        .map(|i| HeldOutTask {
            id: format!("t{i}"),
            input: format!("input-{i}"),
            input_digest: format!("digest-{i}"),
        })
        .collect()
}

fn req(ts: Vec<HeldOutTask>, seed: u64) -> EvaluationRequest {
    EvaluationRequest {
        candidate_id: "cand-1".into(),
        tasks: ts,
        min_pairs: 1,
        seed,
    }
}

// --- named integration tests ---

#[test]
fn lesson_candidate_evaluated_in_paired_trial() {
    let score = evaluate(&PassingRunner, req(tasks(3), 42)).unwrap();
    assert_eq!(score.candidate_id, "cand-1");
    assert!(score.checked > 0, "checked must be > 0");
    assert_eq!(score.checked, score.total, "checked must equal total");
}

#[test]
fn rejected_candidate_not_promoted() {
    let score = evaluate(&RejectingRunner, req(tasks(3), 42)).unwrap();
    assert!(
        !matches!(score.recommendation, Recommendation::Promote),
        "a candidate whose variant arm fails must not be promoted"
    );
}
