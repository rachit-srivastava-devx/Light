//! Drop-no-why / dedup / sort / cap behavior from BLUEPRINT §6, §9.

mod common;
use common::q;

use fleet_scan::{merge_questions, Assessment, GapSeverity, ProbeKind, Question};

#[test]
fn merge_drops_empty_why() {
    let candidates = vec![q("only one", "", GapSeverity::High, ProbeKind::Business)];
    assert_eq!(merge_questions(candidates), Assessment::Clear);

    let candidates = vec![
        q("dropped", "   ", GapSeverity::High, ProbeKind::Business),
        q("kept", "has a reason", GapSeverity::Low, ProbeKind::Technical),
    ];
    match merge_questions(candidates) {
        Assessment::Open(open) => assert_eq!(open.as_slice().len(), 1),
        Assessment::Clear => panic!("expected one survivor"),
    }
}

#[test]
fn merge_dedups_above_jaccard_point_six() {
    let candidates = vec![
        q("what is the target audience", "why1", GapSeverity::Low, ProbeKind::Research),
        q("what is the target audience segment", "why2", GapSeverity::High, ProbeKind::Business),
        q("completely different unrelated topic here", "why3", GapSeverity::Medium, ProbeKind::Memory),
    ];
    match merge_questions(candidates) {
        Assessment::Open(open) => {
            let items = open.as_slice();
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].gap, GapSeverity::High);
            assert_eq!(items[0].probe, ProbeKind::Business);
        }
        Assessment::Clear => panic!("expected survivors"),
    }
}

#[test]
fn merge_sorts_by_descending_gap_severity() {
    let candidates = vec![
        q("low one", "w", GapSeverity::Low, ProbeKind::Business),
        q("blocking one", "w", GapSeverity::Blocking, ProbeKind::Research),
        q("medium one", "w", GapSeverity::Medium, ProbeKind::Memory),
        q("high one", "w", GapSeverity::High, ProbeKind::Technical),
    ];
    match merge_questions(candidates) {
        Assessment::Open(open) => {
            let gaps: Vec<GapSeverity> = open.as_slice().iter().map(|q| q.gap).collect();
            assert_eq!(
                gaps,
                vec![GapSeverity::Blocking, GapSeverity::High, GapSeverity::Medium, GapSeverity::Low]
            );
        }
        Assessment::Clear => panic!("expected survivors"),
    }
}

#[test]
fn merge_caps_at_four() {
    let words = [
        "apple", "bridge", "canyon", "desert", "ember", "falcon", "granite", "harbor", "island",
        "jungle",
    ];
    let candidates: Vec<Question> = words
        .iter()
        .map(|w| q(w, "reason", GapSeverity::Low, ProbeKind::Business))
        .collect();
    match merge_questions(candidates) {
        Assessment::Open(open) => assert_eq!(open.as_slice().len(), 4),
        Assessment::Clear => panic!("expected survivors"),
    }
}
