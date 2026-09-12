use crate::explain::StageEvidence;
use crate::filter::Candidate;

pub const CONSERVATIVE_BASELINE: u64 = 1000;

pub fn score_and_select(candidates: &[Candidate]) -> (&Candidate, u64, StageEvidence) {
    let n = candidates.len();
    let (selected, cost) = candidates
        .iter()
        .map(|c| (c, c.historical_cost.unwrap_or(CONSERVATIVE_BASELINE)))
        .min_by_key(|&(_, cost)| cost)
        .expect("candidates slice is non-empty");
    (
        selected,
        cost,
        StageEvidence {
            stage: "score".into(),
            candidates_in: n,
            candidates_out: n,
        },
    )
}
