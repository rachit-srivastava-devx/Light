//! Tells the worker model, in the prompt, the output shape `apply::parse` actually accepts.
//!
//! Without this the raw task went straight to the model, which answered in ordinary prose with a
//! bare ```rust fence. `apply/parse.rs` requires a DECLARED TARGET PATH and refuses rather than
//! guessing a filename, so a perfectly cooperative model produced `applied_files: []`, the
//! change-honesty check downgraded the claimed `Done` to `Refused`, and `fleet swarm` could never
//! apply anything. Every layer behaved correctly; nobody had told the model the contract.
//!
//! The wording below mirrors `apply/parse.rs` exactly -- fence info string `path:<file>`, or a
//! `path:`/`file:` line immediately above the fence. If that parser's accepted syntax changes,
//! this text must change with it; `contract_names_the_syntax_the_parser_accepts` fails if the
//! two drift apart.

/// Wrap a task in the output contract. Pure -- no IO, no env.
pub fn with_edit_contract(task: &str) -> String {
    format!(
        "{task}\n\n\
         ---\n\
         OUTPUT FORMAT (required -- a reply that ignores this is discarded unapplied):\n\
         Return the COMPLETE new contents of every file you change, each in its own fenced code \
         block whose target path is declared. Declare the path in either form:\n\
         \n\
         ```path:src/lib.rs\n\
         <the entire file, not a fragment or a diff>\n\
         ```\n\
         \n\
         or put a line reading `path: src/lib.rs` immediately above the fence.\n\
         \n\
         Paths are relative to the repository root. Do not use absolute paths or `..`. A fence \
         with no declared path is refused rather than guessed at, so an explanation containing a \
         bare ```rust block applies nothing. Prose outside the fences is fine and ignored."
    )
}

#[cfg(test)]
mod tests {
    use super::with_edit_contract;
    use crate::freelane::apply::extract_fences;

    #[test]
    fn the_task_survives_verbatim() {
        assert!(with_edit_contract("add mul to src/lib.rs").starts_with("add mul to src/lib.rs"));
    }

    /// The contract is only worth anything if the syntax it advertises is the syntax the parser
    /// accepts. This feeds the contract's own example back through the real parser, so the two
    /// cannot drift into disagreeing -- the failure mode that made this file necessary.
    #[test]
    fn contract_names_the_syntax_the_parser_accepts() {
        let text = with_edit_contract("t");
        let fences = extract_fences(&text);
        assert!(!fences.is_empty(), "the contract must show at least one example fence");
        let declared: Vec<&str> =
            fences.iter().filter_map(|f| f.declared_path.as_deref()).collect();
        assert!(
            declared.contains(&"src/lib.rs"),
            "the parser did not recognise the contract's own example: {declared:?}"
        );
    }
}
