//! S2 (`docs/USER-JOURNEY-2.md`): `fleet version`/`fleet doctor` must reveal which BUILD is
//! running (git sha, dirty marker, build timestamp, crate version), human and `--json` alike --
//! previously neither said anything, so a stale installed binary looked like a live regression.
//! Drives the real binary; pins presence, not exact values (the sha is this checkout's own).

mod support;
use support::cmd;

fn get(v: &serde_json::Value, key: &str) -> String {
    v.get(key).unwrap_or_else(|| panic!("missing field {key:?} in {v}")).as_str().unwrap().to_string()
}

fn assert_identity_fields(v: &serde_json::Value) {
    assert!(!get(v, "commit_sha").is_empty());
    let tree_state = get(v, "tree_state");
    assert!(matches!(tree_state.as_str(), "clean" | "dirty" | "unknown"), "got {tree_state:?}");
    let build_time = get(v, "build_time");
    assert!(build_time.contains('T') && build_time.ends_with('Z'), "not RFC3339 UTC: {build_time:?}");
}

#[test]
fn version_json_has_build_identity() {
    let out = cmd().args(["version", "--json"]).output().expect("binary runs");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(get(&v, "version"), env!("CARGO_PKG_VERSION"));
    assert_identity_fields(&v);
}

#[test]
fn doctor_json_has_the_same_build_identity() {
    let out = cmd().args(["doctor", "--json"]).output().expect("binary runs");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_identity_fields(&v);
}

#[test]
fn version_human_output_mentions_sha_and_build_time() {
    let out = cmd().args(["version"]).output().expect("binary runs");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("commit_sha"), "human `version` missing commit_sha: {text}");
    assert!(text.contains("build_time"), "human `version` missing build_time: {text}");
}
