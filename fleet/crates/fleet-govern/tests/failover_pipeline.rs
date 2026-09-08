//! `next_provider` end-to-end against an in-memory `MeterStore`/`CooldownStore`: asserts this
//! crate's own assembly is correct (cooldown filtering, remaining-token forwarding) without
//! re-testing `fleet_router::decide`'s internals.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use fleet_govern::{next_provider, FailoverInputs, LaneState, MeterIoError, MeterStore};
use fleet_router::TaskClass;
use fleet_types::{LaneId, Role, Tokens};
use support::{measured, FixedCooldown, FixedStore};

#[test]
fn next_provider_forwards_to_fleet_router_and_respects_cooldown() {
    let mut lanes = BTreeMap::new();
    lanes.insert("codex".to_string(), measured(1000, 0));
    lanes.insert("claude".to_string(), measured(1000, 0));
    let store = FixedStore { lanes };
    let cooldowns = FixedCooldown { cooling: BTreeSet::from(["codex"]) };

    let inputs = FailoverInputs {
        role: Some(Role::Builder),
        class: TaskClass::Implementation,
        builder_resolved_model: None,
        capable: BTreeSet::from(["codex", "claude"]),
        required_tokens: Tokens::new(10),
        now: SystemTime::UNIX_EPOCH,
    };

    let decision = next_provider(&store, &cooldowns, &inputs).unwrap();

    assert_ne!(decision.selected_adapter, Some("codex"), "cooling-down adapter must never be selected");
    assert_eq!(decision.selected_adapter, Some("claude"));
}

#[test]
fn next_provider_forwards_meter_store_io_errors_whole() {
    struct FailingStore;
    impl MeterStore for FailingStore {
        fn with_lane_locked(&self, _lane: &LaneId, _f: &mut dyn FnMut(&mut Option<LaneState>)) -> Result<(), MeterIoError> {
            unreachable!()
        }
        fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError> {
            Err(MeterIoError("corrupt meter file".into()))
        }
    }
    let cooldowns = FixedCooldown { cooling: BTreeSet::new() };
    let inputs = FailoverInputs {
        role: Some(Role::Builder),
        class: TaskClass::Implementation,
        builder_resolved_model: None,
        capable: BTreeSet::from(["codex"]),
        required_tokens: Tokens::new(10),
        now: SystemTime::UNIX_EPOCH,
    };
    let err = next_provider(&FailingStore, &cooldowns, &inputs).unwrap_err();
    assert_eq!(err, MeterIoError("corrupt meter file".into()));
}
