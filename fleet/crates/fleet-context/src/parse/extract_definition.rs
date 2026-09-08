//! `definition` -- per-language "is this node a function definition" + arity.
//! Ported verbatim from `graph.rs:863-890`.

use crate::error::ContextError;
use crate::parse::extract::node_text;
use crate::types::Language;
use tree_sitter::Node;

pub fn definition(
    node: Node<'_>,
    source: &str,
    language: Language,
) -> Result<Option<(String, u64)>, ContextError> {
    let kind = node.kind();
    let is_definition = match language {
        Language::Rust => kind == "function_item",
        Language::Python => kind == "function_definition",
        Language::Bash => kind == "function_definition",
    };
    if !is_definition {
        return Ok(None);
    }
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let name = node_text(name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }
    let arity = match node.child_by_field_name("parameters") {
        Some(parameters) => parameters.named_child_count() as u64,
        None => 0,
    };
    Ok(Some((name, arity)))
}
