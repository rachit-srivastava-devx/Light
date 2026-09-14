//! Shared "everything is healthy" `RuntimeState` -- every candidate confirmed installed, with an
//! unmeasured-but-generous quota window and no cooldown: the best basis a fresh CLI invocation
//! with no measured data can assume (real measured values are `fleet-govern`'s job, not a
//! composition root's). Single source of truth for `route_cmd.rs` and `run_cmd.rs` so the two
//! never drift -- they already had: `route_cmd.rs` kept a stale copy that built `capable` from
//! candidate `id`s instead of `adapter`s and left `remaining` empty, so `fleet route` refused at
//! stage 3 (lead/designer) or stage 4 (every other role) unconditionally, for every `--role`.

use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn healthy_runtime() -> route::RuntimeState {
    let preference: Vec<&'static str> = route::ORDER.iter().map(|c| c.id).collect();
    let capable = route::ORDER
        .iter()
        .map(|c| c.adapter)
        .collect::<BTreeSet<_>>();
    let remaining = capable
        .iter()
        .map(|a| (a.to_string(), Some(u64::MAX)))
        .collect::<BTreeMap<_, _>>();
    route::RuntimeState {
        capable,
        remaining,
        cooldown: BTreeSet::new(),
        required_tokens: 0,
        preference,
    }
}
