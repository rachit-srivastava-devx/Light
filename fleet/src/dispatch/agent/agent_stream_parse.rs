//! Pure provider-JSON text extraction -- no `ClaudeStream` state, just "what text (if any) does
//! this one line of stream-json carry."

use serde_json::Value;

pub(super) fn streamed_text(value: &Value, saw_deltas: bool) -> Option<String> {
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

pub(super) fn is_delta(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("stream_event")
        && value.pointer("/event/type").and_then(Value::as_str) == Some("content_block_delta")
}

/// A milestone line is a one-line UI announcement, not a transcript -- cap it so one long
/// generated command (or a multi-line heredoc) can't push the actual model text off screen.
pub(super) fn shorten(text: &str, max: usize) -> String {
    let first_line = text.lines().next().unwrap_or("");
    let truncated = first_line.chars().count() > max;
    let kept: String = first_line.chars().take(max).collect();
    if truncated || first_line.len() != text.len() {
        format!("{kept}…")
    } else {
        kept
    }
}
