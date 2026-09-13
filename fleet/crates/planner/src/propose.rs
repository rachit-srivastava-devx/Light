use std::collections::HashSet;

use crate::types::{PlanDraft, PlanInput, PlannerError, ValidationReport};

/// Injected model port. No direct model calls allowed (C9).
pub trait PlannerModel: Send + Sync {
    fn propose(&self, input: &PlanInput) -> Result<PlanDraft, PlannerError>;
}

/// Validate a draft against its input. Returns Err on the first violation found.
pub fn validate_draft(
    input: &PlanInput,
    draft: &PlanDraft,
) -> Result<ValidationReport, PlannerError> {
    if input.task_digest.is_empty()
        || input.recipe_digest.is_empty()
        || input.context_digest.is_empty()
    {
        return Err(PlannerError::InvalidInput("empty digest".into()));
    }
    if input.max_modules == 0 {
        return Err(PlannerError::InvalidInput("max_modules must be > 0".into()));
    }
    if draft.modules.len() as u32 > input.max_modules {
        return Err(PlannerError::InvalidDraft(format!(
            "module count {} exceeds max {}",
            draft.modules.len(),
            input.max_modules
        )));
    }
    // unique IDs
    let mut seen: HashSet<&str> = HashSet::new();
    for m in &draft.modules {
        if !seen.insert(m.id.as_str()) {
            return Err(PlannerError::InvalidDraft(format!(
                "duplicate id: {}",
                m.id
            )));
        }
    }
    // all deps must name existing modules (internal deps)
    let ids: HashSet<&str> = draft.modules.iter().map(|m| m.id.as_str()).collect();
    for m in &draft.modules {
        for dep in &m.dependencies {
            if !ids.contains(dep.as_str()) {
                return Err(PlannerError::InvalidDraft(format!(
                    "unknown dependency: {dep}"
                )));
            }
        }
    }
    // acceptance refs must be nonempty per module if provided
    if input.acceptance_refs.is_empty() {
        return Err(PlannerError::InvalidInput(
            "acceptance_refs must be nonempty".into(),
        ));
    }
    let total = draft.modules.len() as u32;
    let digest = canonical_digest(input, draft);
    Ok(ValidationReport {
        checked: total,
        total,
        digest,
    })
}

fn canonical_digest(input: &PlanInput, draft: &PlanDraft) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.task_digest.hash(&mut h);
    input.context_digest.hash(&mut h);
    draft.version.hash(&mut h);
    for m in &draft.modules {
        m.id.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}
