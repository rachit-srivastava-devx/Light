use fleet_plan::{derive_questions, intake_gate, open_questions, GateDecision, QuestionId, StageReadiness, Trigger};
use std::collections::BTreeSet;

#[test]
fn question_id_parse_matches_all_15_and_only_15() {
    let ids = [
        "scope", "success", "cli_flag", "schema_migration", "ui_view", "api_endpoint", "deletion",
        "scale", "tenancy", "precision", "auth", "ownership", "failure", "unhappy", "rename_refactor",
    ];
    for id in ids {
        let q = QuestionId::parse(id).unwrap();
        assert_eq!(q.as_str(), id);
    }
    assert!(QuestionId::parse("not_a_real_id").is_err());
}

#[test]
fn derive_questions_core_present_even_on_empty_intent() {
    let qs = derive_questions("");
    assert_eq!(qs.len(), 2);
    assert!(qs.iter().all(|q| q.trigger == Trigger::Core));
    assert!(qs.iter().any(|q| q.id == QuestionId::Scope));
    assert!(qs.iter().any(|q| q.id == QuestionId::Success));
}

#[test]
fn derive_questions_first_match_only_not_every_occurrence() {
    let qs = derive_questions("please add a retry retry retry mechanism");
    let failure_rows: Vec<_> = qs.iter().filter(|q| q.id == QuestionId::Failure).collect();
    assert_eq!(failure_rows.len(), 1);
    assert_eq!(failure_rows[0].trigger, Trigger::Token("retry".to_string()));
}

#[test]
fn derive_questions_unicode_only_intent_gets_core_only() {
    let qs = derive_questions("\u{4f60}\u{597d}\u{4e16}\u{754c}");
    assert_eq!(qs.len(), 2);
}

#[test]
fn open_questions_excludes_answered_ids() {
    let qs = derive_questions("add a --flag for the api endpoint with auth and retry");
    let mut answered = BTreeSet::new();
    answered.insert(QuestionId::Scope);
    answered.insert(QuestionId::Success);
    let open = open_questions(&qs, &answered);
    assert_eq!(open.len(), qs.len() - 2);
    assert!(open.iter().all(|q| q.id != QuestionId::Scope && q.id != QuestionId::Success));
}

#[test]
fn intake_gate_priority_order_matches_bash() {
    let stages = StageReadiness { sow: false, atomic: false, challenges: false, clarifications: false };
    let d = intake_gate(3, stages, 5);
    assert_eq!(d, GateDecision::BlockedByRubric);
}

#[test]
fn intake_gate_reports_ready() {
    let stages = StageReadiness { sow: true, atomic: true, challenges: true, clarifications: true };
    assert_eq!(intake_gate(0, stages, 0), GateDecision::Ready);
}

#[test]
fn intake_gate_blocked_by_stages_when_rubric_clear() {
    let stages = StageReadiness { sow: true, atomic: false, challenges: true, clarifications: true };
    assert_eq!(intake_gate(0, stages, 2), GateDecision::BlockedByStages);
}

#[test]
fn intake_gate_blocked_by_open_clarifications_last() {
    let stages = StageReadiness { sow: true, atomic: true, challenges: true, clarifications: true };
    assert_eq!(intake_gate(0, stages, 7), GateDecision::BlockedByOpenClarifications { count: 7 });
}
