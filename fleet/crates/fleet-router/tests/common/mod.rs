//! Shared `RuntimeState` fixtures for the integration test suite. Each `tests/*.rs` file is its
//! own compiled binary, so a helper unused by one binary is expected -- not dead code.
#![allow(dead_code)]

use fleet_router::{Decision, RuntimeState, ORDER};
use std::collections::{BTreeMap, BTreeSet};

pub fn full_runtime() -> RuntimeState {
    let mut remaining = BTreeMap::new();
    for c in ORDER {
        remaining.insert(c.adapter.to_string(), Some(1_000));
    }
    RuntimeState {
        capable: ORDER.iter().map(|c| c.adapter).collect::<BTreeSet<_>>(),
        remaining,
        cooldown: BTreeSet::new(),
        required_tokens: 10,
        preference: ORDER.iter().map(|c| c.id).collect(),
    }
}

pub fn empty_runtime() -> RuntimeState {
    RuntimeState {
        capable: BTreeSet::new(),
        remaining: BTreeMap::new(),
        cooldown: BTreeSet::new(),
        required_tokens: 10,
        preference: Vec::new(),
    }
}

pub fn assert_mutually_exclusive(decision: &Decision) {
    assert_eq!(decision.refusal.is_some(), decision.selected_adapter.is_none());
    assert_eq!(decision.refusal.is_some(), decision.requested_model.is_none());
    assert_eq!(decision.refusal.is_some(), decision.resolved_model.is_none());
    assert_eq!(decision.refusal.is_some(), decision.decided_at_stage.is_none());
    assert_eq!(decision.stages.len(), 6);
}
