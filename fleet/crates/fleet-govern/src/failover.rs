//! Assembling `fleet_router::RuntimeState` and calling `decide` -- `route.rs:316-373`'s
//! `runtime`/`for_plan`, re-homed: the meter-snapshot read and the cooldown check move here, the
//! decision logic stays in `fleet-router`. This crate never probes capability itself; `capable`
//! arrives already computed by the caller (`fleet-worker`).

use std::collections::BTreeSet;
use std::time::SystemTime;

use fleet_router::{decide, Decision, RuntimeState, TaskClass, ORDER};
use fleet_types::{Role, Tokens};

use crate::store::{CooldownStore, MeterStore};
use crate::types::MeterIoError;

/// Everything this crate must be told to build one `RuntimeState`.
pub struct FailoverInputs<'a> {
    pub role: Option<Role>,
    pub class: TaskClass,
    pub builder_resolved_model: Option<&'a str>,
    pub capable: BTreeSet<&'static str>,
    pub required_tokens: Tokens,
    pub now: SystemTime,
}

pub fn next_provider(
    store: &dyn MeterStore,
    cooldowns: &dyn CooldownStore,
    inputs: &FailoverInputs<'_>,
) -> Result<Decision, MeterIoError> {
    let remaining = store
        .snapshot_remaining()?
        .into_iter()
        .map(|(k, v)| (k, v.map(Tokens::get)))
        .collect();

    let mut cooldown = BTreeSet::new();
    let mut checked = BTreeSet::new();
    for candidate in ORDER {
        if checked.insert(candidate.adapter) && cooldowns.is_cooling_down(candidate.adapter, inputs.now)? {
            cooldown.insert(candidate.adapter.to_string());
        }
    }

    let runtime = RuntimeState {
        capable: inputs.capable.clone(),
        remaining,
        cooldown,
        required_tokens: inputs.required_tokens.get(),
        preference: ORDER.iter().map(|c| c.id).collect(),
    };

    Ok(decide(inputs.role, inputs.class, inputs.builder_resolved_model, &runtime))
}
