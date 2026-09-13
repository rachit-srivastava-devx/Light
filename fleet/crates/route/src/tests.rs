use super::{
    admit, BudgetSnapshot, Candidate, CandidateId, CatalogSnapshot, IntentSpec, PolicySnapshot,
    CONSERVATIVE_BASELINE,
};

fn open_policy() -> PolicySnapshot {
    PolicySnapshot {
        max_cost_per_call: u64::MAX,
        allowed_providers: vec![],
    }
}

#[test]
fn capability_filter_rejects_missing() {
    let candidate = Candidate {
        id: CandidateId("c1".into()),
        capabilities: vec!["basic".into()],
        historical_cost: Some(100),
        provider: "p1".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![candidate],
        digest: "d1".into(),
    };
    let intent = IntentSpec {
        required_capabilities: vec!["advanced".into()],
        task_class: "x".into(),
    };
    let budget = BudgetSnapshot {
        limit: 1000,
        spent: 0,
    };
    let result = admit(&intent, &snapshot, &budget, &open_policy());
    assert!(result.is_err());
    let stages = result.unwrap_err().stages();
    assert!(!stages.is_empty());
    assert_eq!(stages[0].stage, "capability");
}

#[test]
fn score_boundary_conservative_floor() {
    let candidate = Candidate {
        id: CandidateId("c1".into()),
        capabilities: vec![],
        historical_cost: None,
        provider: "p1".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![candidate],
        digest: "d1".into(),
    };
    let intent = IntentSpec {
        required_capabilities: vec![],
        task_class: "x".into(),
    };
    let budget = BudgetSnapshot {
        limit: u64::MAX,
        spent: 0,
    };
    let result = admit(&intent, &snapshot, &budget, &open_policy()).expect("should succeed");
    let score_stage = result
        .stages
        .iter()
        .find(|s| s.stage == "score")
        .expect("score stage missing");
    assert_eq!(score_stage.candidates_in, 1);
    // Absence of historical cost must use the configured baseline, not zero.
    assert_eq!(result.selected_cost, CONSERVATIVE_BASELINE);
}

#[test]
fn refusal_receipt_contains_first_empty_stage() {
    let snapshot = CatalogSnapshot {
        candidates: vec![],
        digest: "d1".into(),
    };
    let intent = IntentSpec {
        required_capabilities: vec!["x".into()],
        task_class: "y".into(),
    };
    let budget = BudgetSnapshot {
        limit: 1000,
        spent: 0,
    };
    let err = admit(&intent, &snapshot, &budget, &open_policy()).expect_err("should fail");
    let stages = err.stages();
    assert!(!stages.is_empty());
    assert_eq!(stages[0].stage, "capability");
    // Empty catalog: checked=0, total=0 per §8.
    assert_eq!(stages[0].candidates_in, 0);
}
