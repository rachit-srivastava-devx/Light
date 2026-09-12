use fleet_plan::{build_walkthrough, WalkthroughError};
use serde_json::{json, Value};

fn brief(node_id: &str, deps: &[&str], open_questions: &[&str]) -> Value {
    json!({
        "schema_version": "1.0",
        "node_id": node_id,
        "grain": "module",
        "purpose": format!("purpose of {node_id}"),
        "owner": "me",
        "owner_path": "a/",
        "interface": [{"name": "f", "signature": "() => void"}],
        "data_owned": [],
        "deps": deps,
        "registry": {"kind": "build_new", "searched": ["a"]},
        "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
        "non_goals": [],
        "open_questions": open_questions,
        "guarantees": [{"claim": format!("{node_id} works"), "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
        "alternatives": [
            {"option": "a", "why_killed": "w", "revive_trigger": "r"},
            {"option": "b", "why_killed": "w", "revive_trigger": "r"}
        ],
        "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
    })
}

#[test]
fn walkthrough_of_a_plan_with_n_modules_lists_them_in_work_order() {
    let modules = vec![brief("mod-c", &["mod-b"], &[]), brief("mod-a", &[], &[]), brief("mod-b", &["mod-a"], &[])];
    let w = build_walkthrough("demo plan", &modules).expect("valid plan");

    assert_eq!(w.what_will_be_built.len(), 3);
    assert_eq!(w.work_order.len(), 3);
    let order: Vec<&str> = w.work_order.iter().map(|s| s.node_id.as_str()).collect();
    assert_eq!(order, vec!["mod-a", "mod-b", "mod-c"]);
    assert_eq!(w.work_order[0].position, 1);
    assert_eq!(w.work_order[2].because, "after: mod-b");
}

#[test]
fn empty_plan_yields_a_typed_refusal_not_an_empty_success() {
    let err = build_walkthrough("empty plan", &[]).unwrap_err();
    assert_eq!(err, WalkthroughError::EmptyPlan);
}

#[test]
fn blank_module_brief_yields_invalid_module_brief_refusal_not_a_silent_narration() {
    let mut blank = brief("mod-a", &[], &[]);
    blank["purpose"] = json!("");
    let err = build_walkthrough("plan", &[blank]).unwrap_err();
    matches!(err, WalkthroughError::InvalidModuleBrief { index: 0, .. });
}

#[test]
fn cyclic_deps_are_refused_not_silently_ordered() {
    let modules = vec![brief("mod-a", &["mod-b"], &[]), brief("mod-b", &["mod-a"], &[])];
    let err = build_walkthrough("plan", &modules).unwrap_err();
    match err {
        WalkthroughError::CyclicDependency { cycle } => {
            assert_eq!(cycle, vec!["mod-a".to_string(), "mod-b".to_string()]);
        }
        other => panic!("expected CyclicDependency, got {other:?}"),
    }
}
