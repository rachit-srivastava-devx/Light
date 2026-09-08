//! Refusal + verifier-independence cases: §9 of blueprints/fleet-router/BLUEPRINT.md.

mod common;

use common::{assert_mutually_exclusive, empty_runtime, full_runtime};
use fleet_router::{decide, TaskClass};
use fleet_types::Role;
use std::collections::BTreeMap;

#[test]
fn every_filter_stage_names_its_empty_set() {
    let full = full_runtime();

    let stage1 = decide(None, TaskClass::General, None, &full);
    assert_eq!(stage1.refusal.unwrap().stage, 1);

    let stage2 = decide(Some(Role::Lead), TaskClass::Implementation, None, &full);
    assert_eq!(stage2.refusal.unwrap().stage, 2);

    let stage3 = decide(Some(Role::Builder), TaskClass::General, None, &empty_runtime());
    assert_eq!(stage3.refusal.unwrap().stage, 3);

    let mut no_quota = full.clone();
    no_quota.remaining = BTreeMap::new();
    let stage4 = decide(Some(Role::Builder), TaskClass::General, None, &no_quota);
    assert_eq!(stage4.refusal.unwrap().stage, 4);

    // No resolved builder model at all -- stage 5 clears every verifier candidate.
    let stage5 = decide(Some(Role::Verifier), TaskClass::General, None, &full);
    assert_eq!(stage5.refusal.unwrap().stage, 5);

    let mut no_preference = full.clone();
    no_preference.preference = Vec::new();
    let stage6 = decide(Some(Role::Builder), TaskClass::General, None, &no_preference);
    assert_eq!(stage6.refusal.unwrap().stage, 6);
}

#[test]
fn first_refusal_wins_over_later_stages() {
    // Empties both stage 3 (no capable adapter) and stage 4 (no measured quota) at once --
    // the reported refusal must still name stage 3, never stage 4.
    let decision = decide(Some(Role::Builder), TaskClass::General, None, &empty_runtime());
    assert_eq!(decision.refusal.unwrap().stage, 3);
}

#[test]
fn decision_refusal_and_selection_are_mutually_exclusive() {
    for runtime in [empty_runtime(), full_runtime()] {
        for role in [Role::Lead, Role::Builder, Role::Verifier, Role::Designer, Role::Meter] {
            assert_mutually_exclusive(&decide(Some(role), TaskClass::General, None, &runtime));
        }
    }
    assert_mutually_exclusive(&decide(None, TaskClass::General, None, &full_runtime()));
}
