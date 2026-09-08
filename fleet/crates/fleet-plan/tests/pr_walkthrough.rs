use fleet_plan::review::VerdictName;
use fleet_plan::{
    build_pr_walkthrough, AcceptanceResult, AttestationSummary, DiffSummary, FileChange, PrWalkthroughError,
};
use serde_json::{json, Value};

fn brief() -> Value {
    json!({
        "schema_version": "1.0", "node_id": "mod-a", "grain": "module", "purpose": "does the thing",
        "owner": "me", "owner_path": "a/", "interface": [{"name": "f", "signature": "() => void"}],
        "data_owned": [], "deps": [], "registry": {"kind": "build_new", "searched": ["a"]},
        "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
        "non_goals": [], "open_questions": [],
        "guarantees": [{"claim": "c", "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
        "alternatives": [{"option": "a", "why_killed": "w", "revive_trigger": "r"}, {"option": "b", "why_killed": "w", "revive_trigger": "r"}],
        "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
    })
}

fn diff() -> DiffSummary {
    DiffSummary {
        files: vec![
            FileChange { path: "src/small.rs".into(), lines_added: 2, lines_removed: 0 },
            FileChange { path: "src/big.rs".into(), lines_added: 40, lines_removed: 10 },
        ],
    }
}

fn passing_acceptance() -> Vec<AcceptanceResult> {
    vec![AcceptanceResult { check_name: "check_1".into(), oracle_kind: "test".into(), passed: true, detail: "ok".into() }]
}

fn attestation(verdict: VerdictName) -> AttestationSummary {
    AttestationSummary { builder: "worker-1".into(), verdict, notes: "n".into() }
}

#[test]
fn pr_walkthrough_names_the_largest_diff_as_riskiest_when_all_checks_pass() {
    let w = build_pr_walkthrough(&brief(), &diff(), &passing_acceptance(), &attestation(VerdictName::Accept)).unwrap();
    assert!(w.riskiest_part.contains("src/big.rs"));
    assert_eq!(w.verified.len(), 1);
    assert!(w.not_verified.is_empty());
    assert_eq!(w.why, "does the thing");
    assert!(w.what_changed.contains("mod-a"));
}

#[test]
fn empty_diff_is_a_typed_refusal_not_an_empty_success() {
    let empty = DiffSummary { files: vec![] };
    let err = build_pr_walkthrough(&brief(), &empty, &passing_acceptance(), &attestation(VerdictName::Accept)).unwrap_err();
    assert_eq!(err, PrWalkthroughError::EmptyDiff);
}

#[test]
fn zero_acceptance_results_is_a_typed_refusal() {
    let err = build_pr_walkthrough(&brief(), &diff(), &[], &attestation(VerdictName::Accept)).unwrap_err();
    assert_eq!(err, PrWalkthroughError::NoAcceptanceResults);
}

#[test]
fn invalid_module_brief_is_a_typed_refusal() {
    let mut bad = brief();
    bad["purpose"] = json!("");
    let err = build_pr_walkthrough(&bad, &diff(), &passing_acceptance(), &attestation(VerdictName::Accept)).unwrap_err();
    matches!(err, PrWalkthroughError::InvalidModuleBrief { .. });
}
