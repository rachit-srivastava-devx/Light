//! Byte-faithful port of `validate_clarifications` (`intake.sh:272-284`) EXCLUDING
//! `clarification_ref_exists`'s own file lookups (`intake.sh:264-271`) -- `ref_known` stands in.

use super::sow::StageViolation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClarificationKind {
    Business,
    Technical,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClarificationRow {
    pub id: String,
    pub kind: ClarificationKind,
    pub gap_ref: String,
    pub question: String,
    pub blocking: bool,
    pub answer: String,
}

const GENERIC_PREFIXES: [&str; 4] =
    ["what do you want", "tell me more", "provide more detail", "any other requirements"];

fn is_generic(question: &str) -> bool {
    let lc = question.to_lowercase();
    GENERIC_PREFIXES.iter().any(|p| lc.starts_with(p))
}

pub fn validate_clarification_rows(
    rows: &[ClarificationRow],
    ref_known: impl Fn(&str) -> bool,
) -> Vec<StageViolation> {
    let mut out = Vec::new();
    if rows.is_empty() {
        out.push(StageViolation("clarification register has no rows".into()));
        return out;
    }
    for row in rows {
        if !ref_known(&row.gap_ref) {
            out.push(StageViolation(format!("clarification {} has an untraceable gap: {}", row.id, row.gap_ref)));
        }
        if is_generic(&row.question) {
            out.push(StageViolation(format!("clarification {} is generic noise; tie it to {}", row.id, row.gap_ref)));
        }
        if !row.blocking && row.answer.is_empty() {
            out.push(StageViolation(format!("non-blocking clarification {} still needs an answer", row.id)));
        }
    }
    out
}
