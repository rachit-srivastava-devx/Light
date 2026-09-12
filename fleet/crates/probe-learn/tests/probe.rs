use probe_learn::{probe, LearnInput, LessonHit, MemoryReader, ProbeError};

struct FakeMemoryReader {
    lessons: Vec<LessonHit>,
}

impl MemoryReader for FakeMemoryReader {
    fn recall(&self, _q: &str, scope: &str, _limit: u32) -> Result<Vec<LessonHit>, ProbeError> {
        Ok(self.lessons.iter().filter(|l| l.scope == scope).cloned().collect())
    }
}

#[test]
fn ambiguity_request_retrieves_scoped_lessons() {
    let reader = FakeMemoryReader {
        lessons: vec![LessonHit {
            id: "1".into(),
            text: "ownership rules in Rust: each value has a single owner".into(),
            scope: "rust/borrowing".into(),
        }],
    };
    let input = LearnInput {
        text: "ownership and borrowing in Rust; scope: rust/borrowing".into(),
        scope: "rust/borrowing".into(),
    };
    let questions = probe(&input, &reader).unwrap();
    assert!(!questions.is_empty());
    let body = questions[0].text.to_lowercase();
    assert!(body.contains("ownership"), "expected 'ownership' in '{}', but not found", body);
}

#[test]
fn no_matching_lessons_returns_question() {
    let reader = FakeMemoryReader { lessons: vec![] };
    let input = LearnInput {
        text: "some requirement text".into(),
        scope: "general".into(),
    };
    let questions = probe(&input, &reader).unwrap();
    assert_eq!(questions.len(), 1);
    assert!(!questions[0].text.is_empty());
}

#[test]
fn returned_question_differs_by_scope() {
    let reader = FakeMemoryReader { lessons: vec![] };

    let input_alpha = LearnInput { text: "T".into(), scope: "alpha".into() };
    let input_beta = LearnInput { text: "T".into(), scope: "beta".into() };

    let q_alpha = probe(&input_alpha, &reader).unwrap();
    let q_beta = probe(&input_beta, &reader).unwrap();

    assert_ne!(
        q_alpha[0].text,
        q_beta[0].text,
        "Questions should differ by scope but got identical: '{}'",
        q_alpha[0].text
    );
}
