use offline::{
    evaluate, EvaluationRequest, HeldOutTask, OfflineError, Outcome, TrialRunner, Variant,
};

struct PassingRunner;
impl TrialRunner for PassingRunner {
    fn run(&self, task: &HeldOutTask, variant: Variant) -> Result<Outcome, OfflineError> {
        Ok(Outcome { task_id: task.id.clone(), variant, passed: true })
    }
}

fn tasks(n: usize) -> Vec<HeldOutTask> {
    (0..n).map(|i| HeldOutTask {
        id: format!("t{i}"),
        input: format!("input-{i}"),
        input_digest: format!("digest-{i}"),
    }).collect()
}

fn req(ts: Vec<HeldOutTask>, seed: u64) -> EvaluationRequest {
    EvaluationRequest { candidate_id: "cand-1".into(), tasks: ts, min_pairs: 1, seed }
}

// Kills: evaluate body → constant stub (same seed must produce same output)
#[test]
fn evaluation_is_deterministic() {
    let s1 = evaluate(&PassingRunner, req(tasks(3), 42)).unwrap();
    let s2 = evaluate(&PassingRunner, req(tasks(3), 42)).unwrap();
    assert_eq!(s1.baseline_pass, s2.baseline_pass);
    assert_eq!(s1.variant_pass, s2.variant_pass);
    assert_eq!(s1.checked, s2.checked);
    assert_eq!(s1.total, s2.total);
    assert_eq!(s1.recommendation, s2.recommendation);
    assert_eq!(s1.candidate_id, s2.candidate_id);
}
