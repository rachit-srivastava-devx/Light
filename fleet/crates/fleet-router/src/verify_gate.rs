//! Stages 2 and 5: safety policy and verifier independence -- the two stages that reason over
//! `RoleCheck`. Re-homed from `fleet/keel/fleet/src/route.rs:165-179,224-247`.

use crate::role_check::{evaluate_role_check, RoleCheck};
use crate::table::{CandidateSpec, TaskClass};
use crate::types::Refusal;
use fleet_types::Role;

/// Stage 2: refuse `HumanOnly` outright; refuse a Lead role on `Implementation`. Returns the
/// `(reason, fix)` pair on refusal, matching `RoleRefusal::reason()`'s wire strings.
pub(crate) fn safety_refusal(
    role: Option<Role>,
    class: TaskClass,
) -> Option<(&'static str, &'static str)> {
    match (role, class) {
        (_, TaskClass::HumanOnly) => Some((
            "HUMAN_ONLY",
            "assign the money, contract, or migration decision to a human",
        )),
        (Some(Role::Lead), TaskClass::Implementation) => {
            let check = RoleCheck { role: Role::Lead, diff_adds_code: true, builder_model: None, verifier_model: None };
            evaluate_role_check(&check)
                .err()
                .map(|reason| (reason.reason(), "route implementation with --role builder"))
        }
        _ => None,
    }
}

/// Stage 5: if `role == Verifier`, each surviving candidate's resolved model must pass
/// `evaluate_role_check` against `builder_resolved_model`. `None` builder model clears all
/// candidates -- it does not skip the stage.
pub(crate) fn filter_verifier(
    candidates: &mut Vec<CandidateSpec>,
    role: Option<Role>,
    builder_resolved_model: Option<&str>,
) {
    if role != Some(Role::Verifier) {
        return;
    }
    match builder_resolved_model {
        Some(builder) => candidates.retain(|c| {
            let check = RoleCheck {
                role: Role::Verifier,
                diff_adds_code: false,
                builder_model: Some(builder),
                verifier_model: Some(c.resolved),
            };
            evaluate_role_check(&check).is_ok()
        }),
        None => candidates.clear(),
    }
}

/// Stage 5's refusal, if `candidates` came back empty.
pub(crate) fn refusal_verifier(candidates: &[CandidateSpec]) -> Option<Refusal> {
    candidates.is_empty().then(|| Refusal {
        stage: 5,
        stage_name: "verifier independence",
        reason: "no verifier with a resolved model distinct from the builder remains".into(),
        fix: "pass the builder's resolved model and make another worker model available".into(),
    })
}
