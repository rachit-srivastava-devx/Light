//! Kills cycle-detection and max_modules mutations added to validate.rs.
use planner::{validate_draft, ModuleDraft, PlanDraft, PlanInput, PlannerError};

fn valid_input() -> PlanInput {
    PlanInput {
        task_digest: "t1".into(),
        recipe_digest: "r1".into(),
        context_digest: "c1".into(),
        acceptance_refs: vec!["ac1".into()],
        max_modules: 5,
    }
}

#[test]
fn cycle_in_modules_is_rejected() {
    let draft = PlanDraft {
        version: 1,
        modules: vec![
            ModuleDraft {
                id: "a".into(),
                dependencies: vec!["b".into()],
                write_set: vec![],
                acceptance_refs: vec![],
            },
            ModuleDraft {
                id: "b".into(),
                dependencies: vec!["a".into()],
                write_set: vec![],
                acceptance_refs: vec![],
            },
        ],
        explanation: "cycle".into(),
    };
    let result = validate_draft(&valid_input(), &draft);
    assert!(
        matches!(result, Err(PlannerError::InvalidDraft(_))),
        "cycle A→B→A must be rejected, got: {result:?}",
    );
}

#[test]
fn draft_exceeds_max_modules_is_rejected() {
    let input = PlanInput {
        max_modules: 2,
        ..valid_input()
    };
    let draft = PlanDraft {
        version: 1,
        modules: vec![
            ModuleDraft {
                id: "a".into(),
                dependencies: vec![],
                write_set: vec![],
                acceptance_refs: vec![],
            },
            ModuleDraft {
                id: "b".into(),
                dependencies: vec![],
                write_set: vec![],
                acceptance_refs: vec![],
            },
            ModuleDraft {
                id: "c".into(),
                dependencies: vec![],
                write_set: vec![],
                acceptance_refs: vec![],
            },
        ],
        explanation: "too many".into(),
    };
    let result = validate_draft(&input, &draft);
    assert!(
        matches!(result, Err(PlannerError::InvalidDraft(_))),
        "3 modules when max_modules=2 must be rejected, got: {result:?}",
    );
}
