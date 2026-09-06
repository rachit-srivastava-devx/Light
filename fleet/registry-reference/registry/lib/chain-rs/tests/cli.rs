use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fleet-ledger-cli-integration-{label}-{}-{stamp}.jsonl",
        std::process::id()
    ))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet-ledger"))
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}.ckpt", path.display()));
    let _ = fs::remove_dir_all(format!("{}.lock", path.display()));
}

#[test]
fn main_maps_success_usage_unparseable_and_invariant_statuses() {
    let ledger = temp_path("status");
    let path = ledger.to_str().unwrap();

    let help = binary().arg("--help").output().unwrap();
    assert_eq!(help.status.code(), Some(0));
    let help_text = String::from_utf8(help.stdout).unwrap();
    assert!(help_text.contains("fleet-ledger verify"));
    assert!(help_text.contains("fleet-ledger append"));

    let usage = binary().output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8(usage.stderr)
        .unwrap()
        .contains("fleet-ledger invariants i3"));

    let verify = binary()
        .args(["verify", "--ledger", path])
        .output()
        .unwrap();
    assert_eq!(verify.status.code(), Some(0));

    let unparseable = binary()
        .args([
            "append",
            "--ledger",
            path,
            "ts",
            "event",
            "component",
            "task",
            "sonnet",
            "opus",
            "not-an-integer",
            "1",
            "2",
            "3",
        ])
        .output()
        .unwrap();
    assert_eq!(unparseable.status.code(), Some(4));

    let invariant = binary()
        .args(["invariants", "i1", "opus", "oracle"])
        .output()
        .unwrap();
    assert_eq!(invariant.status.code(), Some(6));

    cleanup(&ledger);
}

#[test]
fn main_maps_chain_broken_status() {
    let ledger = temp_path("broken");
    fs::write(&ledger, "not json\n").unwrap();
    let result = binary()
        .args(["verify", "--ledger", ledger.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(8));
    cleanup(&ledger);
}
