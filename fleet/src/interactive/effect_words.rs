//! Vocabulary helpers for deterministic effect parsing.

pub const WRITE: &[&str] = &[
    "add",
    "change",
    "delete",
    "edit",
    "fix",
    "implement",
    "modify",
    "patch",
    "refactor",
    "remove",
    "rename",
    "replace",
    "update",
    "write",
];
pub const COMMAND: &[&str] = &["build", "check", "execute", "run", "test", "verify"];
pub const PUBLISH: &[&str] = &["deploy", "merge", "publish", "push", "release"];
const INFO: &[&str] = &[
    "describe ",
    "explain ",
    "how do i ",
    "how to ",
    "is it safe to ",
    "tell me ",
    "what happens ",
    "what is ",
    "why ",
];

pub fn normalize(mut clause: &str) -> &str {
    for prefix in [
        "and ",
        "please ",
        "can you ",
        "could you ",
        "would you ",
        "i need you to ",
        "i want you to ",
    ] {
        if let Some(rest) = clause.strip_prefix(prefix) {
            clause = rest.trim_start();
            break;
        }
    }
    clause
}

pub fn informational(clause: &str) -> bool {
    INFO.iter().any(|prefix| clause.starts_with(prefix))
}

pub fn leads_with(clause: &str, actions: &[&str]) -> bool {
    clause
        .split_whitespace()
        .next()
        .is_some_and(|word| actions.contains(&word))
}

pub fn action_in_followup(clause: &str, actions: &[&str]) -> bool {
    [" and ", " then "]
        .iter()
        .filter_map(|separator| clause.split_once(separator).map(|(_, rest)| rest))
        .map(normalize)
        .any(|rest| leads_with(rest, actions))
}
