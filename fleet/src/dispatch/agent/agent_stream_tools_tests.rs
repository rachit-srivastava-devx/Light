use super::ClaudeStream;

#[test]
fn tool_use_becomes_a_milestone_but_never_pollutes_the_recorded_response() {
    use std::path::Path;
    let mut stream = ClaudeStream::with_root(Path::new("/repo"));
    let delta = stream.push(
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/repo/calc.py"}}]}}"#,
    );
    assert_eq!(delta.as_deref(), Some("\n\u{2192} Read(calc.py)\n"));
    // A milestone is UI-only: the persisted response body must stay empty, not the label.
    assert_eq!(stream.body()["response"], "");
}

#[test]
fn bash_tool_use_shows_a_shortened_command() {
    let mut stream = ClaudeStream::default();
    let long_cmd = "cargo test --workspace --no-fail-fast -- --this-is-a-very-long-flag-list-to-force-truncation";
    let line = serde_json::json!({
        "type": "assistant",
        "message": {"content": [{"type": "tool_use", "name": "Bash", "input": {"command": long_cmd}}]},
    })
    .to_string();
    let delta = stream.push(&line).unwrap();
    assert!(delta.starts_with("\n\u{2192} Bash(cargo test --workspace"));
    assert!(
        delta.contains('\u{2026}'),
        "expected an ellipsis in {delta:?}"
    );
    assert!(delta.len() < long_cmd.len());
}

#[test]
fn mixed_text_and_tool_use_in_one_message_keeps_both() {
    let mut stream = ClaudeStream::default();
    let delta = stream.push(
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Checking now."},{"type":"tool_use","name":"Glob","input":{"pattern":"**/calc.py"}}]}}"#,
    );
    let delta = delta.expect("mixed message yields a delta");
    assert!(delta.contains("Glob(**/calc.py)"));
    assert!(delta.ends_with("Checking now."));
    assert_eq!(stream.body()["response"], "Checking now.");
}

#[test]
fn tool_result_messages_are_silent_and_not_provider_success_evidence() {
    let mut stream = ClaudeStream::default();
    assert!(stream
        .push(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"calc.py"}]}}"#)
        .is_none());
    assert!(!stream.has_result());
}
