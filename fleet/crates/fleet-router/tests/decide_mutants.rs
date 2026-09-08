//! Mutation-testing targets named in §9 of blueprints/fleet-router/BLUEPRINT.md: the D50
//! reachability regression and the `ORDER` ordering it depends on.

use fleet_router::{decide, CandidateSpec, RuntimeState, TaskClass, Tier, ORDER};
use fleet_types::Role;
use std::collections::{BTreeMap, BTreeSet};

fn role_for(tier: Tier) -> Role {
    match tier {
        Tier::Lead => Role::Lead,
        Tier::Worker => Role::Builder,
        Tier::Cheap => Role::Meter,
    }
}

#[test]
fn every_reachable_candidate_is_reachable() {
    // Direct regression for D50: for every entry in `ORDER`, a runtime where exactly that
    // candidate's adapter is capable/quota'd/uncooled must route to it -- no entry may be
    // structurally unreachable (e.g. `freelane` sitting outside every reachable path).
    for candidate in ORDER {
        let CandidateSpec { adapter, id, resolved, tier, .. } = *candidate;
        let mut remaining = BTreeMap::new();
        remaining.insert(adapter.to_string(), Some(1_000));
        let runtime = RuntimeState {
            capable: BTreeSet::from([adapter]),
            remaining,
            cooldown: BTreeSet::new(),
            required_tokens: 10,
            preference: ORDER.iter().map(|c| c.id).collect(),
        };
        let decision = decide(Some(role_for(tier)), TaskClass::General, None, &runtime);
        assert_eq!(decision.selected_adapter, Some(adapter), "candidate {id} unreachable");
        assert_eq!(decision.resolved_model, Some(resolved), "candidate {id} unreachable");
    }
}
