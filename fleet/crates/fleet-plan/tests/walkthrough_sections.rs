use fleet_plan::build_walkthrough;
use serde_json::{json, Value};

fn brief(node_id: &str, open_questions: &[&str]) -> Value {
    json!({
        "schema_version": "1.0", "node_id": node_id, "grain": "module",
        "purpose": format!("purpose of {node_id}"), "owner": "me", "owner_path": "a/",
        "interface": [{"name": "f", "signature": "() => void"}], "data_owned": [], "deps": [],
        "registry": {"kind": "build_new", "searched": ["a"]},
        "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
        "non_goals": [], "open_questions": open_questions,
        "guarantees": [{"claim": format!("{node_id} works"), "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
        "alternatives": [{"option": "a", "why_killed": "w", "revive_trigger": "r"}, {"option": "b", "why_killed": "w", "revive_trigger": "r"}],
        "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
    })
}

#[test]
fn open_questions_drive_owner_focus_ranking() {
    let modules = vec![brief("mod-a", &[]), brief("mod-b", &["is this safe?"])];
    let w = build_walkthrough("plan", &modules).expect("valid plan");
    assert_eq!(w.owner_focus, vec!["mod-b".to_string()]);
    assert_eq!(w.risks.len(), 1);
    assert_eq!(w.risks[0].node_id, "mod-b");
}

#[test]
fn key_decisions_and_acceptance_preview_cover_every_module() {
    let modules = vec![brief("mod-a", &[]), brief("mod-b", &[])];
    let w = build_walkthrough("plan", &modules).expect("valid plan");
    assert_eq!(w.key_decisions.len(), 2);
    assert_eq!(w.acceptance_preview.len(), 2);
}
