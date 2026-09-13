use planner::{validate_draft, ModuleDraft, PlanDraft, PlanInput, PlannerError};

fn base_input() -> PlanInput {
    PlanInput {
        task_digest: "td".into(),
        recipe_digest: "rd".into(),
        context_digest: "cd".into(),
        acceptance_refs: vec!["ref1".into()],
        max_modules: 10,
    }
}

#[test]
fn rejects_duplicate_ids() {
    let draft = PlanDraft {
        version: 1,
        modules: vec![
            ModuleDraft {
                id: "dup".into(),
                dependencies: vec![],
                write_set: vec![],
                acceptance_refs: vec![],
            },
            ModuleDraft {
                id: "dup".into(),
                dependencies: vec![],
                write_set: vec![],
                acceptance_refs: vec![],
            },
        ],
        explanation: "duplicate ids".into(),
    };
    let result = validate_draft(&base_input(), &draft);
    assert!(
        matches!(result, Err(PlannerError::InvalidDraft(_))),
        "expected InvalidDraft for duplicate ids, got: {result:?}",
    );
}

#[test]
fn rejects_unknown_dependency() {
    let draft = PlanDraft {
        version: 1,
        modules: vec![ModuleDraft {
            id: "a".into(),
            dependencies: vec!["nonexistent".into()],
            write_set: vec![],
            acceptance_refs: vec![],
        }],
        explanation: "bad dep".into(),
    };
    let result = validate_draft(&base_input(), &draft);
    assert!(
        matches!(result, Err(PlannerError::InvalidDraft(_))),
        "expected InvalidDraft for unknown dependency, got: {result:?}",
    );
}

#[test]
fn rejects_empty_acceptance() {
    let input = PlanInput {
        task_digest: "td".into(),
        recipe_digest: "rd".into(),
        context_digest: "cd".into(),
        acceptance_refs: vec![],
        max_modules: 10,
    };
    let draft = PlanDraft {
        version: 1,
        modules: vec![ModuleDraft {
            id: "a".into(),
            dependencies: vec![],
            write_set: vec![],
            acceptance_refs: vec![],
        }],
        explanation: "no acceptance refs".into(),
    };
    let result = validate_draft(&input, &draft);
    assert!(
        matches!(result, Err(PlannerError::InvalidInput(_))),
        "expected InvalidInput for empty acceptance_refs, got: {result:?}",
    );
}
