//! Tool-call milestone rendering -- turns a complete `assistant` message's `tool_use` parts into
//! the one-line `→ Read(calc.py)` style announcements the interactive REPL streams live (see the
//! module doc on `ClaudeStream::push`). Free functions, not methods: nothing here needs anything
//! from `ClaudeStream` besides `root`, passed explicitly so this stays testable without a stream.

use serde_json::Value;
use std::path::Path;

use super::agent_stream_parse::shorten;

/// A complete `assistant` message is the only place a tool call's name and arguments arrive in
/// full (arguments stream in as `input_json_delta` fragments beforehand, which is raw token
/// chatter, not a milestone -- LLD §7). One tool_use content part -> one milestone line.
pub(super) fn tool_milestone(root: &Path, value: &Value) -> Option<String> {
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
        let label = match tool_use_detail(root, name, part.get("input")) {
            Some(detail) => format!("{name}({detail})"),
            None => name.to_string(),
        };
        out.push_str(&format!("\n\u{2192} {label}\n"));
    }
    (!out.is_empty()).then_some(out)
}

fn tool_use_detail(root: &Path, name: &str, input: Option<&Value>) -> Option<String> {
    let input = input?.as_object()?;
    match name {
        "Read" | "Edit" | "Write" | "NotebookEdit" => {
            let path = input.get("file_path").and_then(Value::as_str)?;
            Some(
                Path::new(path)
                    .strip_prefix(root)
                    .unwrap_or(Path::new(path))
                    .display()
                    .to_string(),
            )
        }
        "Glob" | "Grep" => input
            .get("pattern")
            .and_then(Value::as_str)
            .map(str::to_owned),
        "Bash" => input
            .get("command")
            .and_then(Value::as_str)
            .map(|cmd| shorten(cmd, 72)),
        _ => None,
    }
}
