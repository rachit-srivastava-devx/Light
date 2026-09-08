//! Byte-faithful port of `validate_challenges` (`intake.sh:243-254`) EXCLUDING
//! `challenge_source_exists`'s own file grep (`intake.sh:236-242`) -- `corpus_known` stands in
//! for it: `true` iff the caller already resolved that exact `source` string against
//! `FAILURE-CORPUS.md`/`THREAD-LESSONS.md`.

use super::sow::StageViolation;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeRow {
    pub id: String,
    pub source: String,
    pub affected_leaf: String,
    pub risk: String,
    pub trigger: String,
    pub mitigation: String,
}

pub fn validate_challenge_rows(
    rows: &[ChallengeRow],
    atomic_ids: &BTreeSet<String>,
    corpus_known: impl Fn(&str) -> bool,
) -> Vec<StageViolation> {
    let mut out = Vec::new();
    if rows.is_empty() {
        out.push(StageViolation("challenge register has no rows".into()));
        return out;
    }
    for row in rows {
        if !corpus_known(&row.source) {
            out.push(StageViolation(format!("challenge {} cites no known corpus row: {}", row.id, row.source)));
        }
        if !atomic_ids.contains(&row.affected_leaf) {
            out.push(StageViolation(format!("challenge {} points at unknown atomic leaf: {}", row.id, row.affected_leaf)));
        }
        if row.risk.is_empty() || row.trigger.is_empty() || row.mitigation.is_empty() {
            out.push(StageViolation(format!("challenge {} needs risk, trigger, and mitigation", row.id)));
        }
    }
    out
}
