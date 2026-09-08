//! `call` -- per-language "is this node a call" + callee name + arity, with the
//! `if`/`for`/`while`/`case` false-positive filter. Ported verbatim from `graph.rs:892-931`.

use crate::error::ContextError;
use crate::parse::extract::node_text;
use crate::types::Language;
use tree_sitter::Node;

pub fn call(
    node: Node<'_>,
    source: &str,
    language: Language,
) -> Result<Option<(String, u64)>, ContextError> {
    let kind = node.kind();
    let is_call = match language {
        Language::Rust => kind == "call_expression",
        Language::Python => kind == "call",
        Language::Bash => kind == "command",
    };
    if !is_call {
        return Ok(None);
    }
    let callee = if language == Language::Bash {
        node.child_by_field_name("name")
            .or_else(|| node.named_child(0))
    } else {
        node.child_by_field_name("function")
    };
    let Some(callee) = callee else {
        return Ok(None);
    };
    let raw_name = node_text(callee, source)?;
    let name = raw_name
        .trim()
        .trim_end_matches("()")
        .rsplit([':', '.'])
        .next()
        .unwrap_or(raw_name.trim())
        .trim()
        .to_string();
    if name.is_empty() || matches!(name.as_str(), "if" | "for" | "while" | "case") {
        return Ok(None);
    }
    let arity = if language == Language::Bash {
        node.named_child_count().saturating_sub(1) as u64
    } else {
        match node.child_by_field_name("arguments") {
            Some(arguments) => arguments.named_child_count() as u64,
            None => 0,
        }
    };
    Ok(Some((name, arity)))
}
