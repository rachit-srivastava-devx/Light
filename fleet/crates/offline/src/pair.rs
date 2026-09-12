use std::collections::HashMap;

use crate::types::{EvaluationRequest, HeldOutTask, OfflineError};

pub struct PairedTrial<'a> {
    pub task: &'a HeldOutTask,
}

/// Build a deterministic, seed-ordered list of trials from the request.
///
/// Returns `PairMismatch` if two tasks share an id but differ in digest.
/// Returns `Coverage` if the task list is empty.
/// Returns `InsufficientPairs` if the task count is less than `min_pairs`.
pub fn build_pairs(req: &EvaluationRequest) -> Result<Vec<PairedTrial<'_>>, OfflineError> {
    if req.tasks.is_empty() {
        return Err(OfflineError::Coverage);
    }
    if req.min_pairs == 0 {
        return Err(OfflineError::InsufficientPairs { need: 1, got: 0 });
    }
    let n = req.tasks.len() as u64;
    if n < req.min_pairs {
        return Err(OfflineError::InsufficientPairs {
            need: req.min_pairs,
            got: n,
        });
    }
    // Reject duplicate ids with mismatched digests.
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for task in &req.tasks {
        match seen.get(task.id.as_str()) {
            Some(&d) if d != task.input_digest => return Err(OfflineError::PairMismatch),
            _ => {
                seen.insert(&task.id, &task.input_digest);
            }
        }
    }
    // Deterministic index permutation driven by seed.
    let mut indices: Vec<usize> = (0..req.tasks.len()).collect();
    let seed = req.seed;
    for i in (1..indices.len()).rev() {
        let j = (seed.wrapping_add(i as u64) % (i as u64 + 1)) as usize;
        indices.swap(i, j);
    }
    let pairs = indices
        .into_iter()
        .map(|idx| PairedTrial {
            task: &req.tasks[idx],
        })
        .collect();
    Ok(pairs)
}
