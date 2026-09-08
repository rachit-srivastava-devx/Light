//! Location-independence: embedded gate scripts materialize with the exec bit and are runnable,
//! an override root takes precedence over the embedded copies, and a missing/unknown gate is a
//! typed error rather than a dangling path handed to `ProcessRunner`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use fleet_verify::{GateAssetError, GatesRoot};

#[test]
fn embedded_scripts_materialize_with_exec_bit_and_are_runnable() {
    let gates = GatesRoot::materialize().expect("embedded assets must materialize");

    for relative in ["semgrep-gate.sh", "trivy-gate.sh", "recur-gate.sh", "detector-integrity.sh"] {
        let path = gates.require(relative).unwrap_or_else(|e| panic!("{relative}: {e}"));
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "{relative} is not executable (mode {mode:o})");
    }

    // `detector-integrity.sh` is self-contained against its own materialized corpus copy: it
    // should run to completion (whatever verdict it reaches) rather than fail to even start.
    let path = gates.require("detector-integrity.sh").unwrap();
    let status = Command::new(&path).status().expect("script must be spawnable");
    assert!(status.code().is_some(), "script must exit, not signal-die");
}

#[test]
fn override_root_takes_precedence_over_embedded() {
    let dir = tempfile::tempdir().unwrap();
    let stub = dir.path().join("semgrep-gate.sh");
    fs::write(&stub, "#!/usr/bin/env bash\necho override-stub\n").unwrap();
    let mut perm = fs::metadata(&stub).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&stub, perm).unwrap();

    let gates = GatesRoot::from_override(dir.path()).expect("override dir exists");
    let resolved = gates.require("semgrep-gate.sh").expect("stub is present");
    assert_eq!(resolved, stub);
    let contents = fs::read_to_string(&resolved).unwrap();
    assert_eq!(contents, "#!/usr/bin/env bash\necho override-stub\n");

    // Not shadowed by the embedded copy's real content (which is much longer / different).
    assert!(contents.len() < 100);
}

#[test]
fn missing_override_root_is_a_typed_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    match GatesRoot::from_override(&missing) {
        Err(GateAssetError::OverrideMissing(p)) => assert_eq!(p, missing),
        Err(other) => panic!("expected OverrideMissing, got {other:?}"),
        Ok(_) => panic!("expected OverrideMissing, got Ok"),
    }
}

#[test]
fn unknown_script_under_a_good_root_is_a_typed_error() {
    let gates = GatesRoot::materialize().unwrap();
    match gates.require("definitely-not-a-real-gate.sh") {
        Err(GateAssetError::UnknownScript(name)) => assert_eq!(name, "definitely-not-a-real-gate.sh"),
        other => panic!("expected UnknownScript, got {other:?}"),
    }
}
