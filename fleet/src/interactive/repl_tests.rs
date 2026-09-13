use super::repl::DEFAULT_MODEL;

#[test]
fn default_model_uses_the_current_sonnet_alias() {
    assert_eq!(DEFAULT_MODEL, "sonnet");
}
