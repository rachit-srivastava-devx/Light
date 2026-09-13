use crate::types::{PlanDraft, PlanInput, PlannerError, ValidationReport};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn validate_draft(
    input: &PlanInput,
    draft: &PlanDraft,
) -> Result<ValidationReport, PlannerError> {
    if input.task_digest.is_empty()
        || input.recipe_digest.is_empty()
        || input.context_digest.is_empty()
    {
        return Err(PlannerError::InvalidInput(
            "all digests must be nonempty".into(),
        ));
    }
    if input.max_modules == 0 {
        return Err(PlannerError::InvalidInput(
            "max_modules must be greater than 0".into(),
        ));
    }
    if input.acceptance_refs.is_empty() {
        return Err(PlannerError::InvalidInput(
            "acceptance_refs must not be empty".into(),
        ));
    }
    if draft.modules.is_empty() {
        return Err(PlannerError::InvalidDraft(
            "draft must contain at least one module".into(),
        ));
    }
    if draft.modules.len() as u32 > input.max_modules {
        return Err(PlannerError::InvalidDraft(format!(
            "draft has {} modules but max_modules is {}",
            draft.modules.len(),
            input.max_modules
        )));
    }
    let ids: HashSet<&str> = {
        let mut set = HashSet::new();
        for m in &draft.modules {
            if !set.insert(m.id.as_str()) {
                return Err(PlannerError::InvalidDraft(format!(
                    "duplicate module id: {}",
                    m.id
                )));
            }
        }
        set
    };
    for m in &draft.modules {
        for dep in &m.dependencies {
            if !ids.contains(dep.as_str()) {
                return Err(PlannerError::InvalidDraft(format!(
                    "unknown dependency: {dep} in module {}",
                    m.id
                )));
            }
        }
    }
    // Kahn's topological sort — detects cycles in the dependency DAG.
    let mut in_deg: HashMap<&str, usize> = draft
        .modules
        .iter()
        .map(|m| (m.id.as_str(), m.dependencies.len()))
        .collect();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in &draft.modules {
        for dep in &m.dependencies {
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(m.id.as_str());
        }
    }
    let mut queue: VecDeque<&str> = in_deg
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut processed = 0usize;
    while let Some(node) = queue.pop_front() {
        processed += 1;
        if let Some(deps) = dependents.get(node) {
            for &dep_of in deps {
                let d = in_deg.entry(dep_of).or_insert(0);
                *d -= 1;
                if *d == 0 {
                    queue.push_back(dep_of);
                }
            }
        }
    }
    if processed != draft.modules.len() {
        return Err(PlannerError::InvalidDraft(
            "cycle detected in module dependencies".into(),
        ));
    }
    let total = draft.modules.len() as u32;
    let mut ids_sorted: Vec<&str> = draft.modules.iter().map(|m| m.id.as_str()).collect();
    ids_sorted.sort_unstable();
    let digest = ids_sorted.join(",");
    Ok(ValidationReport {
        checked: total,
        total,
        digest,
    })
}
