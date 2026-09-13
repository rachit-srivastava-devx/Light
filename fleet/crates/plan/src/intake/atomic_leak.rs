use super::super::atomic_types::AtomicRow;

pub(super) const DECISION_LEAK_WORDS: [&str; 9] = [
    "tbd",
    "todo",
    "to be decided",
    "decide",
    "design choice",
    "either",
    "unknown",
    "figure out",
    "determine",
];

pub(super) fn leaks_decision(row: &AtomicRow) -> bool {
    let joined = format!(
        "{} {} {} {}",
        row.description, row.inputs, row.outputs, row.acceptance
    )
    .to_lowercase();
    DECISION_LEAK_WORDS.iter().any(|w| joined.contains(w))
}
