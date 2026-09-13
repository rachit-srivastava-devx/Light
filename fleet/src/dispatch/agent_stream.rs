//! Provider-output parsing for the direct Claude adapter.
//!
//! Claude's stream-json protocol is provider-owned and may gain fields. We retain its reported
//! usage object verbatim, extract only documented scalar evidence, and refuse a zero-event stream.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct ClaudeStream {
    response: String,
    result_text: Option<String>,
    model: Option<String>,
    usage: Option<Value>,
    saw_deltas: bool,
    saw_result: bool,
    root: PathBuf,
}

impl ClaudeStream {
    /// `root` is the worktree a tool-use `file_path` is displayed relative to, so a milestone
    /// reads `Edit(calc.py)` rather than the scratch/absolute path the provider reports.
    pub(super) fn with_root(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            ..Self::default()
        }
    }

    /// Returns provider text to print as it arrives: a genuine model text delta, a milestone line
    /// announcing a tool call the model just made (name + primary argument), or both when a
    /// single message carries both. A milestone is UI-only -- it is never folded into
    /// `response`/`body()`, which stay the model's own words (LLD §20: observability is
    /// additive, never a substitute for the recorded evidence).
    pub(super) fn push(&mut self, line: &str) -> Option<String> {
        let value: Value = serde_json::from_str(line).ok()?;
        self.capture_metadata(&value);
        let milestone = self.tool_milestone(&value);
        let text = streamed_text(&value, self.saw_deltas);
        self.saw_deltas |= is_delta(&value);
        if let Some(text) = &text {
            self.response.push_str(text);
        }
        match (milestone, text) {
            (Some(milestone), Some(text)) if !text.is_empty() => Some(milestone + &text),
            (Some(milestone), _) => Some(milestone),
            (None, Some(text)) => Some(text),
            (None, None) => None,
        }
    }

    /// A complete `assistant` message is the only place a tool call's name and arguments arrive
    /// in full (arguments stream in as `input_json_delta` fragments beforehand, which is raw
    /// token chatter, not a milestone -- LLD §7). One tool_use content part -> one milestone line.
    fn tool_milestone(&self, value: &Value) -> Option<String> {
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            return None;
        }
        let parts = value.pointer("/message/content")?.as_array()?;
        let mut out = String::new();
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            let name = part.get("name").and_then(Value::as_str).unwrap_or("tool");
            let label = match self.tool_use_detail(name, part.get("input")) {
                Some(detail) => format!("{name}({detail})"),
                None => name.to_string(),
            };
            out.push_str(&format!("\n\u{2192} {label}\n"));
        }
        (!out.is_empty()).then_some(out)
    }

    fn tool_use_detail(&self, name: &str, input: Option<&Value>) -> Option<String> {
        let input = input?.as_object()?;
        match name {
            "Read" | "Edit" | "Write" | "NotebookEdit" => {
                let path = input.get("file_path").and_then(Value::as_str)?;
                Some(
                    Path::new(path)
                        .strip_prefix(&self.root)
                        .unwrap_or(Path::new(path))
                        .display()
                        .to_string(),
                )
            }
            "Glob" | "Grep" => input.get("pattern").and_then(Value::as_str).map(str::to_owned),
            "Bash" => input
                .get("command")
                .and_then(Value::as_str)
                .map(|cmd| shorten(cmd, 72)),
            _ => None,
        }
    }

    pub(super) fn has_result(&self) -> bool {
        self.saw_result
    }

    pub(super) fn body(self) -> Value {
        let response = if self.response.is_empty() {
            self.result_text.clone().unwrap_or_default()
        } else {
            self.response
        };
        json!({
            "response": response,
            "result": self.result_text,
            "streamed": self.saw_deltas,
            "token_usage": self.usage,
        })
    }

    pub(super) fn model(&self) -> Option<String> {
        self.model.clone()
    }

    pub(super) fn tokens(&self) -> Option<u64> {
        let usage = self.usage.as_ref()?.as_object()?;
        let input = usage.get("input_tokens").and_then(Value::as_u64)?;
        let output = usage.get("output_tokens").and_then(Value::as_u64)?;
        input.checked_add(output).filter(|total| *total > 0)
    }

    fn capture_metadata(&mut self, value: &Value) {
        if self.model.is_none() {
            self.model = value
                .get("model")
                .or_else(|| value.pointer("/message/model"))
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        if value.get("type").and_then(Value::as_str) == Some("result") {
            self.saw_result = true;
            self.result_text = value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned);
            self.usage = value.get("usage").cloned();
        }
    }
}

fn streamed_text(value: &Value, saw_deltas: bool) -> Option<String> {
    if is_delta(value) {
        return value
            .pointer("/event/delta/text")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    let kind = value.get("type").and_then(Value::as_str)?;
    if kind == "assistant" {
        if saw_deltas {
            return None;
        }
        return value
            .pointer("/message/content")?
            .as_array()?
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<String>()
            .into();
    }
    if kind == "content_block_delta" {
        return value
            .pointer("/delta/text")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    None
}

fn is_delta(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("stream_event")
        && value.pointer("/event/type").and_then(Value::as_str) == Some("content_block_delta")
}

/// A milestone line is a one-line UI announcement, not a transcript -- cap it so one long
/// generated command (or a multi-line heredoc) can't push the actual model text off screen.
fn shorten(text: &str, max: usize) -> String {
    let first_line = text.lines().next().unwrap_or("");
    let truncated = first_line.chars().count() > max;
    let kept: String = first_line.chars().take(max).collect();
    if truncated || first_line.len() != text.len() {
        format!("{kept}…")
    } else {
        kept
    }
}

#[cfg(test)]
mod tests {
    use super::ClaudeStream;

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
        assert!(delta.contains('\u{2026}'), "expected an ellipsis in {delta:?}");
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
}
