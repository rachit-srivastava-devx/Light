//! `collect_nodes`/`node_text`, ported from `graph.rs:826-861,933-936` onto this crate's types
//! (no `symbol_id`/`kind` bookkeeping -- callers assign identity via `SymbolId::derive`).

use crate::error::ContextError;
use crate::parse::extract_call::call;
use crate::parse::extract_macro_call::macro_body_call;
use crate::parse::extract_definition::definition;
use crate::types::Language;
use tree_sitter::Node;

/// One raw definition found while walking a file's parse tree.
pub struct RawSymbol {
    pub name: String,
    pub arity: u64,
    pub line: u64,
}

/// One raw call, `caller` indexing into the same file's `RawSymbol` list.
pub struct RawCall {
    pub caller: usize,
    pub callee_name: String,
    pub arity: u64,
}

pub fn collect_nodes(
    node: Node<'_>,
    source: &str,
    language: Language,
    symbols: &mut Vec<RawSymbol>,
    calls: &mut Vec<RawCall>,
    current: Option<usize>,
) -> Result<(), ContextError> {
    let mut next = current;
    if let Some((name, arity)) = definition(node, source, language)? {
        let line = node.start_position().row as u64 + 1;
        symbols.push(RawSymbol { name, arity, line });
        next = Some(symbols.len() - 1);
    }
    if let (Some(caller), Some((callee_name, arity))) = (next, call(node, source, language)?) {
        calls.push(RawCall {
            caller,
            callee_name,
            arity,
        });
    }
    // Rust only: a `token_tree` is a macro invocation's opaque argument list -- scan its named
    // children pairwise for `<name> <nested "(...)"  token_tree>` (see `macro_body_call`'s doc).
    if language == Language::Rust && node.kind() == "token_tree" {
        if let Some(caller) = next {
            let children: Vec<Node> = (0..node.named_child_count()).filter_map(|i| node.named_child(i)).collect();
            for pair in children.windows(2) {
                if let Some((callee_name, arity)) = macro_body_call(pair[0], pair[1], source) {
                    calls.push(RawCall { caller, callee_name, arity });
                }
            }
        }
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_nodes(child, source, language, symbols, calls, next)?;
        }
    }
    Ok(())
}

pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> Result<&'a str, ContextError> {
    node.utf8_text(source.as_bytes())
        .map_err(|_| ContextError::ParseError {
            path: String::new(),
        })
}
