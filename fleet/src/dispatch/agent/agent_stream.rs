//! Provider-output parsing for the direct Claude adapter.
//!
//! Claude's stream-json protocol is provider-owned and may gain fields. We retain its reported
//! usage object verbatim, extract only documented scalar evidence, and refuse a zero-event stream.

use serde_json::Value;
use std::path::{Path, PathBuf};

#[path = "agent_stream_parse.rs"]
mod agent_stream_parse;
#[path = "agent_stream_result.rs"]
mod agent_stream_result;
#[path = "agent_stream_tools.rs"]
mod agent_stream_tools;
use agent_stream_parse::{is_delta, streamed_text};
use agent_stream_tools::tool_milestone;

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
        let milestone = tool_milestone(&self.root, &value);
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

    pub(super) fn has_result(&self) -> bool {
        self.saw_result
    }
}

#[cfg(test)]
#[path = "agent_stream_tests.rs"]
mod tests;
