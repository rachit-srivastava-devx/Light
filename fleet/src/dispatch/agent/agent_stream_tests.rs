use super::ClaudeStream;

#[path = "agent_stream_tools_tests.rs"]
mod tool_tests;

#[test]
fn keeps_provider_text_model_and_usage_without_using_the_requested_alias() {
    let mut stream = ClaudeStream::default();
    assert_eq!(
        stream.push(r#"{"type":"assistant","message":{"model":"claude-sonnet-4","content":[{"type":"text","text":"Hello"}]}}"#),
        Some("Hello".to_string())
    );
    assert!(stream.push(r#"{"type":"result","result":"Hello","usage":{"input_tokens":11,"output_tokens":7}}"#).is_none());
    assert_eq!(stream.model().as_deref(), Some("claude-sonnet-4"));
    assert_eq!(stream.tokens(), Some(18));
    assert_eq!(stream.body()["response"], "Hello");
}

#[test]
fn zero_or_non_json_lines_are_not_provider_success_evidence() {
    let mut stream = ClaudeStream::default();
    assert!(stream.push("waiting for approval").is_none());
    assert!(!stream.has_result());
}

#[test]
fn renders_claudes_nested_deltas_once_and_keeps_final_usage() {
    let mut stream = ClaudeStream::default();
    assert_eq!(
        stream.push(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"REA"}}}"#),
        Some("REA".to_string())
    );
    assert_eq!(
        stream.push(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"DY"}}}"#),
        Some("DY".to_string())
    );
    assert!(stream.push(r#"{"type":"assistant","message":{"model":"claude-sonnet-5","content":[{"type":"text","text":"READY"}]}}"#).is_none());
    assert!(stream
        .push(r#"{"type":"result","usage":{"input_tokens":2,"output_tokens":4}}"#)
        .is_none());
    assert_eq!(stream.tokens(), Some(6));
    let body = stream.body();
    assert_eq!(body["response"], "READY");
    assert_eq!(body["streamed"], true);
}

#[test]
fn result_text_is_shown_when_a_provider_did_not_emit_deltas() {
    let mut stream = ClaudeStream::default();
    assert!(stream
        .push(r#"{"type":"result","result":"Complete","usage":{"input_tokens":2,"output_tokens":4}}"#)
        .is_none());
    assert_eq!(stream.body()["response"], "Complete");
}
