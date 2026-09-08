//! Per-file tree-sitter parse + `SymbolId` assignment, split out of `repomap.rs` to keep both
//! files under the 80-line gate.

use crate::error::ContextError;
use crate::parse::{collect_nodes, RawCall};
use crate::types::{Language, SourceFile, SymbolId, SymbolRef};
use tree_sitter::Parser;

fn ts_language(language: Language) -> tree_sitter::Language {
    match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Bash => tree_sitter_bash::LANGUAGE.into(),
    }
}

/// One file's extracted symbols, their ids in definition order (for indexing
/// `RawCall::caller`), and its raw calls.
pub type ParsedFile = (Vec<SymbolRef>, Vec<SymbolId>, Vec<RawCall>);

/// Parses one file into its `ParsedFile` triple.
pub fn parse_file(file: &SourceFile) -> Result<ParsedFile, ContextError> {
    let mut parser = Parser::new();
    parser
        .set_language(&ts_language(file.language))
        .map_err(|_| ContextError::GrammarLoad(file.language))?;
    let tree = parser
        .parse(&file.source, None)
        .ok_or_else(|| ContextError::ParseError {
            path: file.path.clone(),
        })?;
    let mut raw_symbols = Vec::new();
    let mut calls = Vec::new();
    collect_nodes(
        tree.root_node(),
        &file.source,
        file.language,
        &mut raw_symbols,
        &mut calls,
        None,
    )?;

    let mut refs = Vec::with_capacity(raw_symbols.len());
    let mut ids = Vec::with_capacity(raw_symbols.len());
    for symbol in raw_symbols {
        let id = SymbolId::derive(&file.path, &symbol.name, symbol.arity);
        refs.push(SymbolRef {
            id: id.clone(),
            path: file.path.clone(),
            name: symbol.name,
            arity: symbol.arity,
            line: symbol.line,
        });
        ids.push(id);
    }
    Ok((refs, ids, calls))
}
