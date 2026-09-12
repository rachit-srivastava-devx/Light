use fleet_plan::review::VerdictName;
use fleet_plan::{build_pr_walkthrough, AcceptanceResult, AttestationSummary, DiffSummary, FileChange};
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
    DiffSummary { files: vec![FileChange { path: "src/big.rs".into(), lines_added: 40, lines_removed: 10 }] }
}

fn passing_acceptance() -> Vec<AcceptanceResult> {
    vec![AcceptanceResult { check_name: "check_1".into(), oracle_kind: "test".into(), passed: true, detail: "ok".into() }]
}

fn attestation(verdict: VerdictName) -> AttestationSummary {
    AttestationSummary { builder: "worker-1".into(), verdict, notes: "n".into() }
}

#[test]
fn a_failed_check_outranks_diff_size_as_the_riskiest_part() {
    let mut results = passing_acceptance();
    results.push(AcceptanceResult { check_name: "check_2".into(), oracle_kind: "test".into(), passed: false, detail: "broke".into() });
    let w = build_pr_walkthrough(&brief(), &diff(), &results, &attestation(VerdictName::Accept)).unwrap();
    assert!(w.riskiest_part.contains("check_2"));
    assert_eq!(w.verified.len(), 1);
    assert_eq!(w.not_verified.len(), 1);
    assert_eq!(w.reviewer_focus[0], "check_2 did not pass: broke");
    assert_eq!(w.reviewer_focus.last().unwrap(), &w.riskiest_part);
}

#[test]
fn non_accept_verdict_leads_reviewer_focus() {
    let w = build_pr_walkthrough(&brief(), &diff(), &passing_acceptance(), &attestation(VerdictName::Reject)).unwrap();
    assert!(w.reviewer_focus[0].contains("Reject"));
}
