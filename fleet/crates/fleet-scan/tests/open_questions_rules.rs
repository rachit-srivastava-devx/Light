//! Jaccard-similarity contract and `OpenQuestions` construction bounds, BLUEPRINT §9.

mod common;
use common::q;

use fleet_scan::{jaccard_similarity, GapSeverity, OpenQuestionsError, ProbeKind, Question};

#[test]
fn jaccard_similarity_is_symmetric_and_bounded() {
    let a = "what is the target audience";
    let b = "who is the target audience for this";
    assert_eq!(jaccard_similarity(a, b), jaccard_similarity(b, a));
    assert!((0.0..=1.0).contains(&jaccard_similarity(a, b)));
    assert_eq!(jaccard_similarity("", ""), 0.0);
}

#[test]
fn open_questions_rejects_empty() {
    assert_eq!(fleet_scan::OpenQuestions::new(vec![]), Err(OpenQuestionsError::Empty));
}

#[test]
fn open_questions_rejects_more_than_four() {
    let items = vec![q("a", "w", GapSeverity::Low, ProbeKind::Business); 5];
    assert_eq!(fleet_scan::OpenQuestions::new(items), Err(OpenQuestionsError::TooMany(5)));
}

#[test]
fn open_questions_accepts_one_to_four() {
    for n in 1..=4 {
        let items: Vec<Question> = (0..n)
            .map(|i| q(&format!("q{i}"), "w", GapSeverity::Low, ProbeKind::Business))
            .collect();
        assert!(fleet_scan::OpenQuestions::new(items).is_ok());
    }
}

#[test]
fn into_vec_returns_all_questions() {
    let items = vec![
        q("q1", "why1", GapSeverity::High, ProbeKind::Business),
        q("q2", "why2", GapSeverity::Low, ProbeKind::Technical),
    ];
    let oq = fleet_scan::OpenQuestions::new(items.clone()).unwrap();
    let out = oq.into_vec();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].text, "q1");
    assert_eq!(out[1].text, "q2");
}
