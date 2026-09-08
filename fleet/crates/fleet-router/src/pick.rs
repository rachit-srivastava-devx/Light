//! Stage 6: the deterministic tie-breaker. Re-homed from
//! `fleet/keel/fleet/src/route.rs:249-267`.

use crate::table::{CandidateSpec, ORDER};
use crate::types::Refusal;

/// Walk `ORDER` (equivalently `runtime.preference`'s committed order) and take the first id
/// both listed in `preference` and still alive after stage 5.
pub(crate) fn pick(candidates: &[CandidateSpec], preference: &[&'static str]) -> Option<CandidateSpec> {
    ORDER
        .iter()
        .find(|ordered| preference.contains(&ordered.id) && candidates.iter().any(|c| c.id == ordered.id))
        .copied()
}

/// This stage's refusal, if no candidate was `pick`ed.
pub(crate) fn refusal_pick(selected: Option<CandidateSpec>) -> Option<Refusal> {
    selected.is_none().then(|| Refusal {
        stage: 6,
        stage_name: "deterministic pick",
        reason: "no surviving candidate appears in the committed preference order".into(),
        fix: "add an eligible candidate to the committed route order".into(),
    })
}
