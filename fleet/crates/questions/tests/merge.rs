use questions::{merge_probes, Question};
use std::num::NonZeroU8;

fn q(text: &str, why: &str, severity: u8) -> Question {
    Question {
        text: text.to_string(),
        why: why.to_string(),
        severity,
    }
}

#[test]
fn output_capped_at_three() {
    let inputs = vec![
        vec![
            q("biz q1", "probe_business", 10),
            q("biz q2", "probe_business", 8),
        ],
        vec![q("tech q1", "probe_tech", 9), q("tech q2", "probe_tech", 7)],
        vec![
            q("learn q1", "probe_learn", 6),
            q("learn q2", "probe_learn", 5),
        ],
        vec![
            q("research q1", "probe_research", 4),
            q("research q2", "probe_research", 3),
        ],
    ];
    let result = merge_probes(inputs, NonZeroU8::new(3).unwrap()).unwrap();
    assert_eq!(result.items.len(), 3);
}

#[test]
fn all_probe_contributions_merged() {
    let inputs = vec![
        vec![q("business question", "probe_business", 9)],
        vec![q("tech question", "probe_tech", 8)],
        vec![q("learn question", "probe_learn", 7)],
        vec![q("research question", "probe_research", 6)],
    ];
    let result = merge_probes(inputs, NonZeroU8::new(3).unwrap()).unwrap();
    let sources: std::collections::HashSet<_> =
        result.items.iter().map(|q| q.why.as_str()).collect();
    assert!(
        sources.len() >= 3,
        "Expected items from at least 3 probe sources, got {:?}",
        sources
    );
    assert_eq!(result.items.len(), 3);
}

#[test]
fn empty_probe_output_tolerated() {
    let inputs = vec![
        vec![q("business question", "probe_business", 9)],
        vec![q("tech question", "probe_tech", 8)],
        vec![],
        vec![q("research question", "probe_research", 7)],
    ];
    let result = merge_probes(inputs, NonZeroU8::new(3).unwrap()).unwrap();
    assert!(!result.items.is_empty() && result.items.len() <= 3);
}

#[test]
fn question_with_empty_why_is_filtered() {
    // Kills: `replace && with ||` — if || were used, a question with non-empty text
    // but empty why would NOT be filtered, and the result would have 2 items.
    let inputs = vec![
        vec![q("valid question", "has a why", 9)],
        vec![q("no why here", "", 10)],
    ];
    let result = merge_probes(inputs, NonZeroU8::new(3).unwrap()).unwrap();
    assert_eq!(result.items.len(), 1, "empty-why question must be dropped");
    assert_eq!(result.items[0].text, "valid question");
}

#[test]
fn question_with_empty_text_is_filtered() {
    let inputs = vec![
        vec![q("", "has why but no text", 9)],
        vec![q("real question", "real why", 5)],
    ];
    let result = merge_probes(inputs, NonZeroU8::new(3).unwrap()).unwrap();
    assert_eq!(result.items.len(), 1, "empty-text question must be dropped");
}

#[test]
fn max_cap_one_is_valid() {
    // Kills: `replace > with <` — if < were used, max=1 would be rejected (1 < 3)
    let inputs = vec![vec![q("q1", "why1", 9), q("q2", "why2", 8)]];
    let result = merge_probes(inputs, NonZeroU8::new(1).unwrap()).unwrap();
    assert_eq!(result.items.len(), 1);
}

#[test]
fn max_cap_four_is_rejected() {
    use questions::merge_probes;
    let inputs: Vec<Vec<questions::Question>> = vec![];
    let result = merge_probes(inputs, NonZeroU8::new(4).unwrap());
    assert!(
        result.is_err(),
        "cap=4 must be rejected; only 1..=3 are valid"
    );
}
