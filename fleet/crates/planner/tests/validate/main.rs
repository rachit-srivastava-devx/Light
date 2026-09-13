//! Unit tests targeting the validate_draft guard conditions.
//! Kills the two `||`→`&&` mutations at types.rs:48.

use planner::{validate_draft, ModuleDraft, PlanDraft, PlanInput};

fn valid_input() -> PlanInput {
    PlanInput {
        task_digest: "t1".into(),
        recipe_digest: "r1".into(),
        context_digest: "c1".into(),
        acceptance_refs: vec!["ac1".into()],
        max_modules: 5,
    }
}

fn one_module_draft() -> PlanDraft {
    PlanDraft {
        version: 1,
        modules: vec![ModuleDraft {
            id: "m1".into(),
            dependencies: vec![],
            write_set: vec![],
            acceptance_refs: vec!["ac1".into()],
        }],
        explanation: "ok".into(),
    }
}

#[test]
fn recipe_digest_empty_is_rejected() {
    // Kills: `replace first || with &&` — with &&, only rejecting when BOTH task AND recipe
    // are empty; an empty recipe_digest alone would pass.
    let mut input = valid_input();
    input.recipe_digest = String::new();
    assert!(validate_draft(&input, &one_module_draft()).is_err());
}

#[test]
fn context_digest_empty_is_rejected() {
    // Kills: `replace second || with &&` — with &&, only rejecting when (task || recipe) AND
    // context are empty; an empty context_digest alone would pass.
    let mut input = valid_input();
    input.context_digest = String::new();
    assert!(validate_draft(&input, &one_module_draft()).is_err());
}

#[test]
fn task_digest_empty_is_rejected() {
    let mut input = valid_input();
    input.task_digest = String::new();
    assert!(validate_draft(&input, &one_module_draft()).is_err());
}

#[test]
fn all_digests_present_passes() {
    let report =
        validate_draft(&valid_input(), &one_module_draft()).expect("valid input must pass");
    assert_eq!(report.checked, report.total);
    assert!(report.total > 0);
}
