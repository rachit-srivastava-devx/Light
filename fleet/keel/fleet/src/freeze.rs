//! F09: the missing stamped-`Freeze` producer between F06's gate and F07's intake (lane contract
//! `docs/lane-contracts/F09-orb-fleet-handoff.md`). `fleet freeze stamp <brief.json> --out
//! <lld.json>` is the only CLI surface; this module holds the three pieces that surface needs and
//! nothing already shipped provides: the nested authority-key scan (§5.2), the module_brief ->
//! freeze/sow_seed projection (§6), and reading the on-disk freeze ledger to resolve
//! version/supersedes/idempotency (§5.4). Shape, readiness and content_hash are reused verbatim
//! from `lld`/`lld_ready` -- zero edits to either module.
//!
//! `pub mod freeze;` lives in `lib.rs` (unlike `sow`, which is a `main.rs`-local module) so
//! `tests/f09_freeze_stamp.rs` can call `project_freeze` directly with a `Verdict` built via
//! `lld_ready::evaluate_with(&brief, &refs, &[])` -- the `MeasuredNothing` branch is unreachable
//! through the CLI (which always calls the real `check_and_evaluate`, over the full 14-check
//! array), exactly the unreachable-branch trap F06 already fixed once for the TS gate (F06 lane
//! contract, `lld_ready.rs`'s own header comment) and must not be reintroduced here.
//!
//! This module does not write the ledger itself: `resolve_ledger` only READS
//! `$FLEET_STATE/freezes/<node_id>/*.json` to compute what the next version/supersedes/identity
//! should be. The actual write (and the `--out` write) stays in `main.rs`, which already owns
//! every other on-disk write in this crate (`sows/`, `ledger/`, `artifacts/`) -- this module gains
//! no new file-writing responsibility disjoint from that existing ownership split.

use crate::lld_ready::{self, Outcome, Verdict};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// The 6 authority keys a proposer must never graft into a `ModuleBrief`, at ANY depth (contract
/// §5.2). `lld::validate_module_brief`'s own `FORBIDDEN_MODULE_BRIEF_FIELDS` already refuses these
/// at the brief's TOP level; this closes the nested hole F07 proved live
/// (`guarantees[].derivation` is never type/key-checked -- only `kind`/`calc`/`enforced_by` are
/// ever read there) by walking every object key at every depth, not just the top level.
pub const FORBIDDEN_KEYS: [&str; 6] = [
    "stamped_by",
    "freeze_id",
    "content_hash",
    "depth_evidence",
    "state",
    "version",
];

/// Recursive, total KEY scan -- never a substring grep over rendered text (F05's recorded false
/// positive: `grep -c stamped_by` over a real response matched a `guarantees[].claim` STRING that
/// merely described the rule, on real data; pinned here by only ever inspecting object keys, never
/// string contents). Returns the dotted/bracket-indexed path of the FIRST forbidden key found in
/// document order (object keys are visited in the parsed `Map`'s own order; arrays in stored
/// order), or `None` if none exists at any depth.
pub fn find_forbidden_key(value: &Value) -> Option<String> {
    find_forbidden_key_at(value, "$")
}

fn find_forbidden_key_at(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_KEYS.contains(&key.as_str()) {
                    return Some(format!("{path}.{key}"));
                }
                if let Some(found) = find_forbidden_key_at(child, &format!("{path}.{key}")) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                if let Some(found) = find_forbidden_key_at(item, &format!("{path}[{index}]")) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// `"fz-" + blake3(content_hash + "@" + version).to_hex()[..16]` (contract §5.4). Deterministic --
/// same `content_hash`+`version` always yields the same id, which is what makes the ledger's
/// idempotency rule (§5.4) observable rather than merely intended. `blake3` is already a workspace
/// dependency (`sow::id_for_task` uses the same crate, same `to_hex()` API).
pub fn freeze_id_for(content_hash: &str, version: u64) -> String {
    let input = format!("{content_hash}@{version}");
    let hex = blake3::hash(input.as_bytes()).to_hex().to_string();
    format!("fz-{}", &hex[..16])
}

/// module_brief + Verdict -> the full `lld.v1` wrapper (contract §6, both tables). `None` iff
/// `verdict.outcome != Ready` -- covers both `NotReady` and `MeasuredNothing` in the one guard
/// F09-T5 exercises directly (a `MeasuredNothing` verdict is not reachable through the CLI, which
/// always evaluates the full 14-check vocabulary; this function's contract does not care how its
/// caller obtained the `Verdict`, so a test can hand it one built via
/// `lld_ready::evaluate_with(&brief, &refs, &[])` and observe the same refusal).
///
/// `content_hash`, `freeze_id`, `version` and `supersedes` are taken as already-resolved inputs
/// (STAMPED -- ledger-derived, contract §5.4) rather than computed here, so this function stays a
/// pure projection with no ledger/file-I/O concern of its own.
pub fn project_freeze(
    brief: &Value,
    verdict: &Verdict,
    content_hash: &str,
    freeze_id: &str,
    version: u64,
    supersedes: Option<&str>,
) -> Option<Value> {
    if verdict.outcome != Outcome::Ready {
        return None;
    }
    let depth_evidence = lld_ready::depth_evidence(verdict)?;

    let node_id = str_field(brief, "node_id");
    let owner = str_field(brief, "owner");
    let purpose = str_field(brief, "purpose");
    let acceptance = brief.get("acceptance").cloned().unwrap_or(Value::Null);
    let registry = brief.get("registry").cloned().unwrap_or(Value::Null);
    let interface = brief
        .get("interface")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let failure_story = brief.get("failure_story").cloned().unwrap_or(Value::Null);
    let alternatives = brief
        .get("alternatives")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let accepts_when = json!({
        "predicate": {
            "given": acceptance.get("given").cloned().unwrap_or(Value::Null),
            "when": acceptance.get("when").cloned().unwrap_or(Value::Null),
            "then": acceptance.get("then").cloned().unwrap_or(Value::Null),
            "oracle_kind": acceptance.get("oracle_kind").cloned().unwrap_or(Value::Null),
        },
        "artifact": acceptance.get("artifact").cloned().unwrap_or(Value::Null),
    });

    let freeze = json!({
        "schema_version": "1.0",
        "freeze_id": freeze_id,
        "version": version,
        "node_id": node_id,
        "content_hash": content_hash,
        "decision": purpose,
        "why": why_text(&alternatives),
        "killed_alternatives": alternatives,
        "accepts_when": accepts_when.clone(),
        "owner": owner,
        "depth_evidence": depth_evidence,
        "supersedes": supersedes,
        "stamped_by": lld_ready::STAMPED_BY,
    });

    let interface_names: Vec<String> = interface
        .iter()
        .map(|item| str_field(item, "name"))
        .collect();
    let restatement = format!(
        "Build {node_id} — registry verdict {}. Interface: {}. Fails if: {}",
        str_field(&registry, "kind"),
        interface_names.join(", "),
        str_field(&failure_story, "trigger"),
    );

    let sow_seed = json!({
        "restatement": restatement,
        "blind_suite_seed": accepts_when,
        "blast_radius": failure_story.get("blast_radius").cloned().unwrap_or(Value::Null),
        "owner": owner,
        "registry_verdict": registry,
    });

    Some(json!({
        "schema_version": "1.0",
        "module_brief": brief,
        "freeze": freeze,
        "sow_seed": sow_seed,
    }))
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// `"Chosen over " + N + " killed alternatives: " + join("; ", alternatives.map(a => a.option +
/// " — " + a.why_killed))` (contract §6.1). The separator is an em dash (U+2014), never a pipe --
/// `sow.rs`'s `sanitize` (reused unmodified downstream, at `fleet sow --lld` time) refuses any `|`
/// in every compiled field.
fn why_text(alternatives: &[Value]) -> String {
    let n = alternatives.len();
    let parts: Vec<String> = alternatives
        .iter()
        .map(|alt| {
            format!(
                "{} — {}",
                str_field(alt, "option"),
                str_field(alt, "why_killed")
            )
        })
        .collect();
    format!("Chosen over {n} killed alternatives: {}", parts.join("; "))
}

/// What the on-disk ledger says the next stamp for `node_id` must carry (contract §5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerResolution {
    /// `1 + max(existing versions)`, or `1` if none exist.
    pub version: u64,
    /// The prior version's `freeze_id`, or `None` at `version == 1`.
    pub supersedes: Option<String>,
    /// `Some(existing lld.v1)` when the latest entry for `node_id` already carries this exact
    /// `content_hash` -- the caller must return this unchanged (same `freeze_id`, same `version`,
    /// no new ledger entry) rather than allocate a new version (§5.4's idempotency rule).
    pub idempotent_existing: Option<Value>,
}

/// READS `$FLEET_STATE/freezes/<node_id>/*.json` (never writes -- `main.rs` performs the actual
/// ledger write, the same split every other piece of on-disk state in this crate already follows).
/// A missing directory is not an error: it means no freeze has ever been stamped for this
/// `node_id`, i.e. `version = 1`, `supersedes = None`.
pub fn resolve_ledger(
    state: &Path,
    node_id: &str,
    content_hash: &str,
) -> Result<LedgerResolution, String> {
    let dir = state.join("freezes").join(node_id);
    if !dir.is_dir() {
        return Ok(LedgerResolution {
            version: 1,
            supersedes: None,
            idempotent_existing: None,
        });
    }

    let mut versions: Vec<u64> = fs::read_dir(&dir)
        .map_err(|error| format!("could not list {}: {error}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                return None;
            }
            path.file_stem()?.to_str()?.parse::<u64>().ok()
        })
        .collect();
    if versions.is_empty() {
        return Ok(LedgerResolution {
            version: 1,
            supersedes: None,
            idempotent_existing: None,
        });
    }
    versions.sort_unstable();
    let latest_version = *versions.last().expect("versions is non-empty");
    let latest_path = dir.join(format!("{latest_version}.json"));
    let latest_text = fs::read_to_string(&latest_path)
        .map_err(|error| format!("could not read {}: {error}", latest_path.display()))?;
    let latest_value: Value = serde_json::from_str(&latest_text)
        .map_err(|error| format!("{} is not valid JSON: {error}", latest_path.display()))?;
    let latest_hash = latest_value
        .get("freeze")
        .and_then(|freeze| freeze.get("content_hash"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if latest_hash == content_hash {
        let supersedes = latest_value
            .get("freeze")
            .and_then(|freeze| freeze.get("supersedes"))
            .and_then(Value::as_str)
            .map(str::to_string);
        return Ok(LedgerResolution {
            version: latest_version,
            supersedes,
            idempotent_existing: Some(latest_value),
        });
    }

    let latest_freeze_id = latest_value
        .get("freeze")
        .and_then(|freeze| freeze.get("freeze_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(LedgerResolution {
        version: latest_version + 1,
        supersedes: latest_freeze_id,
        idempotent_existing: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn find_forbidden_key_walks_nested_objects_and_arrays_not_just_the_top_level() {
        // The VALUE deliberately is not the gate's real const -- this is a KEY scan, so any value
        // must trip it; using a different string here also keeps this test itself out of the
        // T12 census's literal count (freeze.rs's only real occurrence must be the production
        // reference to `lld_ready::STAMPED_BY`, not an incidental mention in a test fixture).
        let brief = json!({
            "guarantees": [
                {"claim": "c", "derivation": {"kind": "structural", "enforced_by": "gate:x", "stamped_by": "not-the-real-gate"}}
            ]
        });
        let found = find_forbidden_key(&brief).expect("nested stamped_by must be found");
        assert!(found.contains("stamped_by"), "{found}");
        assert!(
            found.contains("derivation"),
            "path should name the nesting: {found}"
        );
    }

    #[test]
    fn find_forbidden_key_never_matches_a_string_value_mentioning_the_word() {
        // The exact false-positive class F05 recorded: a prose STRING that merely mentions
        // "stamped_by" must not trip a KEY scan (mutation M9's pinned regression).
        let brief = json!({
            "guarantees": [
                {"claim": "the pipeline sets stamped_by only via the gate", "derivation": {"kind": "structural", "enforced_by": "gate:x"}}
            ]
        });
        assert_eq!(find_forbidden_key(&brief), None);
    }

    #[test]
    fn freeze_id_is_deterministic_and_versions_diverge() {
        let a = freeze_id_for("sha256:aaaa", 1);
        let b = freeze_id_for("sha256:aaaa", 1);
        assert_eq!(a, b);
        assert!(a.starts_with("fz-"));
        assert_eq!(a.len(), 19);
        let c = freeze_id_for("sha256:aaaa", 2);
        assert_ne!(a, c, "different version must yield a different freeze_id");
    }

    #[test]
    fn project_freeze_refuses_a_not_ready_or_measured_nothing_verdict() {
        let brief = json!({"node_id": "x"});
        let refs = lld_ready::GateRefs::default();
        let not_ready = lld_ready::evaluate(&brief, &refs);
        assert_eq!(not_ready.outcome, Outcome::NotReady);
        assert!(project_freeze(
            &brief,
            &not_ready,
            "sha256:aa",
            "fz-0000000000000000",
            1,
            None
        )
        .is_none());

        let measured_nothing = lld_ready::evaluate_with(&brief, &refs, &[]);
        assert_eq!(measured_nothing.outcome, Outcome::MeasuredNothing);
        assert!(project_freeze(
            &brief,
            &measured_nothing,
            "sha256:aa",
            "fz-0000000000000000",
            1,
            None
        )
        .is_none());
    }

    #[test]
    fn resolve_ledger_starts_at_version_one_with_no_directory() {
        let dir = std::env::temp_dir().join(format!(
            "fleet-freeze-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let resolution =
            resolve_ledger(&dir, "some-node", "sha256:aa").expect("no dir is not an error");
        assert_eq!(resolution.version, 1);
        assert_eq!(resolution.supersedes, None);
        assert!(resolution.idempotent_existing.is_none());
    }
}
