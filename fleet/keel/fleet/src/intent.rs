//! Deterministic natural-language intent routing for the interactive front door.
//!
//! This deliberately is not a model call.  The checked-in table is the routing
//! contract: an utterance either satisfies one rule or is refused with nearby
//! examples.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Intent {
    Change,
    LedgerVerify,
    MeterShow,
    Diagnose,
}

impl Intent {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Change => "implement a change",
            Self::LedgerVerify => "verify the ledger",
            Self::MeterShow => "show token quota",
            Self::Diagnose => "diagnose what broke",
        }
    }

    /// Command prefixes are part of the committed intent contract. Arguments
    /// such as the prompt, repository, and selected worker are added only
    /// after classification.
    pub(crate) fn command_prefixes(self) -> &'static [&'static [&'static str]] {
        match self {
            // D41: a plan that omits a gate the user must pass through is not a plan. `swarm
            // dispatch` REFUSES a task with no accepted SOW (reqs 6/7/14), so a plan naming only
            // the dispatch sends the user straight into a refusal. The planning steps are part of
            // the plan.
            Self::Change => &[&["sow"], &["sow", "accept"], &["swarm", "dispatch"]],
            Self::LedgerVerify => &[&["ledger", "verify"]],
            Self::MeterShow => &[&["meter", "show"]],
            Self::Diagnose => &[&["doctor"], &["ratchet", "show"]],
        }
    }

    pub(crate) fn agent_and_skills(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Change => ("builder", &["rust"]),
            Self::LedgerVerify => ("verifier", &["invariants"]),
            Self::MeterShow => ("meter", &["metrics"]),
            Self::Diagnose => ("builder", &["debugging"]),
        }
    }
}

#[derive(Clone, Copy)]
struct Rule {
    intent: Intent,
    examples: &'static [&'static str],
    matches: fn(&str) -> bool,
}

const RULES: &[Rule] = &[
    Rule {
        intent: Intent::LedgerVerify,
        examples: &[
            "is the ledger ok",
            "verify the ledger",
            "check ledger integrity",
        ],
        matches: |text| {
            contains_any(text, &["ledger"])
                && contains_any(text, &["ok", "verify", "valid", "integrity", "healthy"])
        },
    },
    Rule {
        intent: Intent::MeterShow,
        examples: &[
            "how many tokens left",
            "show token quota",
            "what is the remaining quota",
        ],
        matches: |text| {
            contains_any(text, &["token", "tokens", "quota"])
                && contains_any(
                    text,
                    &[
                        "left",
                        "remaining",
                        "remain",
                        "show",
                        "many",
                        "usage",
                        "available",
                    ],
                )
        },
    },
    Rule {
        intent: Intent::Diagnose,
        examples: &[
            "what broke",
            "diagnose fleet",
            "why is fleet broken",
            "why did the last dispatch fail",
        ],
        matches: |text| {
            contains_any(
                text,
                &[
                    "what broke",
                    "broken",
                    "diagnose",
                    "diagnostic",
                    "failing",
                    "failure",
                ],
            ) || (contains_any(text, &["why did", "why is", "why are"])
                && contains_any(text, &["fail", "error", "break", "refus", "reject"]))
        },
    },
    Rule {
        intent: Intent::Change,
        examples: &[
            "add a --version flag",
            "fix the parser bug",
            "implement the requested feature",
        ],
        matches: |text| {
            contains_any(
                text,
                &[
                    "add ",
                    "build ",
                    "change ",
                    "create ",
                    "fix ",
                    "implement ",
                    // D52: the verb list missed ordinary implementation words and rejected
                    // "handle division by zero in the calculator" -- a normal task. A committed
                    // table is the right call for auditability, but it has to cover the language
                    // people actually use. Added from real phrasings, not invented ones.
                    "delete ",
                    "disable ",
                    "enable ",
                    "extract ",
                    "guard ",
                    "handle ",
                    "improve ",
                    "introduce a ",
                    "introduce an ",
                    // "make " is deliberately ABSENT. It matched "make me a sandwich" and turned
                    // the canonical refusal case into a routed change -- N5/N6 caught it
                    // immediately. A table that must never guess should lose recall rather than
                    // gain a false positive; "make the parser stricter" is expressible as
                    // "change the parser to be stricter".
                    "migrate ",
                    "move ",
                    "optimise ",
                    "optimize ",
                    "port ",
                    "refactor ",
                    "remove ",
                    "rename ",
                    "replace ",
                    "support ",
                    "update ",
                    "validate ",
                    "wire ",
                ],
            )
        },
    },
];

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub(crate) fn classify(prompt: &str) -> Result<Intent, Vec<&'static str>> {
    let normalized = format!(" {} ", prompt.trim().to_lowercase());
    if let Some(rule) = RULES.iter().find(|rule| (rule.matches)(&normalized)) {
        return Ok(rule.intent);
    }
    let mut examples = RULES
        .iter()
        .flat_map(|rule| rule.examples.iter().copied())
        .map(|example| (edit_distance(normalized.trim(), example), example))
        .collect::<Vec<_>>();
    examples.sort_by_key(|(distance, example)| (*distance, *example));
    examples.dedup_by_key(|(_, example)| *example);
    Err(examples
        .into_iter()
        .take(3)
        .map(|(_, example)| example)
        .collect())
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.chars().enumerate() {
            current.push(
                (current[column] + 1)
                    .min(previous[column + 1] + 1)
                    .min(previous[column] + usize::from(left_char != right_char)),
            );
        }
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{classify, Intent};

    #[test]
    fn committed_examples_map_to_expected_commands() {
        let cases = [
            (
                "add a --version flag",
                Intent::Change,
                // D41: a change now plans sow -> accept -> dispatch. The dispatch alone would
                // walk the user into a refusal, because reqs 6/7/14 gate it on an accepted SOW.
                &[
                    &["sow"][..],
                    &["sow", "accept"][..],
                    &["swarm", "dispatch"][..],
                ][..],
            ),
            (
                "is the ledger ok",
                Intent::LedgerVerify,
                &[&["ledger", "verify"][..]][..],
            ),
            (
                "how many tokens left",
                Intent::MeterShow,
                &[&["meter", "show"][..]][..],
            ),
            (
                "what broke",
                Intent::Diagnose,
                &[&["doctor"][..], &["ratchet", "show"][..]][..],
            ),
            (
                "please introduce a dry-run mode for swarm dispatch",
                Intent::Change,
                &[
                    &["sow"][..],
                    &["sow", "accept"][..],
                    &["swarm", "dispatch"][..],
                ][..],
            ),
            (
                "how much quota is still available",
                Intent::MeterShow,
                &[&["meter", "show"][..]][..],
            ),
            (
                "why did the last dispatch fail",
                Intent::Diagnose,
                &[&["doctor"][..], &["ratchet", "show"][..]][..],
            ),
        ];
        for (prompt, expected, commands) in cases {
            assert_eq!(classify(prompt), Ok(expected), "prompt={prompt}");
            assert_eq!(expected.command_prefixes(), commands, "prompt={prompt}");
            let (agent, skills) = expected.agent_and_skills();
            assert!(!agent.is_empty(), "prompt={prompt}");
            assert!(!skills.is_empty(), "prompt={prompt}");
        }
        assert!(classify("make me a sandwich").is_err());
    }

    #[test]
    fn unknown_intent_refuses_instead_of_guessing() {
        let closest = classify("tell me a bedtime story").unwrap_err();
        assert_eq!(closest.len(), 3);
        assert!(closest.iter().all(|example| !example.is_empty()));
    }

    #[test]
    fn every_intent_route_selects_declared_resolved_skills() {
        let agents = fleet::agent::Registry::load_default().unwrap();
        let skills = fleet::skills::Registry::load_default().unwrap();
        for intent in [
            Intent::Change,
            Intent::LedgerVerify,
            Intent::MeterShow,
            Intent::Diagnose,
        ] {
            let (agent, required) = intent.agent_and_skills();
            skills.require_for_agent(&agents, agent, required).unwrap();
        }
    }
}
