use offline::{
    evaluate, EvaluationRequest, HeldOutTask, OfflineError, Outcome, TrialRunner, Variant,
};
use std::sync::Mutex;

struct PassAll;
impl TrialRunner for PassAll {
    fn run(&self, task: &HeldOutTask, v: Variant) -> Result<Outcome, OfflineError> {
        Ok(Outcome {
            task_id: task.id.clone(),
            variant: v,
            passed: true,
        })
    }
}

/// Records task IDs in the order the runner is called.
struct OrderRecorder {
    seen: Mutex<Vec<String>>,
}
impl TrialRunner for OrderRecorder {
    fn run(&self, task: &HeldOutTask, v: Variant) -> Result<Outcome, OfflineError> {
        self.seen.lock().unwrap().push(task.id.clone());
        Ok(Outcome {
            task_id: task.id.clone(),
            variant: v,
            passed: true,
        })
    }
}

fn req(ts: Vec<HeldOutTask>, seed: u64) -> EvaluationRequest {
    EvaluationRequest {
        candidate_id: "c".into(),
        tasks: ts,
        min_pairs: 1,
        seed,
    }
}

fn task(id: &str, digest: &str) -> HeldOutTask {
    HeldOutTask {
        id: id.into(),
        input: "x".into(),
        input_digest: digest.into(),
    }
}

/// Same id AND same digest must NOT return PairMismatch.
/// Kills pair.rs:32:25 (match guard → true): that mutation always errors on seen ids.
#[test]
fn duplicate_task_same_digest_succeeds() {
    let tasks = vec![task("t0", "d1"), task("t0", "d1")];
    let result = evaluate(&PassAll, req(tasks, 0));
    assert!(
        result.is_ok(),
        "identical entries with same digest must succeed, got {result:?}"
    );
}

/// With seed=0 and 2 tasks:
///   original: j = (0+1) % (1+1) = 1 → swap(1,1) → order [t0, t1] → first task is t0
///   mutation: j = (0+1) % (1*1) = 0 → swap(1,0) → order [t1, t0] → first task is t1
/// Kills pair.rs:42:58 (+ → *).
#[test]
fn trial_order_is_seed_deterministic() {
    let tasks = vec![task("t0", "d0"), task("t1", "d1")];
    let recorder = OrderRecorder {
        seen: Mutex::new(Vec::new()),
    };
    evaluate(&recorder, req(tasks, 0)).unwrap();
    let seen = recorder.seen.lock().unwrap();
    assert_eq!(
        seen[0], "t0",
        "with seed=0 and 2 tasks, first trial must be t0"
    );
}
