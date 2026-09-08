//! The real benchmark: judges all 50 rows of the human-labelled agent-ui-human dataset through
//! the live keyless llm7 adapter and prints a full denominator + confusion breakdown. Needs
//! network and the `llm7` feature; `#[ignore]`d so `cargo test` stays offline by default.
//!
//! Run with: `cargo test -p fleet-judge --features llm7 --test benchmark -- --ignored --nocapture`

#![cfg(feature = "llm7")]

#[path = "benchmark/dataset.rs"]
mod dataset;
#[path = "benchmark/scoring.rs"]
mod scoring;

use fleet_judge::{judge, Candidate, Criteria, Verdict};
use fleet_judge::llm7::Llm7Judge;
use scoring::Scoreboard;

const LABELS: [&str; 4] = ["cli", "structured_api", "dom_click", "form"];

fn criteria() -> Criteria {
    Criteria {
        instructions: "Given a task prompt for an autonomous agent, pick which interface the \
            agent should use to carry it out: cli (a shell/scriptable command), structured_api \
            (a typed API/SDK call), dom_click (drive a GUI/browser by clicking), or form (fill \
            and submit a structured form)."
            .into(),
        labels: LABELS.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
#[ignore = "needs network; run explicitly, see module docs"]
fn scores_against_human_labels() {
    let rows = dataset::load();
    assert_eq!(rows.len(), 50, "benchmark dataset must have 50 rows");
    let model = Llm7Judge::new();
    let crit = criteria();
    let mut board = Scoreboard::default();

    for row in &rows {
        let candidate = Candidate {
            input: row.prompt.clone(),
        };
        match judge(&crit, &candidate, &model) {
            Ok(Verdict::Decided { label, .. }) => {
                board.record_decision(&row.preferred_ui, &label)
            }
            Ok(Verdict::Abstain { why }) => {
                eprintln!("[abstain] {}: {why}", row.id);
                board.record_abstain();
            }
            Err(e) => {
                eprintln!("[error] {}: {e}", row.id);
                board.record_error();
            }
        }
    }

    println!("=== fleet-judge benchmark: agent-ui-human (n={}) ===", rows.len());
    println!("{}", board.report());
    assert_eq!(board.total, 50, "every row must be counted exactly once");
}
