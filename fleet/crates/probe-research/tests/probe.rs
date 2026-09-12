use probe_research::{probe, ResearchError, ResearchInput, ResearchPort, Source};

fn src(title: &str) -> Source {
    Source { url: "https://ex.com".into(), title: title.into(), date: "2025-01-01".into(), uncertainty: 0.2 }
}

struct FakePort(String);
impl ResearchPort for FakePort {
    fn search(&self, _q: &str, _ms: u64) -> Result<Vec<Source>, ResearchError> {
        Ok(vec![src(&self.0)])
    }
}

struct TimeoutPort;
impl ResearchPort for TimeoutPort {
    fn search(&self, _q: &str, _ms: u64) -> Result<Vec<Source>, ResearchError> {
        Err(ResearchError::Timeout)
    }
}

struct DeadlinePort;
impl ResearchPort for DeadlinePort {
    fn search(&self, _q: &str, deadline_ms: u64) -> Result<Vec<Source>, ResearchError> {
        if deadline_ms == 0 { Ok(vec![]) } else { Ok(vec![src("result")]) }
    }
}

fn input(unknown: &str) -> ResearchInput {
    ResearchInput { text: "context text".into(), unknown: unknown.into(), deadline_ms: 100 }
}

#[test]
fn ambiguity_request_produces_research_question() {
    let r = probe(&input("OAuth scope"), &FakePort("OAuth 2.0 scopes define access".into())).unwrap();
    assert_eq!(r.questions.len(), 1);
    assert!(!r.questions[0].text.is_empty());
}

#[test]
fn network_timeout_returns_fallback_question() {
    let i = ResearchInput { deadline_ms: 50, ..input("OAuth scope") };
    let r = probe(&i, &TimeoutPort).unwrap();
    assert_eq!(r.questions.len(), 1);
    let body = r.questions[0].text.to_lowercase();
    assert!(body.contains("unable to retrieve") || body.contains("timeout") || body.contains("unavailable"),
        "expected timeout message in '{}', but not found", body);
}

#[test]
fn question_references_external_source() {
    let r = probe(&input("cert chain format"), &FakePort("See the rustls documentation".into())).unwrap();
    let body = r.questions[0].text.to_lowercase();
    assert!(["research", "documentation", "reference", "source"].iter().any(|t| body.contains(t)),
        "expected external source reference in '{}', but none found", body);
}

#[test]
fn source_fields_are_populated() {
    let r = probe(&input("TLS 1.3"), &FakePort("RFC 8446".into())).unwrap();
    assert_eq!(r.sources.len(), 1);
    let s = &r.sources[0];
    assert!(!s.url.is_empty(), "url must be set");
    assert!(!s.title.is_empty(), "title must be set");
    assert!(!s.date.is_empty(), "date must be set");
    assert!((0.0_f32..=1.0).contains(&s.uncertainty), "uncertainty must be in [0,1]");
}

#[test]
fn deadline_zero_ms_returns_empty_sources() {
    let i = ResearchInput { deadline_ms: 0, ..input("something") };
    let r = probe(&i, &DeadlinePort).unwrap();
    assert_eq!(r.sources.len(), 0, "zero deadline should yield no sources");
}

#[test]
fn empty_query_text_returns_error() {
    let bad = ResearchInput { text: "".into(), unknown: "x".into(), deadline_ms: 100 };
    assert!(matches!(probe(&bad, &FakePort("t".into())), Err(ResearchError::EmptyQuery)));
}
