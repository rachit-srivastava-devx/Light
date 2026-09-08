//! The proof that `fleet-memory` is actually wired into `sow`, not merely declared as a
//! dependency: the SAME text, run twice against the SAME `FLEET_STATE_DIR`, must differ on the
//! second run -- BECAUSE the first run's refusal wrote a memory row the second run recalls. If
//! the retrieval call were disconnected, both runs would be byte-identical and this test fails
//! (see `docs/` note in the task report for the watched-fail proof of this file).

mod support;
use support::cmd;

const TEXT: &str = "fix the flaky pagination test";

fn run(state_dir: &std::path::Path) -> (bool, String) {
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir)
        .args(["sow", "--text", TEXT, "--intent-hash", "x"])
        .output()
        .expect("binary runs");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn a_second_identical_sow_recalls_the_first_runs_refusal() {
    let state_dir = tempfile::tempdir().expect("tempdir");

    let (ok1, first) = run(state_dir.path());
    assert!(!ok1, "first run: still a bare --text, should be refused\n{first}");
    assert!(
        !first.contains("This looks like a prior decision"),
        "first run must not recall anything -- memory is empty\n{first}"
    );

    let (ok2, second) = run(state_dir.path());
    assert!(!ok2, "second run: still refused\n{second}");
    assert!(
        second.contains("This looks like a prior decision"),
        "second run over the SAME state dir and SAME text must recall the memory the first \
         run wrote -- if this fails, retrieval is disconnected from the write side\nfirst:\n{first}\nsecond:\n{second}"
    );

    assert_ne!(first, second, "the second run must differ from the first BECAUSE of memory recall");
}
