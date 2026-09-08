//! Aggregates one judged benchmark run. Publishes the full denominator: correct/attempted/total
//! plus abstentions and errors counted separately -- an abstention is never scored as wrong.

use std::collections::BTreeMap;

#[derive(Default)]
pub struct Scoreboard {
    pub total: usize,
    pub correct: usize,
    pub wrong: usize,
    pub abstained: usize,
    pub errored: usize,
    /// (actual_label, predicted_label) -> count, decided rows only.
    pub confusion: BTreeMap<(String, String), usize>,
}

impl Scoreboard {
    pub fn record_decision(&mut self, actual: &str, predicted: &str) {
        self.total += 1;
        *self
            .confusion
            .entry((actual.to_owned(), predicted.to_owned()))
            .or_insert(0) += 1;
        if actual == predicted {
            self.correct += 1;
        } else {
            self.wrong += 1;
        }
    }

    pub fn record_abstain(&mut self) {
        self.total += 1;
        self.abstained += 1;
    }

    pub fn record_error(&mut self) {
        self.total += 1;
        self.errored += 1;
    }

    /// Attempted = rows the judge actually decided on (excludes abstentions and errors).
    pub fn attempted(&self) -> usize {
        self.correct + self.wrong
    }

    pub fn report(&self) -> String {
        let mut out = format!(
            "correct/attempted/total = {}/{}/{}  (abstained={}, errored={})\n",
            self.correct,
            self.attempted(),
            self.total,
            self.abstained,
            self.errored
        );
        out.push_str("confusion (actual -> predicted: count):\n");
        for ((actual, predicted), count) in &self.confusion {
            out.push_str(&format!("  {actual} -> {predicted}: {count}\n"));
        }
        out
    }
}
