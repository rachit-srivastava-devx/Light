//! F02: `lld.v1` mirrors for the fleet (Rust) runtime -- `module-brief.v1` / `freeze.v1` /
//! `lld.v1` (F02 lane contract §5.6). A hand-written `serde_json::Value` validator, matching
//! `validate_lane_status_projection`'s style (`main.rs:3538`: `&Map<String, Value>` in, a
//! structured result out, one check per rule with a human-readable reason). Unlike that function
//! (and unlike the TS/Python mirrors' first-error-is-enough style), this mirror collects ALL
//! violations: the cross-language comparator (`fleet/tests/acceptance/lld-crosslang.sh`) diffs
//! error-path SETS across the three mirrors, and a first-error-only validator cannot participate
//! in that comparison.
//!
//! Path-naming convention (`alternatives[0]`, `guarantees[0].derivation.calc`) is load-bearing:
//! it is mirrored byte-for-byte by `apps/mobile/src/build/lld-v1.ts` and
//! `backend/relay-py/src/orb_relay/proxy/lld_schemas.py`'s hand-written detailed validators.
//!
//! No production caller exists yet in this crate -- F06's gate and F07's intake are the callers
//! (F02 lane contract §5.4 table: fleet keel "no" caller today, "yes" mirror ships). This is a
//! disclosed, deliberate scaffold-with-tests, not an oversight: `main.rs` is not in this lane's
//! edited-file list (§5.5), so nothing here is wired into the CLI dispatch yet.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

/// One shape violation: a dotted/bracket-indexed field path plus a human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub message: String,
}

fn v(path: impl Into<String>, message: impl Into<String>) -> Violation {
    Violation {
        path: path.into(),
        message: message.into(),
    }
}

fn is_non_empty_str(x: Option<&Value>) -> bool {
    matches!(x, Some(Value::String(s)) if !s.trim().is_empty())
}

fn str_trim_len(x: Option<&Value>) -> usize {
    match x {
        Some(Value::String(s)) => s.trim().len(),
        _ => 0,
    }
}

fn as_str(x: Option<&Value>) -> Option<&str> {
    match x {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn is_string_array(x: Option<&Value>) -> bool {
    matches!(x, Some(Value::Array(items)) if items.iter().all(|i| matches!(i, Value::String(_))))
}

/// `^[a-z0-9][a-z0-9-]{2,63}$`, hand-checked -- no `regex` crate dependency anywhere in this
/// workspace, and these three patterns (node_id, freeze_id, content_hash) are simple enough that
/// adding one would be exactly the "adopt a dependency to avoid a dozen lines of hand code" this
/// contract's own killed alternatives push back on elsewhere (see `main.rs`'s `valid_hash` /
/// `valid_artifact_id` for the same style precedent).
pub(crate) fn valid_node_id(s: &str) -> bool {
    let b = s.as_bytes();
    // `{2,63}` after the head character means total length is in [1+2, 1+63] = [3, 64] --
    // NOT `b.len() >= 1`. A one-character id like "x" must be REJECTED; an early Rust draft of
    // this function only checked head_ok plus a vacuously-true `.all()` over an empty tail slice,
    // which wrongly accepted "x" -- caught by the cross-language comparator disagreeing with the
    // TS/Python mirrors (both use a real regex engine, which enforces the {2,63} minimum
    // correctly) on `one_line_freeze.json`'s `node_id: "x"`. See the F02 builder's final report.
    if b.len() < 3 || b.len() > 64 {
        return false;
    }
    let head_ok = b[0].is_ascii_lowercase() || b[0].is_ascii_digit();
    head_ok
        && b[1..]
            .iter()
            .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// `^fz-[0-9a-f]{16}$`.
fn valid_freeze_id(s: &str) -> bool {
    s.len() == 19
        && s.starts_with("fz-")
        && s[3..]
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// `^sha256:[0-9a-f]{64}$`.
fn valid_content_hash(s: &str) -> bool {
    s.len() == 71
        && s.starts_with("sha256:")
        && s[7..]
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub(crate) fn number_calc_ok(calc: &str) -> bool {
    let has_digit = calc.bytes().any(|b| b.is_ascii_digit());
    let has_operator = calc.contains(['+', '-', '*', '/', '÷', '×', '=', '≈', '%']);
    !calc.trim().is_empty() && has_digit && has_operator
}

pub(crate) fn structural_enforced_by_ok(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    if s.starts_with("type:") || s.starts_with("gate:") {
        return true;
    }
    for ext in [".ts", ".tsx", ".py", ".rs", ".json"] {
        if let Some(idx) = s.find(ext) {
            let rest = &s[idx + ext.len()..];
            if rest.is_empty() {
                return true;
            }
            if let Some(digits) = rest.strip_prefix(':') {
                if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
        }
    }
    false
}

const ALLOWED_MODULE_BRIEF_FIELDS: &[&str] = &[
    "schema_version",
    "node_id",
    "grain",
    "purpose",
    "owner",
    "owner_path",
    "interface",
    "data_owned",
    "deps",
    "registry",
    "acceptance",
    "non_goals",
    "open_questions",
    "guarantees",
    "alternatives",
    "failure_story",
];
const FORBIDDEN_MODULE_BRIEF_FIELDS: &[&str] = &[
    "content_hash",
    "freeze_id",
    "depth_evidence",
    "depth_score",
    "stamped_by",
    "state",
    "version",
];
const ALLOWED_FREEZE_FIELDS: &[&str] = &[
    "schema_version",
    "freeze_id",
    "version",
    "node_id",
    "content_hash",
    "decision",
    "why",
    "killed_alternatives",
    "accepts_when",
    "owner",
    "depth_evidence",
    "supersedes",
    "stamped_by",
];
const ALLOWED_SOW_SEED_FIELDS: &[&str] = &[
    "restatement",
    "blind_suite_seed",
    "blast_radius",
    "owner",
    "registry_verdict",
];
const ALLOWED_LLD_V1_FIELDS: &[&str] = &["schema_version", "module_brief", "freeze", "sow_seed"];

/// Hand-written mirror of `module-brief.v1.json`. Collects every violation (not just the first),
/// mirroring `validateModuleBrief` in `apps/mobile/src/build/lld-v1.ts` field-for-field.
pub fn validate_module_brief(value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(b) = value.as_object() else {
        out.push(v("", "ModuleBrief must be a JSON object"));
        return out;
    };

    for key in b.keys() {
        if !ALLOWED_MODULE_BRIEF_FIELDS.contains(&key.as_str()) {
            if FORBIDDEN_MODULE_BRIEF_FIELDS.contains(&key.as_str()) {
                out.push(v(key.as_str(), format!("\"{key}\" is gate-authored (Freeze-only) and must not appear on a ModuleBrief")));
            } else {
                out.push(v(
                    key.as_str(),
                    format!("unexpected property \"{key}\" (additionalProperties: false)"),
                ));
            }
        }
    }

    if b.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("schema_version", "schema_version must be \"1.0\""));
    }
    let node_id = as_str(b.get("node_id"));
    if !node_id.is_some_and(valid_node_id) {
        out.push(v("node_id", "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$"));
    }
    match b.get("grain").and_then(Value::as_str) {
        Some("module") | Some("leaf") => {}
        _ => out.push(v("grain", "grain must be \"module\" or \"leaf\"")),
    }
    match b.get("purpose").and_then(Value::as_str) {
        Some(p) if !p.is_empty() && p.chars().count() <= 200 => {}
        _ => out.push(v(
            "purpose",
            "purpose must be a string of 1..200 characters",
        )),
    }
    if !is_non_empty_str(b.get("owner")) {
        out.push(v("owner", "owner must be a non-empty string"));
    }
    match b.get("owner_path").and_then(Value::as_str) {
        Some(p) if !p.trim().is_empty() && !p.contains("..") => {}
        _ => out.push(v(
            "owner_path",
            "owner_path must be a non-empty path containing no \"..\"",
        )),
    }

    match b.get("interface").and_then(Value::as_array) {
        Some(items) if !items.is_empty() => {
            for (i, item) in items.iter().enumerate() {
                let ok = item.as_object().is_some_and(|o| {
                    is_non_empty_str(o.get("name")) && is_non_empty_str(o.get("signature"))
                });
                if !ok {
                    out.push(v(
                        format!("interface[{i}]"),
                        "each interface entry needs a non-empty name and signature",
                    ));
                }
            }
        }
        _ => out.push(v("interface", "interface must be a non-empty array")),
    }

    match b.get("data_owned").and_then(Value::as_array) {
        Some(items) => {
            for (i, item) in items.iter().enumerate() {
                let obj = item.as_object();
                let shape_ok = obj.is_some_and(|o| {
                    is_non_empty_str(o.get("store")) && is_non_empty_str(o.get("owned_by_node"))
                });
                if !shape_ok {
                    out.push(v(
                        format!("data_owned[{i}]"),
                        "each data_owned entry needs a non-empty store and owned_by_node",
                    ));
                } else if as_str(obj.and_then(|o| o.get("owned_by_node"))) != node_id {
                    out.push(v(
                        format!("data_owned[{i}].owned_by_node"),
                        "owned_by_node must equal the brief's own node_id",
                    ));
                }
            }
        }
        None => out.push(v("data_owned", "data_owned must be an array")),
    }

    if !is_string_array(b.get("deps")) {
        out.push(v("deps", "deps must be an array of strings"));
    }

    check_registry_verdict(b.get("registry"), "registry", &mut out);

    match b.get("acceptance").and_then(Value::as_object) {
        Some(a) => {
            if str_trim_len(a.get("given")) < 3 {
                out.push(v(
                    "acceptance.given",
                    "given must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(a.get("when")) < 3 {
                out.push(v(
                    "acceptance.when",
                    "when must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(a.get("then")) < 3 {
                out.push(v(
                    "acceptance.then",
                    "then must be >=3 non-blank characters",
                ));
            }
            match a.get("oracle_kind").and_then(Value::as_str) {
                Some("test") | Some("property") | Some("metric") => {}
                _ => out.push(v(
                    "acceptance.oracle_kind",
                    "oracle_kind must be one of test | property | metric",
                )),
            }
            if !is_non_empty_str(a.get("artifact")) {
                out.push(v(
                    "acceptance.artifact",
                    "artifact must be a non-empty string",
                ));
            }
        }
        None => out.push(v(
            "acceptance",
            "acceptance must be an AcceptanceLine object",
        )),
    }

    if !is_string_array(b.get("non_goals")) {
        out.push(v("non_goals", "non_goals must be an array of strings"));
    }
    if !is_string_array(b.get("open_questions")) {
        out.push(v(
            "open_questions",
            "open_questions must be an array of strings",
        ));
    }

    match b.get("guarantees").and_then(Value::as_array) {
        Some(items) if !items.is_empty() => {
            for (i, g) in items.iter().enumerate() {
                let obj = g.as_object();
                if !obj.is_some_and(|o| is_non_empty_str(o.get("claim"))) {
                    out.push(v(
                        format!("guarantees[{i}].claim"),
                        "claim must be a non-empty string",
                    ));
                }
                let label_ok = matches!(
                    obj.and_then(|o| o.get("label")).and_then(Value::as_str),
                    Some("kills_structural") | Some("kills_mechanical") | Some("mitigates")
                );
                if !label_ok {
                    out.push(v(
                        format!("guarantees[{i}].label"),
                        "label must be one of kills_structural | kills_mechanical | mitigates",
                    ));
                }
                let derivation = obj
                    .and_then(|o| o.get("derivation"))
                    .and_then(Value::as_object);
                match derivation {
                    None => out.push(v(
                        format!("guarantees[{i}].derivation"),
                        "derivation must be a Derivation object",
                    )),
                    Some(d) => match d.get("kind").and_then(Value::as_str) {
                        Some("number") => {
                            let calc_ok = as_str(d.get("calc")).is_some_and(number_calc_ok);
                            if !calc_ok {
                                out.push(v(format!("guarantees[{i}].derivation.calc"), "a numeric derivation needs non-empty arithmetic (a digit and an operator)"));
                            }
                        }
                        Some("structural") => {
                            let enforced_ok =
                                as_str(d.get("enforced_by")).is_some_and(structural_enforced_by_ok);
                            if !enforced_ok {
                                out.push(v(
                                    format!("guarantees[{i}].derivation.enforced_by"),
                                    "a structural derivation must name a file, type:, or gate:",
                                ));
                            }
                        }
                        _ => out.push(v(
                            format!("guarantees[{i}].derivation.kind"),
                            "kind must be \"number\" or \"structural\"",
                        )),
                    },
                }
            }
        }
        _ => out.push(v("guarantees", "guarantees must be a non-empty array")),
    }

    match b.get("alternatives").and_then(Value::as_array) {
        Some(items) if items.len() >= 2 => {
            for (i, alt) in items.iter().enumerate() {
                let ok = alt.as_object().is_some_and(|o| {
                    is_non_empty_str(o.get("option")) && is_non_empty_str(o.get("why_killed")) && is_non_empty_str(o.get("revive_trigger"))
                });
                if !ok {
                    out.push(v(format!("alternatives[{i}]"), "each alternative needs a non-empty option, why_killed, and revive_trigger"));
                }
            }
        }
        _ => out.push(v("alternatives", "alternatives must have at least 2 entries (flat floor -- no grain exemption, F02 §3.4)")),
    }

    match b.get("failure_story").and_then(Value::as_object) {
        Some(f) => {
            if str_trim_len(f.get("trigger")) < 10 {
                out.push(v(
                    "failure_story.trigger",
                    "trigger must be >=10 non-blank characters",
                ));
            }
            if str_trim_len(f.get("blast_radius")) < 10 {
                out.push(v(
                    "failure_story.blast_radius",
                    "blast_radius must be >=10 non-blank characters",
                ));
            }
            if str_trim_len(f.get("fail_safe")) < 10 {
                out.push(v(
                    "failure_story.fail_safe",
                    "fail_safe must be >=10 non-blank characters",
                ));
            }
        }
        None => out.push(v(
            "failure_story",
            "failure_story must be a FailureStory object",
        )),
    }

    out
}

/// Shared by `module_brief.registry` and `sow_seed.registry_verdict` (identical shape).
fn check_registry_verdict(value: Option<&Value>, path_prefix: &str, out: &mut Vec<Violation>) {
    let Some(r) = value.and_then(Value::as_object) else {
        out.push(v(path_prefix, "must be a RegistryVerdict object"));
        return;
    };
    match r.get("kind").and_then(Value::as_str) {
        Some("install") | Some("extract") => {
            if !is_non_empty_str(r.get("matched_path")) {
                out.push(v(
                    format!("{path_prefix}.matched_path"),
                    "matched_path must be a non-empty string",
                ));
            }
        }
        Some("build_new") => {
            let ok = matches!(r.get("searched"), Some(Value::Array(items)) if !items.is_empty() && items.iter().all(|s| matches!(s, Value::String(x) if !x.trim().is_empty())));
            if !ok {
                out.push(v(
                    format!("{path_prefix}.searched"),
                    "searched must be a non-empty array of non-empty strings",
                ));
            }
        }
        _ => out.push(v(
            format!("{path_prefix}.kind"),
            "kind must be one of install | extract | build_new",
        )),
    }
}

/// Shared by `freeze.accepts_when` and `sow_seed.blind_suite_seed` (identical shape).
fn check_accepts_when(value: Option<&Value>, path_prefix: &str, out: &mut Vec<Violation>) {
    let Some(o) = value.and_then(Value::as_object) else {
        out.push(v(path_prefix, "must be an AcceptsWhen object"));
        return;
    };
    for key in o.keys() {
        if key != "predicate" && key != "artifact" {
            out.push(v(
                format!("{path_prefix}.{key}"),
                format!("unexpected property \"{key}\" (additionalProperties: false)"),
            ));
        }
    }
    match o.get("predicate").and_then(Value::as_object) {
        Some(p) => {
            for key in p.keys() {
                if !["given", "when", "then", "oracle_kind"].contains(&key.as_str()) {
                    out.push(v(
                        format!("{path_prefix}.predicate.{key}"),
                        format!("unexpected property \"{key}\" (additionalProperties: false)"),
                    ));
                }
            }
            if str_trim_len(p.get("given")) < 3 {
                out.push(v(
                    format!("{path_prefix}.predicate.given"),
                    "given must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(p.get("when")) < 3 {
                out.push(v(
                    format!("{path_prefix}.predicate.when"),
                    "when must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(p.get("then")) < 3 {
                out.push(v(
                    format!("{path_prefix}.predicate.then"),
                    "then must be >=3 non-blank characters",
                ));
            }
            match p.get("oracle_kind").and_then(Value::as_str) {
                Some("test") | Some("property") | Some("metric") => {}
                _ => out.push(v(
                    format!("{path_prefix}.predicate.oracle_kind"),
                    "oracle_kind must be one of test | property | metric",
                )),
            }
        }
        None => out.push(v(
            format!("{path_prefix}.predicate"),
            "predicate must be an object",
        )),
    }
    if !is_non_empty_str(o.get("artifact")) {
        out.push(v(
            format!("{path_prefix}.artifact"),
            "artifact must be a non-empty string",
        ));
    }
}

fn check_freeze(value: Option<&Value>, out: &mut Vec<Violation>) {
    let Some(f) = value.and_then(Value::as_object) else {
        out.push(v("freeze", "freeze must be a Freeze object"));
        return;
    };
    for key in f.keys() {
        if !ALLOWED_FREEZE_FIELDS.contains(&key.as_str()) {
            out.push(v(
                format!("freeze.{key}"),
                format!("unexpected property \"{key}\" (additionalProperties: false)"),
            ));
        }
    }
    if f.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("freeze.schema_version", "schema_version must be \"1.0\""));
    }
    if !as_str(f.get("freeze_id")).is_some_and(valid_freeze_id) {
        out.push(v(
            "freeze.freeze_id",
            "freeze_id must match ^fz-[0-9a-f]{16}$",
        ));
    }
    let version_ok =
        matches!(f.get("version"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 1));
    if !version_ok {
        out.push(v("freeze.version", "version must be an integer >= 1"));
    }
    if !as_str(f.get("node_id")).is_some_and(valid_node_id) {
        out.push(v(
            "freeze.node_id",
            "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$",
        ));
    }
    if !as_str(f.get("content_hash")).is_some_and(valid_content_hash) {
        out.push(v(
            "freeze.content_hash",
            "content_hash must match ^sha256:[0-9a-f]{64}$",
        ));
    }
    if !is_non_empty_str(f.get("decision")) {
        out.push(v("freeze.decision", "decision must be a non-empty string"));
    }
    if !is_non_empty_str(f.get("why")) {
        out.push(v("freeze.why", "why must be a non-empty string"));
    }
    match f.get("killed_alternatives").and_then(Value::as_array) {
        Some(items) if items.len() >= 2 => {
            for (i, alt) in items.iter().enumerate() {
                let ok = alt.as_object().is_some_and(|o| {
                    is_non_empty_str(o.get("option"))
                        && is_non_empty_str(o.get("why_killed"))
                        && is_non_empty_str(o.get("revive_trigger"))
                });
                if !ok {
                    out.push(v(format!("freeze.killed_alternatives[{i}]"), "each killed_alternatives entry needs a non-empty option, why_killed, and revive_trigger"));
                }
            }
        }
        _ => out.push(v(
            "freeze.killed_alternatives",
            "killed_alternatives must have at least 2 entries",
        )),
    }
    check_accepts_when(f.get("accepts_when"), "freeze.accepts_when", out);
    if !is_non_empty_str(f.get("owner")) {
        out.push(v("freeze.owner", "owner must be a non-empty string"));
    }
    match f.get("depth_evidence").and_then(Value::as_object) {
        Some(de) => {
            for key in de.keys() {
                if key != "score" && key != "checked_check_ids" {
                    out.push(v(
                        format!("freeze.depth_evidence.{key}"),
                        format!("unexpected property \"{key}\" (additionalProperties: false)"),
                    ));
                }
            }
            match de.get("score").and_then(Value::as_object) {
                Some(s) => {
                    let checks_total_ok = matches!(s.get("checks_total"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
                    if !checks_total_ok {
                        out.push(v(
                            "freeze.depth_evidence.score.checks_total",
                            "checks_total must be an integer >= 0",
                        ));
                    }
                    let checks_passed_ok = matches!(s.get("checks_passed"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
                    if !checks_passed_ok {
                        out.push(v(
                            "freeze.depth_evidence.score.checks_passed",
                            "checks_passed must be an integer >= 0",
                        ));
                    }
                    let ratio_ok = matches!(s.get("ratio"), Some(Value::Number(n)) if n.as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)));
                    if !ratio_ok {
                        out.push(v(
                            "freeze.depth_evidence.score.ratio",
                            "ratio must be a number in [0,1]",
                        ));
                    }
                    if !is_string_array(s.get("failed_check_ids")) {
                        out.push(v(
                            "freeze.depth_evidence.score.failed_check_ids",
                            "failed_check_ids must be an array of strings",
                        ));
                    }
                }
                None => out.push(v(
                    "freeze.depth_evidence.score",
                    "score must be a DepthScore object",
                )),
            }
            if !is_string_array(de.get("checked_check_ids")) {
                out.push(v(
                    "freeze.depth_evidence.checked_check_ids",
                    "checked_check_ids must be an array of strings",
                ));
            }
        }
        None => out.push(v(
            "freeze.depth_evidence",
            "depth_evidence must be a DepthEvidence object",
        )),
    }
    match f.get("supersedes") {
        Some(Value::Null) => {}
        Some(Value::String(s)) if valid_freeze_id(s) => {}
        _ => out.push(v(
            "freeze.supersedes",
            "supersedes must be null or match ^fz-[0-9a-f]{16}$",
        )),
    }
    // The one runtime comparison against the gate's const (F02 §4.1). Assert the retirement, not
    // just the replacement: nothing in this file WRITES this string except this exact line and
    // the one place a Violation's message would otherwise repeat it (deliberately avoided below,
    // see f02_t14_no_source_path_constructs_the_stamper_literal in
    // tests/f02_lld_crosslang.rs, which greps this file for both facts).
    let stamped_by = as_str(f.get("stamped_by")).unwrap_or_default();
    if stamped_by == "keel:lld-ready" {
        // ok -- the only const the gate may satisfy.
    } else {
        out.push(v(
            "freeze.stamped_by",
            "stamped_by must equal the gate's const (only the gate may write it)",
        ));
    }
}

fn check_sow_seed(value: Option<&Value>, out: &mut Vec<Violation>) {
    let Some(s) = value.and_then(Value::as_object) else {
        out.push(v("sow_seed", "sow_seed must be a SowSeed object"));
        return;
    };
    for key in s.keys() {
        if !ALLOWED_SOW_SEED_FIELDS.contains(&key.as_str()) {
            out.push(v(
                format!("sow_seed.{key}"),
                format!("unexpected property \"{key}\" (additionalProperties: false)"),
            ));
        }
    }
    if !is_non_empty_str(s.get("restatement")) {
        out.push(v(
            "sow_seed.restatement",
            "restatement must be a non-empty string",
        ));
    }
    check_accepts_when(s.get("blind_suite_seed"), "sow_seed.blind_suite_seed", out);
    if !is_non_empty_str(s.get("blast_radius")) {
        out.push(v(
            "sow_seed.blast_radius",
            "blast_radius must be a non-empty string",
        ));
    }
    if !is_non_empty_str(s.get("owner")) {
        out.push(v("sow_seed.owner", "owner must be a non-empty string"));
    }
    check_registry_verdict(s.get("registry_verdict"), "sow_seed.registry_verdict", out);
}

/// Hand-written mirror of `lld.v1.json`, the wrapper that is the ONLY thing crossing the
/// orb->fleet seam (blueprint `03` §6; F02 §5.3). Delegates to `validate_module_brief` for
/// `module_brief` (paths re-prefixed `module_brief.`), and hand-written `freeze`/`sow_seed` checks
/// matching the same path-naming convention the TS and Python mirrors use.
pub fn validate_lld_v1(value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(top) = value.as_object() else {
        out.push(v("", "lld.v1 must be a JSON object"));
        return out;
    };

    for key in top.keys() {
        if !ALLOWED_LLD_V1_FIELDS.contains(&key.as_str()) {
            out.push(v(
                key.as_str(),
                format!("unexpected property \"{key}\" (additionalProperties: false)"),
            ));
        }
    }
    if top.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("schema_version", "schema_version must be \"1.0\""));
    }

    let module_brief = top.get("module_brief").cloned().unwrap_or(Value::Null);
    for e in validate_module_brief(&module_brief) {
        let path = if e.path.is_empty() {
            "module_brief".to_string()
        } else {
            format!("module_brief.{}", e.path)
        };
        out.push(v(path, e.message));
    }

    check_freeze(top.get("freeze"), &mut out);
    check_sow_seed(top.get("sow_seed"), &mut out);

    if let (Some(brief_obj), Some(freeze_obj)) = (
        top.get("module_brief").and_then(Value::as_object),
        top.get("freeze").and_then(Value::as_object),
    ) {
        if let (Some(bn), Some(fn_)) = (
            as_str(brief_obj.get("node_id")),
            as_str(freeze_obj.get("node_id")),
        ) {
            if bn != fn_ {
                out.push(v(
                    "freeze.node_id",
                    "freeze.node_id must equal the sibling module_brief.node_id",
                ));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------------------------
// Canonical JSON + content hash (§6.2: "content_hash = sha256(canonical_json(module_brief)), the
// brief and nothing else -- ever").
// ---------------------------------------------------------------------------------------------

/// A JSON number was encountered while canonicalizing -- refused structurally (F02 §6.3):
/// `serde_json::to_string(&1.0f64)` renders `"1.0"`, agreeing with Python and disagreeing with
/// JS's `JSON.stringify(1.0) === "1"`. Refusing every number makes the three-language divergence
/// unrepresentable rather than merely undocumented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericLeafError;

impl std::fmt::Display for NumericLeafError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "numbers are not canonicalizable (F02 §6.3)")
    }
}

impl std::error::Error for NumericLeafError {}

/// Deterministic JSON serialisation: object keys sorted recursively, array order preserved.
/// Mirrors `canonicalJson` (TS) / `canonical_json` (Python) byte-for-byte. String/bool/null
/// scalars are delegated to `serde_json::to_string`, a mature, spec-conformant JSON serializer,
/// rather than hand-rolling escaping rules a third time.
pub fn canonical_json(value: &Value) -> Result<String, NumericLeafError> {
    match value {
        Value::Number(_) => Err(NumericLeafError),
        Value::Array(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                parts.push(canonical_json(item)?);
            }
            Ok(format!("[{}]", parts.join(",")))
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut parts = Vec::with_capacity(keys.len());
            for k in keys {
                let key_json = serde_json::to_string(k).expect("string keys always serialize");
                parts.push(format!("{key_json}:{}", canonical_json(&map[k])?));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        // String, Bool, Null.
        other => Ok(serde_json::to_string(other).expect("non-numeric scalars always serialize")),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hash_canonical_string(canonical: &str) -> String {
    format!(
        "sha256:{}",
        hex_encode(&Sha256::digest(canonical.as_bytes()))
    )
}

/// `"sha256:" + hex(sha256(canonical_json(value)))` (F02 §4.4). Uses the `sha2` crate (RustCrypto,
/// MIT/Apache-2.0, already on `deny.toml`'s allow-list) rather than hand-rolling SHA-256 the way
/// the TS mirror does under duress (Metro/Hermes has no `node:crypto`; Rust is under no such
/// constraint -- F02 lane contract §3.5).
pub fn content_hash(value: &Value) -> Result<String, NumericLeafError> {
    Ok(hash_canonical_string(&canonical_json(value)?))
}

// ---------------------------------------------------------------------------------------------
// Cross-language report (F02 §7.4) -- consumed by fleet/tests/acceptance/lld-crosslang.sh, which
// diffs this mirror's report against the TS and Python mirrors' reports for the same corpus.
// ---------------------------------------------------------------------------------------------

/// The one literal string every mirror's report uses for a numeric-leaf refusal (F02 §6.3). Fixed
/// and language-agnostic so the three reports are byte-identical for this field regardless of
/// which specific numeric leaf a given canonicalizer meets first.
const NUMERIC_REFUSAL_MESSAGE: &str = "numbers are not canonicalizable (F02 §6.3)";

/// Builds this mirror's cross-language report over every `*.json` fixture in `fixtures_dir`.
/// `serde_json::Map` is a `BTreeMap` by default (this crate does not enable the `preserve_order`
/// feature), so every object built here serializes with keys already sorted -- the same mechanism
/// the TS/Python mirrors get by inserting keys in sorted order by hand. This is what makes
/// `serde_json::to_string_pretty` byte-identical to `JSON.stringify(x, null, 2)` /
/// `json.dumps(x, indent=2, ensure_ascii=False)` for this report shape.
pub fn cross_lang_report(fixtures_dir: &Path) -> Value {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir)
        .expect("fixtures dir must exist")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json") {
                p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    names.sort();

    let mut fixtures = Map::new();
    for name in &names {
        let path = fixtures_dir.join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path).expect("read fixture");
        // Named `parsed`, not `raw` -- a local literally named `raw` immediately followed by a
        // reference-of (`&raw`) trips a tree-sitter-rust grammar ambiguity with the (unrelated)
        // `&raw const` / `&raw mut` raw-reference-operator syntax: `graph.rs`'s
        // `every_shipped_module_is_reachable_from_dispatch` parses every file under `src/` with
        // tree-sitter and fails the whole module on any parse error, so this is a real
        // cross-tool constraint, not just a style preference. See the F02 builder's final report.
        let parsed: Value = serde_json::from_str(&text).expect("fixture must be valid JSON");

        let is_wrapper = parsed
            .as_object()
            .is_some_and(|o| o.contains_key("module_brief"));
        let violations = if is_wrapper {
            validate_lld_v1(&parsed)
        } else {
            validate_module_brief(&parsed)
        };
        let valid = violations.is_empty();
        let mut error_paths: Vec<String> = violations.into_iter().map(|viol| viol.path).collect();
        error_paths.sort();
        error_paths.dedup();

        let brief_payload: &Value = if is_wrapper {
            parsed.get("module_brief").unwrap_or(&Value::Null)
        } else {
            &parsed
        };

        let (canonical_json_val, content_hash_val, hash_refusal_val) =
            match canonical_json(brief_payload) {
                Ok(cj) => {
                    let hash = hash_canonical_string(&cj);
                    (Value::String(cj), Value::String(hash), Value::Null)
                }
                Err(_) => (
                    Value::Null,
                    Value::Null,
                    Value::String(NUMERIC_REFUSAL_MESSAGE.to_string()),
                ),
            };

        let mut entry = Map::new();
        entry.insert("canonical_json".to_string(), canonical_json_val);
        entry.insert("content_hash".to_string(), content_hash_val);
        entry.insert(
            "error_paths".to_string(),
            Value::Array(error_paths.into_iter().map(Value::String).collect()),
        );
        entry.insert("hash_refusal".to_string(), hash_refusal_val);
        entry.insert("valid".to_string(), Value::Bool(valid));
        fixtures.insert(name.clone(), Value::Object(entry));
    }

    let mut report = Map::new();
    report.insert("contract".to_string(), Value::String("lld.v1".to_string()));
    report.insert("fixtures".to_string(), Value::Object(fixtures));
    report.insert("mirror".to_string(), Value::String("rs".to_string()));
    Value::Object(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_module_brief_accepts_a_minimal_well_formed_brief() {
        let brief = json!({
            "schema_version": "1.0",
            "node_id": "x-y",
            "grain": "module",
            "purpose": "p",
            "owner": "me",
            "owner_path": "a/",
            "interface": [{"name": "f", "signature": "() => void"}],
            "data_owned": [],
            "deps": [],
            "registry": {"kind": "build_new", "searched": ["a"]},
            "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
            "non_goals": [],
            "open_questions": [],
            "guarantees": [{"claim": "c", "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
            "alternatives": [
                {"option": "a", "why_killed": "w", "revive_trigger": "r"},
                {"option": "b", "why_killed": "w", "revive_trigger": "r"}
            ],
            "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
        });
        let violations = validate_module_brief(&brief);
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn validate_module_brief_rejects_an_extra_top_level_field() {
        let mut brief = json!({
            "schema_version": "1.0",
            "node_id": "x-y",
            "grain": "module",
            "purpose": "p",
            "owner": "me",
            "owner_path": "a/",
            "interface": [{"name": "f", "signature": "() => void"}],
            "data_owned": [],
            "deps": [],
            "registry": {"kind": "build_new", "searched": ["a"]},
            "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
            "non_goals": [],
            "open_questions": [],
            "guarantees": [{"claim": "c", "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
            "alternatives": [
                {"option": "a", "why_killed": "w", "revive_trigger": "r"},
                {"option": "b", "why_killed": "w", "revive_trigger": "r"}
            ],
            "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
        });
        brief
            .as_object_mut()
            .unwrap()
            .insert("depth_evidence".to_string(), json!({"score": {}}));
        let violations = validate_module_brief(&brief);
        assert!(
            violations.iter().any(|viol| viol.path == "depth_evidence"),
            "{violations:?}"
        );
    }

    #[test]
    fn canonical_json_refuses_a_numeric_leaf_at_any_depth() {
        assert!(canonical_json(&json!({"ratio": 1.0})).is_err());
        assert!(canonical_json(&json!({"a": {"b": [1, 2, 3]}})).is_err());
        assert!(canonical_json(&json!({"a": "b", "c": true, "d": null})).is_ok());
    }

    #[test]
    fn canonical_json_sorts_keys_and_is_order_invariant() {
        let a = canonical_json(&json!({"b": "2", "a": "1"})).unwrap();
        let b = canonical_json(&json!({"a": "1", "b": "2"})).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, r#"{"a":"1","b":"2"}"#);
    }

    #[test]
    fn content_hash_matches_the_expected_pattern() {
        let hash = content_hash(&json!({"a": "1"})).unwrap();
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), "sha256:".len() + 64);
        assert!(hash["sha256:".len()..]
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn freeze_stamped_by_only_the_gate_const_is_accepted() {
        let mut freeze_obj = serde_json::Map::new();
        freeze_obj.insert("stamped_by".to_string(), json!("orb:lld-ready"));
        let mut violations = Vec::new();
        check_freeze(Some(&Value::Object(freeze_obj)), &mut violations);
        assert!(
            violations
                .iter()
                .any(|viol| viol.path == "freeze.stamped_by"),
            "{violations:?}"
        );
    }
}
