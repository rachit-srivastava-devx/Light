//! Stage-record builder, the structural filters (role tier, capability, quota), and their
//! refusal messages. Re-homed from `fleet/keel/fleet/src/route.rs:126-134,145-215`.

use crate::allow::role_allows;
use crate::table::{CandidateSpec, ORDER};
use crate::types::{Refusal, RuntimeState, Stage};
use fleet_types::Role;

/// Build one auditable `Stage` record: how many candidates survived, out of the total that
/// started, and their ids.
pub(crate) fn stage(number: usize, name: &'static str, candidates: &[CandidateSpec]) -> Stage {
    Stage {
        number,
        name,
        checked: candidates.len(),
        total: ORDER.len(),
        candidates: candidates.iter().map(|candidate| candidate.id).collect(),
    }
}

/// Stage 1: filter `ORDER` to the candidates `role`'s tier allows. `role = None` yields empty.
pub(crate) fn filter_role(role: Option<Role>) -> Vec<CandidateSpec> {
    role.map(|role| ORDER.iter().copied().filter(|c| role_allows(role, *c)).collect())
        .unwrap_or_default()
}

/// Stage 1's refusal, if `candidates` came back empty.
pub(crate) fn refusal_role(candidates: &[CandidateSpec]) -> Option<Refusal> {
    candidates.is_empty().then(|| Refusal {
        stage: 1,
        stage_name: "explicit role",
        reason: "role has no eligible tier set".into(),
        fix: "pass a role of lead, builder, verifier, designer, or meter".into(),
    })
}

/// Stage 3: keep only candidates whose adapter is confirmed installed and usable.
pub(crate) fn filter_capability(candidates: &mut Vec<CandidateSpec>, runtime: &RuntimeState) {
    candidates.retain(|c| runtime.capable.contains(c.adapter));
}

/// Stage 3's refusal, if `candidates` came back empty.
pub(crate) fn refusal_capability(candidates: &[CandidateSpec]) -> Option<Refusal> {
    candidates.is_empty().then(|| Refusal {
        stage: 3,
        stage_name: "local capability",
        reason: "no eligible adapter is installed and usable non-interactively".into(),
        fix: "install and authenticate the eligible Claude Code or Codex CLI, then retry".into(),
    })
}

/// Stage 4: drop candidates whose adapter is cooling down or lacks enough measured quota. An
/// unmeasured (`None`) window is never treated as available.
pub(crate) fn filter_quota(candidates: &mut Vec<CandidateSpec>, runtime: &RuntimeState) {
    candidates.retain(|c| {
        !runtime.cooldown.contains(c.adapter)
            && runtime.remaining.get(c.adapter).copied().flatten().is_some_and(|r| r >= runtime.required_tokens)
    });
}

/// Stage 4's refusal, if `candidates` came back empty.
pub(crate) fn refusal_quota(candidates: &[CandidateSpec], runtime: &RuntimeState) -> Option<Refusal> {
    candidates.is_empty().then(|| Refusal {
        stage: 4,
        stage_name: "availability/quota",
        reason: "all eligible lanes are cooling down or lack a sufficient known quota window".into(),
        fix: format!("wait for cooldown/reset or configure a measured window of at least {} tokens", runtime.required_tokens),
    })
}
