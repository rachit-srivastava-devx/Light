//! `assess()`/`merge_questions` determinism and the 4-question cap, end to end.

mod common;
use common::{input, qw, FixedProbe, SequentialRunner};

use fleet_scan::{assess, merge_questions, Assessment, GapSeverity, ProbeKind, ProbeOutcome, ProbeSet};

#[test]
fn all_four_clear_yields_clear() {
    let empty = |kind| FixedProbe { kind, outcome: ProbeOutcome::Questions(vec![]) };
    let business = empty(ProbeKind::Business);
    let technical = empty(ProbeKind::Technical);
    let memory = empty(ProbeKind::Memory);
    let research = empty(ProbeKind::Research);
    let probes = ProbeSet { business: &business, technical: &technical, memory: &memory, research: &research };
    let report = assess(&input("requirement"), &probes, &SequentialRunner);

    assert_eq!(report.result, Assessment::Clear);
    assert!(report.faults.is_empty());
}

#[test]
fn assess_never_returns_more_than_four_questions_end_to_end() {
    let words = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
    let make = |kind: ProbeKind, offset: usize| FixedProbe {
        kind,
        outcome: ProbeOutcome::Questions(
            words[offset..offset + 2].iter().map(|w| qw(w, GapSeverity::Medium, kind)).collect(),
        ),
    };
    let business = make(ProbeKind::Business, 0);
    let technical = make(ProbeKind::Technical, 2);
    let memory = make(ProbeKind::Memory, 4);
    let research = FixedProbe { kind: ProbeKind::Research, outcome: ProbeOutcome::Questions(vec![]) };
    let probes = ProbeSet { business: &business, technical: &technical, memory: &memory, research: &research };
    let report = assess(&input("requirement"), &probes, &SequentialRunner);

    match report.result {
        Assessment::Open(open) => assert!(open.as_slice().len() <= 4),
        Assessment::Clear => panic!("expected questions"),
    }
}

#[test]
fn merge_is_order_independent() {
    let a = qw("alpha topic", GapSeverity::High, ProbeKind::Business);
    let b = qw("beta topic", GapSeverity::Medium, ProbeKind::Technical);
    let c = qw("gamma topic", GapSeverity::Low, ProbeKind::Memory);
    let orders = [
        vec![a.clone(), b.clone(), c.clone()],
        vec![c.clone(), a.clone(), b.clone()],
        vec![b.clone(), c.clone(), a.clone()],
    ];
    let results: Vec<_> = orders.into_iter().map(merge_questions).collect();
    assert!(results.windows(2).all(|w| w[0] == w[1]));
}
