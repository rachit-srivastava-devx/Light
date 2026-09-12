//! `derive_questions`/`open_questions` -- byte-faithful port of `write_questions`/`open_count`/
//! `emit_questions` (`intake.sh:91-148`), split from IO: everything here is a pure fn over
//! already-in-memory strings/sets, never a file read or write.

use super::pattern::first_match;
use super::question_id::QuestionId;
use super::rubric_defs::{CORE, DERIVED};
use std::collections::BTreeSet;

/// Why a question is on the rubric.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    Core,
    Token(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeQuestion {
    pub id: QuestionId,
    pub dimension: &'static str,
    pub question: &'static str,
    pub trigger: Trigger,
}

/// Derives the rubric for one intent string. Byte-faithful port of `write_questions`
/// (`intake.sh:91-126`): the 2 core rows always present, the 13 derived rows each gated on a
/// fixed ERE matching the lower-cased intent (`intake.sh:111-123`).
pub fn derive_questions(intent: &str) -> Vec<IntakeQuestion> {
    let intent_lc = intent.to_lowercase();
    let mut out: Vec<IntakeQuestion> = CORE
        .iter()
        .map(|&(id, dimension, question)| IntakeQuestion {
            id,
            dimension,
            question,
            trigger: Trigger::Core,
        })
        .collect();
    for d in DERIVED {
        if let Some(token) = first_match(d.matchers, &intent_lc) {
            out.push(IntakeQuestion {
                id: d.id,
                dimension: d.dimension,
                question: d.question,
                trigger: Trigger::Token(token),
            });
        }
    }
    out
}

/// The subset of `all` not present in `answered`. Mirrors `open_count`/`emit_questions`'s filter
/// (`intake.sh:127-148`), split from I/O: `answered` is the caller's already-read
/// `answers/*.txt` presence set, not a live directory scan.
pub fn open_questions<'a>(
    all: &'a [IntakeQuestion],
    answered: &BTreeSet<QuestionId>,
) -> Vec<&'a IntakeQuestion> {
    all.iter().filter(|q| !answered.contains(&q.id)).collect()
}
