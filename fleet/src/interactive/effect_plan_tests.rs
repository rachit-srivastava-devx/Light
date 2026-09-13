use super::approval::{approved, record_refusal, requires_approval};
use super::effect_plan::{discover, Effect};

#[test]
fn local_only_greeting_prompt_discloses_model_write_and_test_effects() {
    let prompt =
        "Add a pure helper that formats a greeting, test it, and do not publish or push anything.";
    assert_eq!(
        discover(prompt),
        vec![Effect::Model, Effect::LocalWrite, Effect::LocalCommand]
    );
}

#[test]
fn remote_and_deferred_requests_are_never_hidden_in_the_plan() {
    let effects = discover("Schedule a release tomorrow and publish a PR");
    assert_eq!(
        effects,
        vec![
            Effect::Model,
            Effect::RemotePublish,
            Effect::DeferredSchedule
        ]
    );
}

#[test]
fn only_an_explicit_yes_approves_effects() {
    assert!(approved(" yes "));
    assert!(!approved("yesterday"));
    assert!(!approved(""));
}

#[test]
fn conversational_model_only_prompt_runs_without_a_permission_interruption() {
    assert!(!requires_approval(&[Effect::Model]));
    assert!(requires_approval(&[Effect::Model, Effect::LocalWrite]));
}

#[test]
fn explanatory_and_substring_prompts_do_not_request_effects() {
    for prompt in [
        "explain how to run tests",
        "what is the latest release?",
        "describe how credit checking works",
    ] {
        assert_eq!(discover(prompt), vec![Effect::Model], "prompt={prompt}");
    }
}

#[test]
fn imperative_synonyms_cannot_bypass_write_or_publish_approval() {
    assert!(discover("please refactor the parser").contains(&Effect::LocalWrite));
    assert!(discover("update the dependency").contains(&Effect::LocalWrite));
    assert!(discover("deploy this release").contains(&Effect::RemotePublish));
    assert!(discover("open a pull request").contains(&Effect::RemotePublish));
}

#[test]
fn deferred_imperatives_keep_both_effects() {
    let effects = discover("run the tests after 2 hours");
    assert!(effects.contains(&Effect::LocalCommand));
    assert!(effects.contains(&Effect::DeferredSchedule));
    assert_eq!(
        discover("after 2 hours"),
        vec![Effect::Model, Effect::DeferredSchedule]
    );
}

#[test]
fn denied_effects_write_a_parent_attributed_receipt() {
    let state = tempfile::tempdir().expect("state dir");
    let effects = discover("Add a test");
    record_refusal(state.path(), "Add a test", &effects, "user denied approval").expect("receipt");
    let chain = std::fs::read_to_string(state.path().join("ledger.chain")).expect("ledger");
    let row: serde_json::Value = serde_json::from_str(chain.trim()).expect("json receipt");
    assert_eq!(row["event"], "refusal");
    assert_eq!(row["actor"], "fleet-cli-interactive");
    assert_eq!(row["body"]["reason"], "user denied approval");
}
