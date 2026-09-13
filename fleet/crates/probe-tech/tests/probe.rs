use probe_tech::{probe, CodebaseReader, ProbeError, SymbolFact, TechnicalInput};

struct FakeReader;

impl CodebaseReader for FakeReader {
    fn symbol(&self, q: &str) -> Result<SymbolFact, ProbeError> {
        Ok(SymbolFact {
            query: q.into(),
            path: "src/auth.rs".into(),
            start_line: 1,
            end_line: 10,
            exists: true,
        })
    }
}

#[test]
fn ambiguity_request_produces_question() {
    let input = TechnicalInput {
        text: "Implement authentication module; see src/auth.rs for existing types".into(),
        repo_digest: "abc123".into(),
    };
    let questions = probe(&input, &FakeReader).unwrap();
    assert_eq!(questions.len(), 1);
    assert!(!questions[0].text.is_empty());
}

#[test]
fn question_references_language_construct() {
    let input = TechnicalInput {
        text: "Implement authentication module; see src/auth.rs for existing types".into(),
        repo_digest: "abc123".into(),
    };
    let questions = probe(&input, &FakeReader).unwrap();
    let body = questions[0].text.to_lowercase();
    let terms = [
        "function",
        "struct",
        "trait",
        "module",
        "dependency",
        "interface",
        "type",
    ];
    assert!(
        terms.iter().any(|t| body.contains(t)),
        "question '{}' lacks language construct term",
        body
    );
}

#[test]
fn empty_file_context_returns_question() {
    let input = TechnicalInput {
        text: "".into(),
        repo_digest: "abc123".into(),
    };
    let questions = probe(&input, &FakeReader).unwrap();
    assert_eq!(questions.len(), 1);
    assert!(!questions[0].text.is_empty());
}
