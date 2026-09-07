//! F06: the `lld-ready` gate. Deterministic, pure, 0 LLM, 0 I/O (blueprint `03` §2.1).
//! Second mirror of `orb/apps/mobile/src/build/LldReadyGate.ts`; kept honest by
//! `fleet/tests/acceptance/lld-ready-crosslang.sh`, not by these functions' own unit tests alone
//! (F06 lane contract §2.3 -- the comparator is what found F02's real bugs, not the unit tests).
//!
//! This module has ZERO file I/O, ZERO clock reads, ZERO non-deterministic inputs of any kind
//! (F06-T5 greps this exact file for that). The CLI (`main.rs`) reads `owners.v1.json` /
//! `gate-refs.v1.json` and the candidate brief off disk, then calls into this module with
//! already-parsed data (§3.5 -- a gate that reads its own reference data has a verdict that
//! depends on the working directory).
//!
//! `checked == 0 => MeasuredNothing` (§4.2) is this lane's headline fix: the TS gate's identical
//! branch (`LldReadyGate.ts:219-222`) is unreachable dead code because its only caller always
//! passes the full 14-check array. `evaluate_with` is `pub` specifically so an empty slice is
//! reachable from a test (F06-T6) -- a branch no test can reach is indistinguishable from a
//! branch that does not exist.

use crate::lld;
use serde_json::Value;

/// The 14 check ids, byte-identical to and in the same order as `LldReadyGate.ts:19-34`.
/// `checks_total` is always derived from this array's length, never a literal (the estate's
/// `gate-census-not-sample` scar: a hard-coded denominator drifts silently below real coverage).
pub const GATE_CHECK_IDS: [&str; 14] = [
    "C1-OPEN",
    "C2-OWNER",
    "C3-ACC-PARSE",
    "C3-ACC-GROUND",
    "C3-ACC-NONTAUT",
    "R17-DERIV",
    "R19-ABSOLUTE",
    "R21-ALTS",
    "R21-FAIL",
    "C12-STORE",
    "C12-DEPS",
    "REG-VERDICT",
    "IFACE",
    "SHAPE",
];

/// Caller-supplied reference sets. NOT read from disk here (§3.5) -- the CLI reads
/// `owners.v1.json` + `gate-refs.v1.json` and constructs this.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateRefs {
    pub owners: Vec<String>,
    pub registry_paths: Vec<String>,
    pub known_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReason {
    pub check_id: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthScore {
    pub checks_total: u32,
    pub checks_passed: u32,
    /// `round(passed/total, 3)` -- matches TS's `Math.round(x*1000)/1000` (§11.1 item 3). Report
    /// data, never a decision input (§1.1/§3.4 -- see `evaluate_with`'s outcome logic below).
    pub ratio: f64,
    pub failed_check_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ready,
    NotReady,
    MeasuredNothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub checked: usize,
    /// `None` **iff** `outcome == MeasuredNothing` -- a verdict cannot report a score without a
    /// denominator (§4.2).
    pub score: Option<DepthScore>,
    pub reasons: Vec<GateReason>,
}

pub type Check = (&'static str, fn(&Value, &GateRefs) -> bool);

pub const CHECKS: &[Check; 14] = &[
    ("C1-OPEN", check_c1_open),
    ("C2-OWNER", check_c2_owner),
    ("C3-ACC-PARSE", check_c3_acc_parse),
    ("C3-ACC-GROUND", check_c3_acc_ground),
    ("C3-ACC-NONTAUT", check_c3_acc_nontaut),
    ("R17-DERIV", check_r17_deriv),
    ("R19-ABSOLUTE", check_r19_absolute),
    ("R21-ALTS", check_r21_alts),
    ("R21-FAIL", check_r21_fail),
    ("C12-STORE", check_c12_store),
    ("C12-DEPS", check_c12_deps),
    ("REG-VERDICT", check_reg_verdict),
    ("IFACE", check_iface),
    ("SHAPE", check_shape),
];

/// The gate's identity. THE ONLY place in this crate that constructs this literal for writing
/// (`lld.rs` only *compares* it, at `check_freeze`). F06-T9 greps this file for that invariant --
/// this doc comment intentionally never repeats the literal itself, so the grep stays honest.
pub const STAMPED_BY: &str = "keel:lld-ready";

/// The public entry point. Always measures the full vocabulary.
pub fn evaluate(brief: &Value, refs: &GateRefs) -> Verdict {
    evaluate_with(brief, refs, CHECKS)
}

/// The same evaluator over an explicit check slice. `pub` so an empty slice is reachable from a
/// test (§4.2) -- passing one is safe by construction: it yields a refusal, never a pass.
pub fn evaluate_with(brief: &Value, refs: &GateRefs, checks: &[Check]) -> Verdict {
    let checked = checks.len();
    if checked == 0 {
        return Verdict {
            outcome: Outcome::MeasuredNothing,
            checked: 0,
            score: None,
            reasons: Vec::new(),
        };
    }

    let mut reasons = Vec::new();
    for &(id, run) in checks {
        if !run(brief, refs) {
            reasons.push(GateReason {
                check_id: id,
                detail: format!("{id} failed"),
            });
        }
    }

    let checks_total = checked as u32;
    let checks_passed = checks_total - reasons.len() as u32;
    let ratio = round3(f64::from(checks_passed) / f64::from(checks_total));
    let failed_check_ids: Vec<String> = reasons.iter().map(|r| r.check_id.to_string()).collect();
    let score = DepthScore {
        checks_total,
        checks_passed,
        ratio,
        failed_check_ids,
    };

    if reasons.is_empty() {
        Verdict {
            outcome: Outcome::Ready,
            checked,
            score: Some(score),
            reasons,
        }
    } else {
        Verdict {
            outcome: Outcome::NotReady,
            checked,
            score: Some(score),
            reasons,
        }
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// The `depth_evidence` block for `freeze.v1` (F02 §5.2: "F06 fills it"). `None` for a
/// `MeasuredNothing` verdict (no score to report). `checked_check_ids` is always the full
/// vocabulary: `depth_evidence` is only ever called on a verdict produced by the CLI's real
/// `evaluate` path (never the test-only empty-slice call), so there is no narrower set to report.
pub fn depth_evidence(v: &Verdict) -> Option<Value> {
    let score = v.score.as_ref()?;
    let checked_ids: Vec<Value> = GATE_CHECK_IDS
        .iter()
        .map(|id| Value::String((*id).to_string()))
        .collect();
    Some(serde_json::json!({
        "score": {
            "checks_total": score.checks_total,
            "checks_passed": score.checks_passed,
            "ratio": score.ratio,
            "failed_check_ids": score.failed_check_ids,
        },
        "checked_check_ids": checked_ids,
    }))
}

/// The full `fleet gate lld-ready` decision, as a single pure function: shape first (F02's
/// validator), then readiness (this gate) -- §5.4's layering. Exists as a library function (not
/// only inline in `main.rs`) specifically so it is directly testable (F06-T7) with no file I/O:
/// `main.rs`'s binary-only code is invisible to `tests/f06_lld_ready.rs`, which links only
/// against this library crate.
pub enum EntryOutcome {
    ShapeInvalid(Vec<lld::Violation>),
    Gate(Verdict),
}

pub fn check_and_evaluate(brief: &Value, refs: &GateRefs) -> EntryOutcome {
    let violations = lld::validate_module_brief(brief);
    if !violations.is_empty() {
        return EntryOutcome::ShapeInvalid(violations);
    }
    EntryOutcome::Gate(evaluate(brief, refs))
}

// -------------------------------------------------------------------------------------------
// Small total accessors. Every check below is total over `serde_json::Value`: a missing, null,
// or mistyped field makes that check FAIL, never panic (mirrors `LldReadyGate.ts:227-231`'s
// `try { ... } catch { passed = false }`).
// -------------------------------------------------------------------------------------------

fn as_str(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn trim_len(v: Option<&Value>) -> usize {
    as_str(v).map(|s| s.trim().len()).unwrap_or(0)
}

fn as_array(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Array(items)) => items.as_slice(),
        _ => &[],
    }
}

fn as_str_array(v: Option<&Value>) -> Vec<&str> {
    as_array(v)
        .iter()
        .filter_map(|item| item.as_str())
        .collect()
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Case-insensitive whole-word/phrase containment, hand-written because this workspace has no
/// `regex` crate (`lld.rs:59-63`). Mirrors a JS `\b...\b` word-boundary match: `needle` must be
/// flanked by either a non-word character or start/end of string on both sides. This is what
/// keeps "whenever" from matching "never" (§11.1 item 1's named false-positive risk).
fn contains_word_ci(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    if n.is_empty() {
        return false;
    }
    for (pos, _) in h.match_indices(&n) {
        let before_ok = match h[..pos].chars().next_back() {
            Some(c) => !is_word_char(c),
            None => true,
        };
        let end = pos + n.len();
        let after_ok = match h[end..].chars().next() {
            Some(c) => !is_word_char(c),
            None => true,
        };
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// `/\b(zero|never|impossible|cannot|no way)\b/i`.
fn contains_absolute_claim(claim: &str) -> bool {
    ["zero", "never", "impossible", "cannot", "no way"]
        .iter()
        .any(|word| contains_word_ci(claim, word))
}

/// The 5-pattern tautology denylist, hand-ported from `TAUTOLOGY_DENYLIST` (`LldReadyGate.ts:62-68`).
/// NOT trimmed first -- the TS check does not trim `a.then` either, so a leading/trailing space
/// defeats the anchored patterns on both sides identically (byte-faithful port, not a "fix").
/// Four of the five patterns anchor both ends (`^...$`); the third (`/^(it )?should work/i`) has
/// no trailing anchor and is therefore a PREFIX match, on purpose -- see the lane contract §11.1.
fn matches_tautology(then: &str) -> bool {
    let t = then.to_lowercase();
    if t == "work" || t == "works" || t == "it work" || t == "it works" {
        return true;
    }
    if t == "correct" || t == "is correct" {
        return true;
    }
    if t.starts_with("should work") || t.starts_with("it should work") {
        return true;
    }
    if t == "passes test"
        || t == "passes tests"
        || t == "passes the test"
        || t == "passes the tests"
    {
        return true;
    }
    if t == "no error" || t == "no errors" {
        return true;
    }
    false
}

/// `NON_TRIGGER_DENYLIST` plus the `/^never$/i` early-allow, exactly as `checkR21Alts` orders it.
fn valid_revive_trigger(trigger: &str) -> bool {
    let t = trigger.trim().to_lowercase();
    if t == "never" {
        return true;
    }
    !matches!(t.as_str(), "tbd" | "n/a" | "none" | "-" | "?")
}

// -------------------------------------------------------------------------------------------
// The 14 checks. Port table + reuse targets: F06 lane contract §5.2.
// -------------------------------------------------------------------------------------------

fn check_c1_open(b: &Value, _refs: &GateRefs) -> bool {
    as_array(b.get("open_questions")).is_empty()
}

fn check_c2_owner(b: &Value, refs: &GateRefs) -> bool {
    match as_str(b.get("owner")) {
        Some(owner) => refs.owners.iter().any(|o| o == owner),
        None => false,
    }
}

fn check_c3_acc_parse(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let oracle_ok = matches!(
        a.and_then(|a| a.get("oracle_kind")).and_then(Value::as_str),
        Some("test") | Some("property") | Some("metric")
    );
    trim_len(a.and_then(|a| a.get("given"))) >= 3
        && trim_len(a.and_then(|a| a.get("when"))) >= 3
        && trim_len(a.and_then(|a| a.get("then"))) >= 3
        && oracle_ok
}

fn check_c3_acc_ground(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let artifact = as_str(a.and_then(|a| a.get("artifact"))).unwrap_or("");
    let owner_path = as_str(b.get("owner_path")).unwrap_or("");
    let then = as_str(a.and_then(|a| a.get("then"))).unwrap_or("");
    artifact.starts_with(owner_path) && then.contains(artifact)
}

fn check_c3_acc_nontaut(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let given = as_str(a.and_then(|a| a.get("given"))).unwrap_or("");
    let then = as_str(a.and_then(|a| a.get("then"))).unwrap_or("");
    if then == given {
        return false;
    }
    !matches_tautology(then)
}

fn check_r17_deriv(b: &Value, _refs: &GateRefs) -> bool {
    let guarantees = as_array(b.get("guarantees"));
    if guarantees.is_empty() {
        return false;
    }
    guarantees.iter().all(|g| {
        let derivation = g.get("derivation");
        match derivation
            .and_then(|d| d.get("kind"))
            .and_then(Value::as_str)
        {
            Some("number") => {
                let calc = as_str(derivation.and_then(|d| d.get("calc"))).unwrap_or("");
                lld::number_calc_ok(calc)
            }
            // Anything other than exactly "number" (including "structural", missing, or
            // malformed) falls through to the structural branch -- TS's checkR17Deriv only
            // special-cases `d.kind === 'number'` and treats everything else this way.
            _ => {
                let enforced_by =
                    as_str(derivation.and_then(|d| d.get("enforced_by"))).unwrap_or("");
                lld::structural_enforced_by_ok(enforced_by)
            }
        }
    })
}

fn check_r19_absolute(b: &Value, _refs: &GateRefs) -> bool {
    as_array(b.get("guarantees")).iter().all(|g| {
        let claim = as_str(g.get("claim")).unwrap_or("");
        if !contains_absolute_claim(claim) {
            return true;
        }
        let label_ok = as_str(g.get("label")) == Some("kills_structural");
        let kind_ok = as_str(g.get("derivation").and_then(|d| d.get("kind"))) == Some("structural");
        label_ok && kind_ok
    })
}

fn check_r21_alts(b: &Value, _refs: &GateRefs) -> bool {
    let alts = as_array(b.get("alternatives"));
    if alts.len() < 2 {
        return false;
    }
    let all_fields_present = alts.iter().all(|a| {
        trim_len(a.get("option")) != 0
            && trim_len(a.get("why_killed")) != 0
            && trim_len(a.get("revive_trigger")) != 0
    });
    if !all_fields_present {
        return false;
    }
    let valid_triggers = alts
        .iter()
        .all(|a| valid_revive_trigger(as_str(a.get("revive_trigger")).unwrap_or("")));
    if !valid_triggers {
        return false;
    }
    let mut options: Vec<String> = alts
        .iter()
        .map(|a| as_str(a.get("option")).unwrap_or("").trim().to_lowercase())
        .collect();
    let total = options.len();
    options.sort();
    options.dedup();
    options.len() == total
}

fn check_r21_fail(b: &Value, _refs: &GateRefs) -> bool {
    let f = b.get("failure_story");
    trim_len(f.and_then(|f| f.get("trigger"))) >= 10
        && trim_len(f.and_then(|f| f.get("blast_radius"))) >= 10
        && trim_len(f.and_then(|f| f.get("fail_safe"))) >= 10
}

fn check_c12_store(b: &Value, _refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id"));
    let items = as_array(b.get("data_owned"));
    let owned_ok = items.iter().all(|d| {
        as_str(d.get("owned_by_node")).is_some() && as_str(d.get("owned_by_node")) == node_id
    });
    if !owned_ok {
        return false;
    }
    let mut stores: Vec<&str> = items
        .iter()
        .map(|d| as_str(d.get("store")).unwrap_or(""))
        .collect();
    let total = stores.len();
    stores.sort();
    stores.dedup();
    stores.len() == total
}

fn check_c12_deps(b: &Value, refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id")).unwrap_or("");
    let deps = as_str_array(b.get("deps"));
    if deps.contains(&node_id) {
        return false;
    }
    if !deps
        .iter()
        .all(|d| refs.known_node_ids.iter().any(|k| k == d))
    {
        return false;
    }
    let stores: Vec<&str> = as_array(b.get("data_owned"))
        .iter()
        .map(|item| as_str(item.get("store")).unwrap_or(""))
        .collect();
    !deps.iter().any(|d| stores.contains(d))
}

fn check_reg_verdict(b: &Value, refs: &GateRefs) -> bool {
    let r = b.get("registry");
    match as_str(r.and_then(|r| r.get("kind"))) {
        Some("install") | Some("extract") => {
            let matched = as_str(r.and_then(|r| r.get("matched_path"))).unwrap_or("");
            refs.registry_paths.iter().any(|p| p == matched)
        }
        // build_new (or anything else -- TS falls through unconditionally to `r.searched`).
        _ => {
            let searched = as_str_array(r.and_then(|r| r.get("searched")));
            !searched.is_empty()
                && searched
                    .iter()
                    .all(|p| refs.registry_paths.iter().any(|rp| rp == p))
        }
    }
}

fn check_iface(b: &Value, _refs: &GateRefs) -> bool {
    let items = as_array(b.get("interface"));
    if items.is_empty() {
        return false;
    }
    items.iter().all(|decl| {
        let sig = as_str(decl.get("signature")).unwrap_or("");
        (sig.contains('(') && sig.contains(')'))
            || sig.starts_with("type ")
            || sig.starts_with("interface ")
    })
}

fn check_shape(b: &Value, _refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id")).unwrap_or("");
    let purpose = as_str(b.get("purpose")).unwrap_or("");
    let owner_path = as_str(b.get("owner_path")).unwrap_or("");
    lld::valid_node_id(node_id)
        && !purpose.is_empty()
        && purpose.chars().count() <= 200
        && !owner_path.is_empty()
        && !owner_path.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn complete_refs() -> GateRefs {
        GateRefs {
            owners: vec!["rachit@devxlabs.ai".to_string()],
            registry_paths: vec![
                "registry/services/llm-gateway".to_string(),
                "registry/features/cost-control-plane".to_string(),
            ],
            known_node_ids: vec!["orb-freeze-ledger".to_string()],
        }
    }

    #[test]
    fn absolute_claim_word_boundary_does_not_false_positive_on_whenever() {
        assert!(!contains_absolute_claim(
            "it works whenever the cache is warm"
        ));
        assert!(contains_absolute_claim("this never fails"));
    }

    #[test]
    fn tautology_anchor_does_not_match_a_longer_sentence() {
        assert!(!matches_tautology("it works when the cache is warm"));
        assert!(matches_tautology("it works"));
        assert!(matches_tautology("works"));
    }

    #[test]
    fn tautology_pattern_three_has_no_end_anchor_by_design() {
        // Byte-faithful port of LldReadyGate.ts's own asymmetry (§11.1): unlike the other four
        // patterns, `/^(it )?should work/i` has no trailing `$`, so it matches as a PREFIX.
        assert!(matches_tautology("it should work when the cache is warm"));
    }

    #[test]
    fn evaluate_is_total_over_a_completely_empty_object() {
        let verdict = evaluate(&json!({}), &complete_refs());
        assert_eq!(verdict.outcome, Outcome::NotReady);
        assert_eq!(verdict.checked, 14);
    }
}
