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
