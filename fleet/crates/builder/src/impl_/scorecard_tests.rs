use super::*;

#[test]
fn unknown_outcome_is_published_but_never_credited() {
    let mut scorecard = Scorecard::empty("agent");
    scorecard.apply(ScorecardOutcome::Unknown);
    assert_eq!(scorecard.credited, 0);
    assert_eq!(scorecard.checked, 0);
    assert_eq!(scorecard.total, 1);
}

#[test]
fn scorecard_refresh_rejects_inconsistent_totals() {
    let corrupt = Scorecard {
        agent_id: "agent".to_string(),
        credited: 1,
        faulted: 0,
        unknown: 0,
        checked: 0, // wrong: should be 1
        total: 0,
    };
    assert!(!corrupt.consistent());

    let mut good = Scorecard::empty("agent");
    good.apply(ScorecardOutcome::Credited);
    assert!(good.consistent());
}
