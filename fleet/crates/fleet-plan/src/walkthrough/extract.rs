//! Reads the fields `build_walkthrough` needs out of an already-`validate_module_brief`-shaped
//! `serde_json::Value` -- no re-validation here, just extraction of what the narration uses.

use serde_json::Value;

pub(crate) struct GuaranteeSummary {
    pub claim: String,
    pub label: String,
}

pub(crate) struct AcceptanceLine {
    pub given: String,
    pub when: String,
    pub then: String,
    pub oracle_kind: String,
}

pub(crate) struct ModuleSummary {
    pub node_id: String,
    pub purpose: String,
    pub deps: Vec<String>,
    pub open_questions: Vec<String>,
    pub guarantees: Vec<GuaranteeSummary>,
    pub acceptance: Option<AcceptanceLine>,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn str_array_field(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn guarantees(v: &Value) -> Vec<GuaranteeSummary> {
    v.get("guarantees")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|g| Some(GuaranteeSummary { claim: str_field(g, "claim")?, label: str_field(g, "label")? }))
                .collect()
        })
        .unwrap_or_default()
}

fn acceptance(v: &Value) -> Option<AcceptanceLine> {
    let a = v.get("acceptance")?;
    Some(AcceptanceLine {
        given: str_field(a, "given")?,
        when: str_field(a, "when")?,
        then: str_field(a, "then")?,
        oracle_kind: str_field(a, "oracle_kind")?,
    })
}

/// `None` means a required field (node_id/purpose) was unreadable -- the caller turns that into
/// `WalkthroughError::IncompleteModuleBrief`, which shape validation should already have caught.
pub(crate) fn extract_module_summary(v: &Value) -> Option<ModuleSummary> {
    Some(ModuleSummary {
        node_id: str_field(v, "node_id")?,
        purpose: str_field(v, "purpose")?,
        deps: str_array_field(v, "deps"),
        open_questions: str_array_field(v, "open_questions"),
        guarantees: guarantees(v),
        acceptance: acceptance(v),
    })
}
