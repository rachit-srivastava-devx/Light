//! Statement-of-Work bridge and durable human-review records.
//!
//! Mechanical SOW validation deliberately remains in `crew.sow`. This module
//! only invokes that source of truth and stores the returned JSON against an
//! exact task hash.

use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const EXIT_READY_AWAITING_REVIEW: i32 = 9;
const EXIT_ENVIRONMENT: i32 = 3;
const EXIT_INVARIANT: i32 = 6;
const EXIT_MISMATCH: i32 = 8;

#[derive(Debug)]
pub enum BuildError {
    Environment(String),
    Refused(String),
    Invariant(String),
}

pub fn id_for_task(task: &str) -> String {
    blake3::hash(task.as_bytes()).to_hex().to_string()
}

pub fn invoke_crew(task: &str) -> Result<Value, BuildError> {
    let python = env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
    let crew_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("crew");
    let output = Command::new(&python)
        .args(["-m", "crew.sow", "--task", task])
        .current_dir(&crew_root)
        .env("PYTHONPATH", &crew_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| BuildError::Environment(format!("could not launch {python}: {error}")))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match output.status.code() {
        Some(0) => serde_json::from_slice::<Value>(&output.stdout).map_err(|error| {
            BuildError::Invariant(format!("crew.sow emitted invalid JSON: {error}"))
        }),
        Some(7) => Err(BuildError::Refused(stderr)),
        Some(3) => Err(BuildError::Environment(stderr)),
        Some(code) => Err(BuildError::Invariant(format!(
            "crew.sow exited {code}: {stderr}"
        ))),
        None => Err(BuildError::Invariant(
            "crew.sow terminated without an exit code".to_string(),
        )),
    }
}

pub fn record_path(state: &Path, id: &str) -> PathBuf {
    state.join("sows").join(format!("{id}.json"))
}

pub fn load(state: &Path, id: &str) -> Result<Option<Value>, i32> {
    let path = record_path(state, id);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|_| EXIT_ENVIRONMENT)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| EXIT_MISMATCH)
}

pub fn ready_record(id: &str, task: &str, sow: Value, created_at: &str) -> Value {
    json!({
        "schema_version": "1.0",
        "id": id,
        "task": task,
        "sow": sow,
        "created_at": created_at,
        "accepted": null
    })
}

pub fn write_atomic(state: &Path, id: &str, record: &Value) -> Result<(), i32> {
    let directory = state.join("sows");
    fs::create_dir_all(&directory).map_err(|_| EXIT_ENVIRONMENT)?;
    let path = record_path(state, id);
    let temporary = directory.join(format!(".{id}.tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(record).map_err(|_| EXIT_INVARIANT)?;
    fs::write(&temporary, bytes).map_err(|_| EXIT_ENVIRONMENT)?;
    fs::rename(temporary, path).map_err(|_| EXIT_ENVIRONMENT)
}

pub fn accepted_for_exact_task(record: &Value, id: &str, task: &str) -> bool {
    record.get("id").and_then(Value::as_str) == Some(id)
        && record.get("task").and_then(Value::as_str) == Some(task)
        && record.get("accepted").is_some_and(|accepted| {
            accepted.get("by").is_some_and(Value::is_string)
                && accepted.get("at").is_some_and(Value::is_string)
                && accepted.get("receipt").is_some_and(Value::is_string)
        })
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Accepted,
    NeedsAcceptance { id: String },
}

pub fn review_status(task: &str, record: Option<&Value>) -> ReviewStatus {
    let id = id_for_task(task);
    if record.is_some_and(|value| accepted_for_exact_task(value, &id, task)) {
        ReviewStatus::Accepted
    } else {
        ReviewStatus::NeedsAcceptance { id }
    }
}

// ---------------------------------------------------------------------------------------------
// F07: `lld.v1` -> crew.sow task text, and the sibling record constructor that carries
// provenance for that origin (lane contract §5-6). Everything above this line (`create_sow`,
// `ready_record`, `accepted_for_exact_task`, `invoke_crew`, ...) is UNCHANGED: this is new code
// appended alongside it, not a rewrite (lane contract §5.2 step 9, §5.3, §9).
// ---------------------------------------------------------------------------------------------

/// A gate-passed `lld.v1` wrapper failed the F07 compiler's sanitization or citation rules
/// (lane contract §6.3/§6.5). `field` is `None` only for `NoCitableEvidence`, which is not tied
/// to one interpolated slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    EmptyAfterNormalize { field: &'static str },
    MarkerCollision { field: &'static str },
    PipeCollision { field: &'static str },
    BracketCollision { field: &'static str },
    NoCitableEvidence,
}

impl CompileError {
    /// The exit-taxonomy reason token (lane contract §8), byte-identical across every refusal
    /// path so a caller (or a test) does not need to know which branch produced it.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::EmptyAfterNormalize { .. } => "EMPTY_AFTER_NORMALIZE",
            Self::MarkerCollision { .. } => "SOW_TEXT_MARKER_COLLISION",
            Self::PipeCollision { .. } => "SOW_TEXT_PIPE_COLLISION",
            Self::BracketCollision { .. } => "SOW_TEXT_BRACKET_COLLISION",
            Self::NoCitableEvidence => "NO_CITABLE_EVIDENCE",
        }
    }

    pub fn field(&self) -> Option<&'static str> {
        match self {
            Self::EmptyAfterNormalize { field }
            | Self::MarkerCollision { field }
            | Self::PipeCollision { field }
            | Self::BracketCollision { field } => Some(field),
            Self::NoCitableEvidence => None,
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.field() {
            Some(field) => write!(f, "{} ({field})", self.reason()),
            None => write!(f, "{}", self.reason()),
        }
    }
}

impl std::error::Error for CompileError {}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Rule 1 (collapse all whitespace runs -- including `\n`/`\r`/`\t` -- to a single space, trim)
/// through rule 5 (no `[`/`]`, RISK only), lane contract §6.5, applied in this exact order to
/// every interpolated string. `is_risk` gates rule 5, which only ever applies to the Challenges
/// `{RISK}` slot. Refuse loudly; never mangle (§6.5's own words) -- every branch here is a
/// refusal, never a silent rewrite of the input beyond the lossless whitespace collapse.
fn sanitize(raw: &str, field: &'static str, is_risk: bool) -> Result<String, CompileError> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err(CompileError::EmptyAfterNormalize { field });
    }
    if contains_marker(&collapsed) {
        return Err(CompileError::MarkerCollision { field });
    }
    if collapsed.contains('|') {
        return Err(CompileError::PipeCollision { field });
    }
    if is_risk && (collapsed.contains('[') || collapsed.contains(']')) {
        return Err(CompileError::BracketCollision { field });
    }
    Ok(collapsed)
}

/// `(?i)(predicate|acceptance)\s*[:=]`, hand-written -- no `regex` crate anywhere in this
/// workspace (`lld.rs:59-63`'s rationale applies identically here). Only a single literal space
/// can ever precede the colon/equals by the time this runs: `sanitize` already collapsed every
/// whitespace run to one space (rule 1 runs before rule 3), so this does not need full `\s*`
/// semantics -- `trim_start_matches(' ')` is enough and stays honest about that ordering
/// dependency rather than reimplementing `\s`.
fn contains_marker(collapsed: &str) -> bool {
    let lower = collapsed.to_lowercase();
    for keyword in ["predicate", "acceptance"] {
        let mut search_from = 0;
        while let Some(found_at) = lower[search_from..].find(keyword) {
            let after = search_from + found_at + keyword.len();
            let rest = lower[after..].trim_start_matches(' ');
            if rest.starts_with(':') || rest.starts_with('=') {
                return true;
            }
            search_from += found_at + 1;
        }
    }
    false
}

/// `^[A-Z]{1,3}\d+` as a PREFIX match, with regex backtracking semantics: prefer the longest
/// leading run of up to 3 uppercase letters that is still followed by at least one digit,
/// backtracking to fewer letters otherwise. Byte-oriented (ASCII only, matching the pattern's
/// own `[A-Z]` class).
fn leading_token_matches(s: &str) -> bool {
    let bytes = s.as_bytes();
    let max_letters = bytes
        .iter()
        .take(3)
        .take_while(|b| b.is_ascii_uppercase())
        .count();
    (1..=max_letters).any(|letters| bytes.get(letters).is_some_and(u8::is_ascii_digit))
}

/// The first `checked_check_ids` entry whose leading token looks like a corpus id, emitted
/// VERBATIM (lane contract §6.3: the emitted text carries the full id, e.g. `C1-OPEN`; only
/// crew.sow's own extractor later truncates it to `C1` when it parses the compiled text back --
/// a disclosed imprecision, not something this function papers over). Array order preserved,
/// first match wins (§6.6 determinism).
fn find_citation(checked_check_ids: &Value) -> Result<String, CompileError> {
    checked_check_ids
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|s| leading_token_matches(s))
        .map(str::to_string)
        .ok_or(CompileError::NoCitableEvidence)
}

/// `lld.v1` -> the exact `crew.sow` task-text grammar (lane contract §6). Pure and deterministic
/// (§6.6): no clock read, no env read, no random value, no absolute path in the output; every
/// object field is read BY NAME (never iterated) and only arrays (`killed_alternatives`,
/// `checked_check_ids`, `non_goals`) are walked, in their own stored order. Callers must run
/// `lld::validate_lld_v1` first (F07 lane contract §5.2 step 2) -- this function trusts the shape
/// and only refuses on the content-level rules §6.3/§6.5 can catch.
pub fn compile_task_text(wrapper: &Value) -> Result<String, CompileError> {
    let module_brief = &wrapper["module_brief"];
    let freeze = &wrapper["freeze"];
    let sow_seed = &wrapper["sow_seed"];
    let failure_story = &module_brief["failure_story"];
    let blind_suite_seed = &sow_seed["blind_suite_seed"];
    let predicate = &blind_suite_seed["predicate"];

    let restatement = sanitize(
        &format!(
            "{} Decision: {} Why: {} Purpose: {}",
            str_at(sow_seed, "restatement"),
            str_at(freeze, "decision"),
            str_at(freeze, "why"),
            str_at(module_brief, "purpose"),
        ),
        "restatement",
        false,
    )?;

    let leaf = sanitize(
        &format!(
            "Given {}, when {}, then {}",
            str_at(predicate, "given"),
            str_at(predicate, "when"),
            str_at(predicate, "then"),
        ),
        "leaf",
        false,
    )?;
    let predicate_text = sanitize(
        &format!(
            "{}:{}",
            str_at(predicate, "oracle_kind"),
            str_at(blind_suite_seed, "artifact"),
        ),
        "predicate",
        false,
    )?;

    let risk = sanitize(
        str_at(failure_story, "trigger"),
        "failure_story.trigger",
        true,
    )?;
    let citation = find_citation(&freeze["depth_evidence"]["checked_check_ids"])?;

    let alternatives = freeze["killed_alternatives"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut alternative_lines = Vec::with_capacity(alternatives.len());
    for (index, alternative) in alternatives.iter().enumerate() {
        let n = index + 1;
        let option = sanitize(
            str_at(alternative, "option"),
            "killed_alternatives[].option",
            false,
        )?;
        let why_killed = sanitize(
            str_at(alternative, "why_killed"),
            "killed_alternatives[].why_killed",
            false,
        )?;
        let revive_trigger = sanitize(
            str_at(alternative, "revive_trigger"),
            "killed_alternatives[].revive_trigger",
            false,
        )?;
        alternative_lines.push(format!(
            "- Alt{n}: {option} — {why_killed} | tradeoff: {revive_trigger}"
        ));
    }

    let fail_safe = sanitize(
        str_at(failure_story, "fail_safe"),
        "failure_story.fail_safe",
        false,
    )?;
    let non_goals = module_brief["non_goals"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut edge_case_lines = Vec::with_capacity(1 + non_goals.len());
    edge_case_lines.push(format!("- {fail_safe}"));
    for non_goal in &non_goals {
        let text = sanitize(non_goal.as_str().unwrap_or(""), "non_goals[]", false)?;
        edge_case_lines.push(format!("- {text}"));
    }

    let mut lines = vec![
        format!("Task: {restatement}"),
        "Leaves:".to_string(),
        format!("- {leaf} | acceptance: {predicate_text}"),
        "Challenges:".to_string(),
        format!("- {risk} [{citation}]"),
        "Alternatives:".to_string(),
    ];
    lines.extend(alternative_lines);
    lines.push("Estimate:".to_string());
    lines.push(
        "- not estimated at freeze time (lld.v1 carries no estimate field; F07 contract §6.4)"
            .to_string(),
    );
    lines.push("Edge cases:".to_string());
    lines.extend(edge_case_lines);
    Ok(lines.join("\n"))
}

/// `= ready_record(..) + { "lld_provenance": provenance }` (lane contract §5.3). `ready_record`
/// itself is UNCHANGED; this is a sibling used only by the `--lld` intake, so a `--task`-created
/// record's shape never carries this field (F07-T13).
pub fn ready_record_with_provenance(
    id: &str,
    task: &str,
    sow: Value,
    created_at: &str,
    provenance: Value,
) -> Value {
    let mut record = ready_record(id, task, sow, created_at);
    record["lld_provenance"] = provenance;
    record
}

#[cfg(test)]
mod f07_compiler_unit_tests {
    //! Small, colocated unit tests for the pure sanitize/citation helpers above -- the 14-test
    //! contract suite (`tests/f07_sow_intake.rs`) exercises `compile_task_text` end-to-end
    //! against real fixtures; these cover the hand-written regex-replacement logic directly,
    //! the same way `lld.rs`'s own `#[cfg(test)] mod tests` covers its hand-written matchers.
    use super::{compile_task_text, sanitize, CompileError};
    use serde_json::json;

    #[test]
    fn sanitize_collapses_all_whitespace_runs_to_one_space_and_trims() {
        assert_eq!(sanitize("  a\tb\n\nc  ", "f", false).unwrap(), "a b c");
    }

    #[test]
    fn sanitize_refuses_empty_after_collapse() {
        assert_eq!(
            sanitize("   \n\t  ", "f", false),
            Err(CompileError::EmptyAfterNormalize { field: "f" })
        );
    }

    #[test]
    fn sanitize_refuses_a_predicate_or_acceptance_marker_case_insensitively() {
        assert_eq!(
            sanitize("x Predicate: y", "f", false),
            Err(CompileError::MarkerCollision { field: "f" })
        );
        assert_eq!(
            sanitize("x acceptance = y", "f", false),
            Err(CompileError::MarkerCollision { field: "f" })
        );
        // Not a false positive on unrelated text with neither keyword.
        assert!(sanitize("x predicated on y", "f", false).is_ok());
    }

    #[test]
    fn sanitize_refuses_a_pipe_anywhere() {
        assert_eq!(
            sanitize("a | b", "f", false),
            Err(CompileError::PipeCollision { field: "f" })
        );
    }

    #[test]
    fn sanitize_refuses_brackets_only_when_marked_as_risk() {
        assert_eq!(
            sanitize("a [b]", "f", true),
            Err(CompileError::BracketCollision { field: "f" })
        );
        assert!(sanitize("a [b]", "f", false).is_ok());
    }

    #[test]
    fn compile_task_text_refuses_no_citable_evidence_when_no_id_has_a_digit() {
        let mut wrapper = complete_wrapper_fixture();
        wrapper["freeze"]["depth_evidence"]["checked_check_ids"] = json!(["IFACE", "SHAPE"]);
        assert_eq!(
            compile_task_text(&wrapper),
            Err(CompileError::NoCitableEvidence)
        );
    }

    #[test]
    fn compile_task_text_emits_the_locked_estimate_literal_and_at_least_two_alternatives() {
        let wrapper = complete_wrapper_fixture();
        let text = compile_task_text(&wrapper).expect("well-formed fixture must compile");
        assert!(text.contains(
            "- not estimated at freeze time (lld.v1 carries no estimate field; F07 contract §6.4)"
        ));
        assert_eq!(text.matches("| tradeoff:").count(), 2);
    }

    /// A minimal, self-contained wrapper -- NOT one of the lead-owned fixtures under
    /// `contracts/fixtures/lld/` (those are for the external contract suite only); built here so
    /// this file's own unit tests have no path dependency on that directory.
    fn complete_wrapper_fixture() -> serde_json::Value {
        json!({
            "schema_version": "1.0",
            "module_brief": {
                "purpose": "p",
                "non_goals": ["ng1"],
                "failure_story": {"trigger": "t", "fail_safe": "fs"}
            },
            "freeze": {
                "decision": "d",
                "why": "w",
                "killed_alternatives": [
                    {"option": "o1", "why_killed": "wk1", "revive_trigger": "r1"},
                    {"option": "o2", "why_killed": "wk2", "revive_trigger": "r2"}
                ],
                "depth_evidence": {"checked_check_ids": ["C1-OPEN"]}
            },
            "sow_seed": {
                "restatement": "r",
                "blind_suite_seed": {
                    "predicate": {"given": "g", "when": "wh", "then": "th", "oracle_kind": "test"},
                    "artifact": "a"
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        accepted_for_exact_task, id_for_task, invoke_crew, ready_record, review_status, BuildError,
        ReviewStatus,
    };
    use serde_json::json;

    #[test]
    fn task_identity_is_exact_and_stable() {
        assert_eq!(id_for_task("same"), id_for_task("same"));
        assert_ne!(id_for_task("same"), id_for_task("same "));
    }

    #[test]
    fn both_unaccepted_and_wrong_task_are_refused() {
        let id = id_for_task("task");
        let mut record = ready_record(&id, "task", json!({}), "2026-08-24T00:00:00Z");
        assert!(!accepted_for_exact_task(&record, &id, "task"));
        record["accepted"] = json!({
            "by":"owner",
            "at":"2026-08-24T00:01:00Z",
            "receipt":"blake3:acceptance"
        });
        assert!(accepted_for_exact_task(&record, &id, "task"));
        assert!(!accepted_for_exact_task(&record, &id, "other"));
    }

    #[test]
    fn vague_task_returns_line_traced_clarifying_questions_and_exit_7() {
        match invoke_crew("add a flag") {
            Err(BuildError::Refused(detail)) => {
                assert!(detail.contains("CLARIFYING QUESTIONS"));
                assert!(detail.contains("[SOW line 1]"));
            }
            other => panic!("expected crew.sow refusal, got {other:?}"),
        }
    }

    #[test]
    fn good_task_returns_complete_sow_for_exit_9() {
        let task = "Task: wire crew.sow into Fleet.\n\
Leaves:\n\
- invoke crew.sow | acceptance: requirement_present(1)\n\
Challenges:\n\
- bridge drift [crew/crew/sow.py:1]\n\
Alternatives:\n\
- Bridge: invoke Python | tradeoff: Python is required\n\
- Port: rewrite in Rust | tradeoff: validation can drift\n\
Estimate:\n\
- 1 engineer-day\n\
Edge cases:\n\
- Python is unavailable";
        let value = invoke_crew(task).expect("complete input should produce a SOW");
        assert_eq!(value["leaves"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["alternatives"].as_array().map(Vec::len), Some(2));
        assert_eq!(value["edge_cases"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["ESTIMATE"], "1 engineer-day");
    }

    #[test]
    fn run_review_gate_names_id_until_acceptance_then_proceeds() {
        let task = "exact task";
        let id = id_for_task(task);
        let mut record = ready_record(&id, task, json!({}), "2026-08-24T00:00:00Z");
        assert_eq!(
            review_status(task, Some(&record)),
            ReviewStatus::NeedsAcceptance { id: id.clone() }
        );
        record["accepted"] = json!({
            "by":"owner",
            "at":"2026-08-24T00:01:00Z",
            "receipt":"blake3:acceptance"
        });
        assert_eq!(review_status(task, Some(&record)), ReviewStatus::Accepted);
    }
}
