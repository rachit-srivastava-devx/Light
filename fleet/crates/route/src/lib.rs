mod explain;
mod filter;
mod policy;
mod score;

// Fleet-router backward-compat re-exports for src/ (Decision, TaskClass, etc.)
pub use fleet_router::{decide, evaluate_role_check, RoleCheck, RoleRefusal,
    CandidateSpec, TaskClass, Tier, ORDER, Decision, Refusal, RuntimeState, Stage};

pub use explain::{CandidateId, ReservationRequest, RouteDecision, RouteRefusal, StageEvidence};
pub use filter::{BudgetSnapshot, Candidate, CatalogSnapshot, IntentSpec};
pub use policy::PolicySnapshot;
pub use score::CONSERVATIVE_BASELINE;

pub fn admit(
    intent: &IntentSpec,
    snapshot: &CatalogSnapshot,
    budget: &BudgetSnapshot,
    policy: &PolicySnapshot,
) -> Result<RouteDecision, RouteRefusal> {
    let (cap_survivors, cap_ev) = filter::capability_filter(intent, snapshot);
    if cap_survivors.is_empty() {
        return Err(RouteRefusal::NoCandidates {
            stage: "capability".into(),
            reason: "no candidates passed capability filter".into(),
            candidates_in: cap_ev.candidates_in,
        });
    }
    let (pol_survivors, pol_ev) = filter::policy_filter(cap_survivors, policy);
    if pol_survivors.is_empty() {
        return Err(RouteRefusal::NoCandidates {
            stage: "policy".into(),
            reason: "no candidates passed policy filter".into(),
            candidates_in: pol_ev.candidates_in,
        });
    }
    let (bud_survivors, bud_ev) = filter::budget_filter(pol_survivors, budget);
    if bud_survivors.is_empty() {
        return Err(RouteRefusal::NoCandidates {
            stage: "budget".into(),
            reason: "no candidates passed budget filter".into(),
            candidates_in: bud_ev.candidates_in,
        });
    }
    let (selected, selected_cost, score_ev) = score::score_and_select(&bud_survivors);
    Ok(RouteDecision {
        selected: selected.id.clone(),
        selected_cost,
        stages: vec![cap_ev, pol_ev, bud_ev, score_ev],
        reservation: ReservationRequest {
            candidate_id: selected.id.clone(),
            reserved_until: std::time::Instant::now()
                + std::time::Duration::from_secs(60),
        },
        snapshot_digest: snapshot.digest.clone(),
    })
}

#[cfg(test)]
mod tests;
