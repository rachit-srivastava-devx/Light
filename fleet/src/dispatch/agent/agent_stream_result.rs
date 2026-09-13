//! The evidence side of `ClaudeStream`: turning captured provider metadata into the recorded
//! result body, resolved model, and token usage `finish()` reports -- as opposed to `push`'s
//! live text/milestone stream, which is UI-only and never touches these fields.

use serde_json::{json, Value};

use super::ClaudeStream;

impl ClaudeStream {
    // pub(in crate::dispatch), not pub(super): `agent_cmd_run.rs` calls these, and it's a
    // sibling of `agent_stream.rs` (this file's grandparent module), not a child of it.
    pub(in crate::dispatch) fn body(self) -> Value {
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

    pub(in crate::dispatch) fn model(&self) -> Option<String> {
        self.model.clone()
    }

    pub(in crate::dispatch) fn tokens(&self) -> Option<u64> {
        let usage = self.usage.as_ref()?.as_object()?;
        let input = usage.get("input_tokens").and_then(Value::as_u64)?;
        let output = usage.get("output_tokens").and_then(Value::as_u64)?;
        input.checked_add(output).filter(|total| *total > 0)
    }

    pub(super) fn capture_metadata(&mut self, value: &Value) {
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
