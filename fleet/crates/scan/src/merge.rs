//! Merge: drop no-"why" -> Jaccard>0.6 dedup -> sort by gap -> cap 4 -> Clear|Open.

use std::cmp::Ordering;

use crate::jaccard::jaccard_similarity;
use crate::open_questions::{Assessment, OpenQuestions};
use crate::probe::Question;

/// Total, deterministic ranking key: gap descending, then `ProbeKind` declaration order
/// ascending, then `text`/`why`/`evidence` ascending -- makes candidates with equal gap/probe a
/// well-defined order regardless of the input vec's original order.
fn rank_key(a: &Question, b: &Question) -> Ordering {
    b.gap
        .cmp(&a.gap)
        .then_with(|| a.probe.cmp(&b.probe))
        .then_with(|| a.text.cmp(&b.text))
        .then_with(|| a.why.cmp(&b.why))
        .then_with(|| a.evidence.cmp(&b.evidence))
}

/// Merge raw candidate questions from all 4 probes into an `Assessment`. Deterministic: the same
/// multiset of `Question`s in any input order always yields the same `Assessment`.
pub fn merge_questions(candidates: Vec<Question>) -> Assessment {
    let mut ranked: Vec<Question> = candidates
        .into_iter()
        .filter(|q| !q.why.trim().is_empty())
        .collect();
    ranked.sort_by(rank_key);

    let mut kept: Vec<Question> = Vec::new();
    'outer: for q in ranked {
        for k in &kept {
            if jaccard_similarity(&q.text, &k.text) > 0.6 {
                continue 'outer;
            }
        }
        kept.push(q);
    }
    kept.truncate(4);

    if kept.is_empty() {
        Assessment::Clear
    } else {
        Assessment::Open(OpenQuestions::new(kept).expect("len is 1..=4 by construction"))
    }
}
