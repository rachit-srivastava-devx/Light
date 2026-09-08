//! `decide` orchestration. Re-homed from `fleet/keel/fleet/src/route.rs:136-278` -- the 6-stage
//! pipeline itself, wired over `stage.rs`/`verify_gate.rs`'s per-stage filters and `table.rs`'s
//! `ORDER`. "First empty wins": `first_empty.get_or_insert(..)` never overwrites an earlier
//! stage's refusal with a later one.

use crate::stage::{filter_capability, filter_quota, filter_role, refusal_capability, refusal_quota, refusal_role, stage};
use crate::table::TaskClass;
use crate::types::{Decision, Refusal, RuntimeState};
use crate::pick::{pick, refusal_pick};
use crate::verify_gate::{filter_verifier, refusal_verifier, safety_refusal};
use fleet_types::Role;

/// Run the full 6-stage pipeline once. Pure and total: never panics, never blocks. Deterministic:
/// identical `(role, class, builder_resolved_model, runtime)` always yields an identical
/// `Decision`, including field order inside `stages`.
pub fn decide(
    role: Option<Role>,
    class: TaskClass,
    builder_resolved_model: Option<&str>,
    runtime: &RuntimeState,
) -> Decision {
    let mut stages = Vec::with_capacity(6);
    let mut first_empty: Option<Refusal> = None;

    let mut candidates = filter_role(role);
    stages.push(stage(1, "explicit role", &candidates));
    if let Some(r) = refusal_role(&candidates) {
        first_empty.get_or_insert(r);
    }

    if let Some((reason, fix)) = safety_refusal(role, class) {
        candidates.clear();
        first_empty.get_or_insert(Refusal { stage: 2, stage_name: "safety policy", reason: reason.into(), fix: fix.into() });
    }
    stages.push(stage(2, "safety policy", &candidates));

    filter_capability(&mut candidates, runtime);
    if let Some(r) = refusal_capability(&candidates) {
        first_empty.get_or_insert(r);
    }
    stages.push(stage(3, "local capability", &candidates));

    filter_quota(&mut candidates, runtime);
    if let Some(r) = refusal_quota(&candidates, runtime) {
        first_empty.get_or_insert(r);
    }
    stages.push(stage(4, "availability/quota", &candidates));

    filter_verifier(&mut candidates, role, builder_resolved_model);
    if let Some(r) = refusal_verifier(&candidates) {
        first_empty.get_or_insert(r);
    }
    stages.push(stage(5, "verifier independence", &candidates));

    let selected = pick(&candidates, &runtime.preference);
    if let Some(r) = refusal_pick(selected) {
        first_empty.get_or_insert(r);
    }
    stages.push(stage(6, "deterministic pick", &selected.into_iter().collect::<Vec<_>>()));

    Decision {
        role: role.map(Role::name).unwrap_or("invalid"),
        stages,
        selected_adapter: selected.map(|c| c.adapter),
        requested_model: selected.map(|c| c.requested),
        resolved_model: selected.map(|c| c.resolved),
        decided_at_stage: selected.map(|_| 6),
        refusal: first_empty,
    }
}
