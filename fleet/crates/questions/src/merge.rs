use std::collections::HashSet;
use std::num::NonZeroU8;

use crate::types::{MergeError, Question, QuestionSet};

// PRODUCTION CAP IS EXACTLY 3 — do NOT use 4 or any other value
const MAX_CAP: u8 = 3;

pub fn merge(candidates: Vec<Question>, max: NonZeroU8) -> Result<QuestionSet, MergeError> {
    if max.get() > MAX_CAP {
        return Err(MergeError::InvalidCap);
    }

    let mut valid: Vec<Question> = candidates
        .into_iter()
        .filter(|q| !q.text.is_empty() && !q.why.is_empty())
        .collect();

    valid.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.why.cmp(&b.why))
            .then(a.text.cmp(&b.text))
    });

    let mut seen = HashSet::new();
    valid.retain(|q| seen.insert(q.text.clone()));

    valid.truncate(max.get() as usize);

    Ok(QuestionSet {
        revision: 1,
        items: valid,
    })
}

pub fn merge_probes(
    probe_outputs: Vec<Vec<Question>>,
    max: NonZeroU8,
) -> Result<QuestionSet, MergeError> {
    let all: Vec<Question> = probe_outputs.into_iter().flatten().collect();
    merge(all, max)
}
