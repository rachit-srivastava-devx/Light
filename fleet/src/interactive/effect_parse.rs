//! Conservative parser for user-requested local and remote effects.

use super::effect_words::{action_in_followup, informational, leads_with, normalize};
use super::effect_words::{COMMAND, PUBLISH, WRITE};

#[derive(Default)]
pub struct Signals {
    pub write: bool,
    pub command: bool,
    pub publish: bool,
    pub schedule: bool,
}

pub fn classify(raw: &str) -> Signals {
    let clause = normalize(raw.trim());
    if clause.is_empty() || informational(clause) {
        return Signals::default();
    }
    let effectful = leads_with(clause, WRITE)
        || leads_with(clause, COMMAND)
        || requests_publish(clause)
        || requests_schedule(clause);
    Signals {
        write: leads_with(clause, WRITE) || effectful && action_in_followup(clause, WRITE),
        command: leads_with(clause, COMMAND) || effectful && action_in_followup(clause, COMMAND),
        publish: requests_publish(clause) || effectful && action_in_followup(clause, PUBLISH),
        schedule: requests_schedule(clause),
    }
}

fn requests_publish(clause: &str) -> bool {
    if clause.starts_with("do not ") || clause.starts_with("don't ") {
        return false;
    }
    leads_with(clause, PUBLISH)
        || [
            "create a pr",
            "open a pr",
            "create a pull request",
            "open a pull request",
        ]
        .iter()
        .any(|phrase| clause.starts_with(phrase))
}

fn requests_schedule(clause: &str) -> bool {
    leads_with(clause, &["remind", "schedule"])
        || ["after ", "at ", "every ", "in ", "later", "tomorrow"]
            .iter()
            .any(|marker| clause.starts_with(marker))
        || ((leads_with(clause, WRITE) || leads_with(clause, COMMAND) || clause.starts_with("do "))
            && [" after ", " later", " tomorrow", " every "]
                .iter()
                .any(|marker| clause.contains(marker)))
}
