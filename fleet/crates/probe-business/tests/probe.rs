use probe_business::{probe, BusinessInput, BusinessReader, ProbeError};

struct FakeBusinessReader {
    response: String,
}

impl BusinessReader for FakeBusinessReader {
    fn recall(&self, _req: &str) -> Result<String, ProbeError> {
        Ok(self.response.clone())
    }
}

#[test]
fn ambiguity_request_produces_question() {
    let input = BusinessInput {
        text: "Build a payment gateway; e-commerce startup context".into(),
    };
    let reader = FakeBusinessReader {
        response: "Consider payment flows".into(),
    };
    let questions = probe(&input, &reader).unwrap();
    assert_eq!(questions.len(), 1);
    assert!(!questions[0].text.is_empty());
}

#[test]
fn empty_context_still_returns_question() {
    let input = BusinessInput {
        text: "Deploy service".into(),
    };
    let reader = FakeBusinessReader {
        response: String::new(),
    };
    let questions = probe(&input, &reader).unwrap();
    assert_eq!(questions.len(), 1);
}

#[test]
fn question_body_contains_domain_term() {
    let input = BusinessInput {
        text: "Redesign onboarding flow; B2B SaaS product context".into(),
    };
    let reader = FakeBusinessReader {
        response: String::new(),
    };
    let questions = probe(&input, &reader).unwrap();
    let body = questions[0].text.to_lowercase();
    let domain_terms = [
        "stakeholder",
        "requirement",
        "constraint",
        "scope",
        "budget",
    ];
    assert!(
        domain_terms.iter().any(|t| body.contains(t)),
        "question '{}' does not contain any domain term",
        body
    );
}
