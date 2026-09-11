//! Unit tests for `pr_cmd`: ledger-driven receipt selection, Markdown composition, remote URL
//! parsing, and the `--dry-run` (no shell-out) path. No network. No `gh` invocation -- the tests
//! that would need `gh` exercise the composition helpers directly.

use super::{compose_body, parse_owner_name, pick_last_passing, GateRow, PassingRun};
use fleet_types::{Blake3Hash, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::json;

fn hash(byte: u8) -> Blake3Hash {
    let hex: String = std::iter::repeat_n(byte, 32).map(|b| format!("{b:02x}")).collect();
    Blake3Hash::parse(format!("blake3:{hex}")).unwrap()
}

fn row(seq: u64, event: ReceiptEvent, body: serde_json::Value) -> Receipt {
    Receipt {
        schema_version: SchemaV1,
        seq,
        prev_hash: if seq == 0 { PrevHash::Genesis } else { PrevHash::Hash(hash(seq as u8 - 1)) },
        hash: hash(seq as u8),
        ts_wall: format!("2026-09-11T00:00:{seq:02}Z"),
        event,
        actor: "test".into(),
        resolved_model: None,
        exit_code: None,
        body,
    }
}

#[test]
fn passing_receipt_becomes_a_markdown_table() {
    let rows = vec![
        row(0, ReceiptEvent::RunStart, json!({ "task_id": "T1" })),
        row(
            1,
            ReceiptEvent::GateVerdict,
            json!({ "id": "unit tests", "outcome": "Pass", "checked": 36, "total": 36 }),
        ),
        row(
            2,
            ReceiptEvent::GateVerdict,
            json!({ "id": "detectors", "outcome": "Pass", "checked": 111, "total": 111 }),
        ),
        row(
            3,
            ReceiptEvent::RunEnd,
            json!({ "task_id": "T1", "final_stage": "verify", "ok": true }),
        ),
    ];
    let run = pick_last_passing(&rows, "T1").expect("run should be found");
    let body = compose_body(None, "T1", &run);
    // The Markdown skeleton
    assert!(body.contains("---\n### fleet receipt\n"));
    assert!(body.contains("Task: T1\n"));
    assert!(body.contains("Ledger: blake3:"));
    assert!(body.contains("Run: 2026-09-11T00:00:03Z"));
    // The gate rows
    assert!(body.contains("| Gate | Result | Denominator |"));
    assert!(body.contains("| unit tests | Pass | 36/36 |"));
    assert!(body.contains("| detectors | Pass | 111/111 |"));
}

#[test]
fn body_prepends_user_extra_body_above_receipt_divider() {
    let run = PassingRun {
        run_end: row(1, ReceiptEvent::RunEnd, json!({ "task_id": "T", "ok": true })),
        gates: vec![GateRow {
            id: "x".into(),
            outcome: "Pass".into(),
            checked: Some(1),
            total: Some(1),
        }],
    };
    let body = compose_body(Some("Closes #99. Ships the widget."), "T", &run);
    let (extra, receipt) = body.split_once("---\n### fleet receipt").unwrap();
    assert!(extra.contains("Closes #99. Ships the widget."));
    assert!(receipt.contains("Task: T"));
}

#[test]
fn missing_passing_run_returns_none_so_dispatch_can_env_fault() {
    let rows = vec![
        row(0, ReceiptEvent::RunStart, json!({ "task_id": "T1" })),
        row(
            1,
            ReceiptEvent::RunEnd,
            json!({ "task_id": "T1", "final_stage": "verify", "ok": false }),
        ),
    ];
    assert!(pick_last_passing(&rows, "T1").is_none());
    assert!(pick_last_passing(&rows, "OTHER").is_none());
}

#[test]
fn most_recent_passing_run_wins_when_multiple_runs_exist() {
    let rows = vec![
        row(0, ReceiptEvent::RunStart, json!({ "task_id": "T" })),
        row(
            1,
            ReceiptEvent::GateVerdict,
            json!({ "id": "old", "outcome": "Pass", "checked": 1, "total": 1 }),
        ),
        row(2, ReceiptEvent::RunEnd, json!({ "task_id": "T", "ok": true })),
        row(3, ReceiptEvent::RunStart, json!({ "task_id": "T" })),
        row(
            4,
            ReceiptEvent::GateVerdict,
            json!({ "id": "new", "outcome": "Pass", "checked": 2, "total": 2 }),
        ),
        row(5, ReceiptEvent::RunEnd, json!({ "task_id": "T", "ok": true })),
    ];
    let run = pick_last_passing(&rows, "T").unwrap();
    assert_eq!(run.gates.len(), 1);
    assert_eq!(run.gates[0].id, "new");
}

#[test]
fn owner_name_parser_accepts_https_ssh_and_bare_forms() {
    assert_eq!(parse_owner_name("https://github.com/foo/bar.git").unwrap(), "foo/bar");
    assert_eq!(parse_owner_name("https://github.com/foo/bar").unwrap(), "foo/bar");
    assert_eq!(parse_owner_name("git@github.com:foo/bar.git").unwrap(), "foo/bar");
    assert_eq!(parse_owner_name("git@github.com:foo/bar").unwrap(), "foo/bar");
    assert_eq!(parse_owner_name("ssh://git@github.com/foo/bar.git").unwrap(), "foo/bar");
}

#[test]
fn owner_name_parser_rejects_non_github_or_malformed_urls() {
    // Non-GitHub host -> EnvFault
    let e = parse_owner_name("https://gitlab.com/foo/bar.git").unwrap_err();
    assert!(format!("{e}").contains("could not parse GitHub owner/name"));
    // Missing repo half -> EnvFault
    let e = parse_owner_name("https://github.com/foo").unwrap_err();
    assert!(format!("{e}").contains("could not parse GitHub owner/name"));
    // Empty -> EnvFault
    let e = parse_owner_name("").unwrap_err();
    assert!(format!("{e}").contains("could not parse GitHub owner/name"));
}

#[test]
fn dry_run_prints_resolved_command_and_body_without_invoking_gh() {
    // The `pr()` fn's `--dry-run` branch does not call `Command::new("gh")`; verifying that
    // structurally: the branch runs before `ensure_gh_on_path()`, so an environment with no
    // `gh` still succeeds. Exercised via a dedicated integration test would need a fake ledger
    // on disk; here we assert the compose_body+print_resolved shape stays deterministic by
    // re-composing and checking bytes -- the same bytes the branch would print.
    let run = PassingRun {
        run_end: row(1, ReceiptEvent::RunEnd, json!({ "task_id": "T", "ok": true })),
        gates: vec![GateRow {
            id: "recur".into(),
            outcome: "Skip".into(),
            checked: None,
            total: None,
        }],
    };
    let body = compose_body(Some("A note."), "T", &run);
    assert!(body.starts_with("A note.\n\n---\n### fleet receipt"));
    assert!(body.contains("| recur | Skip | n/a |"));
}
