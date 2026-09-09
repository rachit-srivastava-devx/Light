//! Best-effort call detection *inside* a macro invocation's argument list (Rust only). Split out
//! of `extract_call.rs` to keep both files under the 80-line gate.
//!
//! Tree-sitter-rust does not parse macro arguments as expressions: `println!("{}", add(2, 3))`
//! has no `call_expression` node for `add(2, 3)` at all -- the whole argument list is one opaque
//! `token_tree`, with `add` and the nested `(2, 3)` appearing as a bare `identifier` followed by
//! a nested `token_tree` (confirmed via `tree.root_node().to_sexp()` on that exact snippet).
//! This scans consecutive named children of a `token_tree` for `<name> <token_tree of "(...)">`.

use crate::parse::extract::node_text;
use tree_sitter::Node;

/// Arity is deliberately reported as `0` here -- a placeholder, not a real count -- because
/// counting commas inside a token tree can't distinguish top-level args from nested ones cheaply
/// and correctly; `resolve_callee`'s unique-name fallback does not consult arity, so a wrong
/// placeholder can only ever cost precision (an extra, still-correct edge), never resolve wrong.
pub fn macro_body_call(name_node: Node<'_>, args_node: Node<'_>, source: &str) -> Option<(String, u64)> {
    if args_node.kind() != "token_tree" {
        return None;
    }
    if !matches!(name_node.kind(), "identifier" | "scoped_identifier" | "field_expression") {
        return None;
    }
    let opens_with_paren = node_text(args_node, source).ok()?.trim_start().starts_with('(');
    if !opens_with_paren {
        return None;
    }
    let raw_name = node_text(name_node, source).ok()?;
    let name = raw_name.trim().rsplit([':', '.']).next().unwrap_or(raw_name.trim()).trim().to_string();
    if name.is_empty() || matches!(name.as_str(), "if" | "for" | "while" | "case") {
        return None;
    }
    Some((name, 0))
}
