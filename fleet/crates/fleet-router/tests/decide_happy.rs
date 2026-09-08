//! Happy-path resolution: §9 of blueprints/fleet-router/BLUEPRINT.md.

mod common;

use common::full_runtime;
use fleet_router::{decide, TaskClass};
use fleet_types::Role;

#[test]
fn lead_implementation_refuses_and_lead_general_routes_to_opus() {
    let runtime = full_runtime();

    let refused = decide(Some(Role::Lead), TaskClass::Implementation, None, &runtime);
    assert_eq!(refused.refusal.unwrap().reason, "LEAD_WROTE_CODE");
    assert!(refused.selected_adapter.is_none());

    let routed = decide(Some(Role::Lead), TaskClass::General, None, &runtime);
    assert!(routed.refusal.is_none());
    assert_eq!(routed.resolved_model, Some("opus"));
}

#[test]
fn deterministic_across_runs_and_both_availability_directions() {
    let runtime = full_runtime();
    let first = decide(Some(Role::Builder), TaskClass::General, None, &runtime);
    for _ in 0..20 {
        let again = decide(Some(Role::Builder), TaskClass::General, None, &runtime);
        assert_eq!(again.resolved_model, first.resolved_model);
    }
    assert_eq!(first.resolved_model, Some("codex"));

    let mut cooling = runtime.clone();
    cooling.cooldown.insert("codex".to_string());
    let shifted = decide(Some(Role::Builder), TaskClass::General, None, &cooling);
    assert_eq!(shifted.resolved_model, Some("sonnet"));
}

#[test]
fn verifier_compares_resolved_identity_in_both_directions() {
    let runtime = full_runtime();

    // A codex builder must not be re-picked as its own verifier: "codex" is filtered by
    // resolved-identity, so the next worker-tier candidate (sonnet) is chosen instead.
    let against_resolved = decide(Some(Role::Verifier), TaskClass::General, Some("codex"), &runtime);
    assert_eq!(against_resolved.resolved_model, Some("sonnet"));

    // The comparison is against `resolved`, not `requested`: "codex-worker" is codex's
    // *requested* alias, not its resolved identity, so it does not filter codex out.
    let against_requested = decide(Some(Role::Verifier), TaskClass::General, Some("codex-worker"), &runtime);
    assert_eq!(against_requested.resolved_model, Some("codex"));
}
