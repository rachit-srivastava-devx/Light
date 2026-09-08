//! Regression pin for the exact DX-AUDIT finding named in the task brief: `adjudicate` used to
//! be a disclosed stub (`--help` literally said "NOT IMPLEMENTED: fleet-verify has no
//! adjudication-table fn yet.") and nothing would have caught that label going stale once
//! `dispatch::run` grew a real arm for it (`adjudicate_cmd::adjudicate`, see `dispatch/mod.rs`).
//! A second regression this also catches for free: every command still routed through
//! `ops_cmd::not_yet_implemented` (source-truth: `dispatch/mod.rs`'s `other @ (...)` catch-all
//! arm) must still carry the label -- silently dropping it would misrepresent a real stub as
//! working. This is a ONE-DIRECTION check: a command NOT in that arm may still legitimately
//! disclose itself as unimplemented via its own inline `DispatchError::NotYetImplemented` (e.g.
//! `mcp`, see `worker_cmd.rs`) -- that is a true, disclosed gap, not a stale label.

mod support;
use support::cmd;

#[test]
fn stubs_still_wired_through_not_yet_implemented_keep_their_label() {
    let mod_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/dispatch/mod.rs"))
        .expect("dispatch/mod.rs readable");
    let arm_start = mod_rs.find("other @ (").expect("a not-yet-implemented arm");
    let arm_end = mod_rs[arm_start..].find("=> Err(ops_cmd::not_yet_implemented").expect("arm's own arrow");
    let arm = &mod_rs[arm_start..arm_start + arm_end];
    let stub_names: Vec<String> = arm
        .split("Commands::")
        .skip(1)
        .filter_map(|s| s.split(|c: char| !c.is_alphanumeric()).next())
        .map(kebab)
        .collect();
    assert!(!stub_names.is_empty(), "found no stub command names in dispatch/mod.rs -- measuring nothing");

    let out = cmd().arg("--help").output().expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut unlabeled = Vec::new();
    for line in stdout.split("Commands:\n").nth(1).expect("a Commands: section").lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !line.starts_with("  ") {
            break;
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").to_string();
        let desc = parts.next().unwrap_or("").trim();
        if stub_names.contains(&name) && !desc.contains("NOT IMPLEMENTED") {
            unlabeled.push(name);
        }
    }
    assert!(unlabeled.is_empty(), "still-a-stub command(s) missing their NOT IMPLEMENTED label: {unlabeled:?}");

    // The specific case the brief named: `adjudicate` must have left the stub set above AND
    // its label must have actually changed to match (not just been deleted).
    let adjudicate_desc = stdout.lines().find(|l| l.trim_start().starts_with("adjudicate ")).unwrap_or("");
    assert!(!stub_names.contains(&"adjudicate".to_string()), "adjudicate is still in the stub arm");
    assert!(
        !adjudicate_desc.contains("NOT IMPLEMENTED"),
        "adjudicate --help still carries a stale NOT IMPLEMENTED label: {adjudicate_desc:?}"
    );
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.char_indices() {
        if i > 0 && c.is_uppercase() {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}
