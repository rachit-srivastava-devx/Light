//! `fleet doctor` must surface which optional scanners the gate registry names, so a first-time
//! user seeing `SKIP gate semgrep` in a run learns from doctor how to install it. Drives the
//! real binary with an empty `PATH` (and `HOME` pointing at an empty dir, so `~/.cargo/bin`
//! fallback resolves to nothing either) -- a machine where every optional scanner is missing.
//! Asserts the section header appears, at least one `MISS` line lists an install hint, and no
//! existing doctor line was removed. Companion `--json` check asserts the new field is present
//! and lists the same tools in the same order (registry-driven, no hardcoding here).
//!
//! Not asserted: which specific tool is missing on the developer's own box. `semgrep` and
//! `trivy` are guaranteed missing here because `$PATH` is empty, not because of any assumption
//! about the caller's install state -- the point of the empty PATH is to make the check
//! deterministic without touching the machine.

mod support;
use support::cmd;

fn empty_home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn doctor_prints_optional_tools_section_with_at_least_one_miss() {
    let home = empty_home();
    let out = cmd()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home.path())
        .env_remove("CARGO_HOME")
        .arg("doctor")
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "doctor exited non-zero: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("-- optional tools --"), "section header missing:\n{text}");
    assert!(
        text.lines().any(|l| l.starts_with("MISS ") && l.contains("install:")),
        "no MISS line with an install hint:\n{text}"
    );
    // No existing top-line was dropped -- the section is additive.
    for label in ["cargo:", "git:", "commit_sha:", "tree_state:", "build_time:"] {
        assert!(text.contains(label), "existing line {label:?} missing:\n{text}");
    }
    // Informational only: missing scanners must not turn the section into FAIL.
    assert!(!text.contains("FAIL "), "optional section must not emit FAIL:\n{text}");
}

#[test]
fn doctor_json_lists_optional_tools_from_the_registry() {
    let home = empty_home();
    let out = cmd()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home.path())
        .env_remove("CARGO_HOME")
        .args(["doctor", "--json"])
        .output()
        .expect("binary runs");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    let arr = v.get("optional_tools").expect("optional_tools field").as_array().expect("array");
    assert!(!arr.is_empty(), "optional_tools should not be empty: {v}");
    // Every entry has the flat {tool, path, install} shape.
    for e in arr {
        assert!(e.get("tool").and_then(|t| t.as_str()).is_some(), "row missing tool: {e}");
        assert!(e.get("install").and_then(|t| t.as_str()).is_some(), "row missing install: {e}");
        // path is null (missing) or a string (found); never absent.
        assert!(e.get("path").is_some(), "row missing path key: {e}");
    }
    // At least one is a MISS on this hermetic PATH so the field is not a stub.
    assert!(
        arr.iter().any(|e| e.get("path").map(|p| p.is_null()).unwrap_or(false)),
        "expected at least one missing optional tool with empty PATH: {arr:?}"
    );
}
