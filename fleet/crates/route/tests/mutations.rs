// Authority predicate tests — each test catches a named mutation target.
use route::{
    admit, BudgetSnapshot, Candidate, CandidateId, CatalogSnapshot, IntentSpec, PolicySnapshot,
    CONSERVATIVE_BASELINE,
};

fn open_policy() -> PolicySnapshot {
    PolicySnapshot {
        max_cost_per_call: u64::MAX,
        allowed_providers: vec![],
    }
}

fn big_budget() -> BudgetSnapshot {
    BudgetSnapshot {
        limit: u64::MAX,
        spent: 0,
    }
}

fn any_intent() -> IntentSpec {
    IntentSpec {
        required_capabilities: vec![],
        task_class: "t".into(),
    }
}

/// Catches mutation: replace min_by_key(cost) with .first() in score_candidate.
/// The expensive candidate is first; the cheap one must win.
#[test]
fn score_picks_cheapest_not_first() {
    let expensive = Candidate {
        id: CandidateId("c1-expensive".into()),
        capabilities: vec![],
        historical_cost: Some(500),
        provider: "p".into(),
    };
    let cheap = Candidate {
        id: CandidateId("c2-cheap".into()),
        capabilities: vec![],
        historical_cost: Some(100),
        provider: "p".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![expensive, cheap],
        digest: "d".into(),
    };
    let result =
        admit(&any_intent(), &snapshot, &big_budget(), &open_policy()).expect("should succeed");
    assert_eq!(result.selected, CandidateId("c2-cheap".into()));
    assert_eq!(result.selected_cost, 100);
}

/// Catches mutation: replace CONSERVATIVE_BASELINE with 0 in apply_conservative_baseline.
/// Unknown cost must be scored as CONSERVATIVE_BASELINE, not zero.
#[test]
fn conservative_baseline_used_not_zero() {
    let c = Candidate {
        id: CandidateId("unknown-cost".into()),
        capabilities: vec![],
        historical_cost: None,
        provider: "p".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![c],
        digest: "d".into(),
    };
    let result =
        admit(&any_intent(), &snapshot, &big_budget(), &open_policy()).expect("should succeed");
    assert_eq!(result.selected_cost, CONSERVATIVE_BASELINE);
    // Compile-time guard: baseline is non-zero so the mutation 0 is distinguishable.
    const _: () = assert!(CONSERVATIVE_BASELINE > 0);
}

/// Catches mutation: policy filter omitted — a disallowed provider must not reach scoring.
#[test]
fn policy_filter_rejects_disallowed_provider() {
    let c = Candidate {
        id: CandidateId("c1".into()),
        capabilities: vec![],
        historical_cost: Some(10),
        provider: "blocked-provider".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![c],
        digest: "d".into(),
    };
    let policy = PolicySnapshot {
        max_cost_per_call: u64::MAX,
        allowed_providers: vec!["allowed-provider".into()],
    };
    let err = admit(&any_intent(), &snapshot, &big_budget(), &policy)
        .expect_err("blocked provider must be refused");
    let stages = err.stages();
    assert!(!stages.is_empty());
    assert_eq!(stages[0].stage, "policy");
}

/// Catches mutation: budget filter omitted — a candidate over budget must not reach scoring.
#[test]
fn budget_filter_rejects_over_budget() {
    let c = Candidate {
        id: CandidateId("expensive".into()),
        capabilities: vec![],
        historical_cost: Some(1000),
        provider: "p".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![c],
        digest: "d".into(),
    };
    let budget = BudgetSnapshot {
        limit: 500,
        spent: 0,
    };
    let err = admit(&any_intent(), &snapshot, &budget, &open_policy())
        .expect_err("over-budget candidate must be refused");
    let stages = err.stages();
    assert!(!stages.is_empty());
    assert_eq!(stages[0].stage, "budget");
}

/// Reservation must identify the same candidate that was selected,
/// and its expiry must be in the future (catches + -> - mutation on reserved_until).
#[test]
fn reservation_matches_selected_and_is_future() {
    let c = Candidate {
        id: CandidateId("winner".into()),
        capabilities: vec![],
        historical_cost: Some(42),
        provider: "p".into(),
    };
    let snapshot = CatalogSnapshot {
        candidates: vec![c],
        digest: "d".into(),
    };
    let before = std::time::Instant::now();
    let result =
        admit(&any_intent(), &snapshot, &big_budget(), &open_policy()).expect("should succeed");
    assert_eq!(result.reservation.candidate_id, result.selected);
    // reserved_until must be strictly after the call — catches + replaced with -.
    assert!(result.reservation.reserved_until > before);
}
