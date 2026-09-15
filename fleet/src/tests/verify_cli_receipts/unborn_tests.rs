use super::*;
use std::fs;

#[test]
fn unborn_git_gate_records_directory_fallback_in_canonical_receipt() {
    let repo = tempfile::tempdir().expect("repo");
    let init = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(init.success());
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname=\"unborn-verify\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("manifest");
    fs::create_dir(repo.path().join("src")).expect("src");
    fs::write(repo.path().join("src/lib.rs"), "pub fn proof() {}\n").expect("source");
    let state = tempfile::tempdir().expect("state");
    let out = run(
        state.path(),
        &[
            "gate",
            "--id",
            "unit tests",
            "--repo",
            repo.path().to_str().expect("utf8"),
        ],
    );
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let rows = ledger(state.path()).rows(false).expect("receipt rows");
    let row = rows
        .iter()
        .find(|row| row.event == ReceiptEvent::GateVerdict)
        .expect("gate receipt");
    assert_eq!(row.body["secret_scan"]["git_requested"], true);
    assert_eq!(row.body["secret_scan"]["git_backed"], false);
    assert_eq!(row.body["secret_scan"]["no_head_fallback"], true);
}
