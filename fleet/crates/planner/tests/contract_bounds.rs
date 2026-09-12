use planner::{validate_draft, ModuleDraft, PlanDraft, PlanInput, PlannerError, PlannerModel};
use std::path::PathBuf;

struct FakePlanner { draft: PlanDraft }

impl PlannerModel for FakePlanner {
    fn propose(&self, _input: &PlanInput) -> Result<PlanDraft, PlannerError> {
        Ok(self.draft.clone())
    }
}

fn load_input() -> PlanInput {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/blueprint-planner/two-modules.json");
    let data = std::fs::read_to_string(&path).expect("fixture not found");
    serde_json::from_str(&data).expect("invalid fixture JSON")
}

fn two_module_draft() -> PlanDraft {
    PlanDraft {
        version: 1,
        modules: vec![
            ModuleDraft {
                id: "module-a".into(), title: "Module A".into(),
                description: "First module".into(), dependencies: vec![],
            },
            ModuleDraft {
                id: "module-b".into(), title: "Module B".into(),
                description: "Second module".into(), dependencies: vec!["module-a".into()],
            },
        ],
        explanation: "Two modules for the blueprint-planner contract test".into(),
    }
}

#[test]
fn dag_to_planner_to_plan_review_contract() {
    let input = load_input();
    let planner = FakePlanner { draft: two_module_draft() };
    let proposed = planner.propose(&input).expect("propose must succeed");
    assert_eq!(proposed.modules.len(), 2, "expected 2 modules in draft");
    let report = validate_draft(&input, &proposed).expect("validate must succeed");
    assert_eq!(report.checked, 2, "checked must equal 2");
    assert_eq!(report.total, 2, "total must equal 2");
    let digest1 = serde_json::to_string(&proposed).expect("serialize draft");
    let digest2 = serde_json::to_string(&proposed).expect("serialize draft again");
    assert_eq!(digest1, digest2, "canonical serialization must be stable");
    assert!(!digest1.is_empty(), "digest must not be empty");
}
