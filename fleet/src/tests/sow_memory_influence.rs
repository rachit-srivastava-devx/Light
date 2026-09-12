//! The proof that `fleet-memory` is actually wired into `sow`, not merely declared as a
//! dependency: the SAME accepted text, run twice against the SAME `FLEET_STATE_DIR`, must
//! differ on the second run -- BECAUSE the first run's acceptance wrote a memory row the
//! second run recalls. If retrieval is disconnected, both runs succeed and this test fails.
//! Only ACCEPTED SOWs are prior decisions worth remembering (see `memory/write.rs`).

mod support;
use support::cmd;

const VALID: &str = "source_intent_hash: abc123\n\
request: fix the flaky pagination test\n\n\
## Request restatement\nMake the pagination test deterministic and exit 0.\n\n\
## Built for\nCLI users and the fleet team; the success metric is a green suite.\n\n\
## Must do\n- Seed the fixture ordering\n- Assert on a stable page cursor\n\n\
## Explicitly will not do\n- Not touching the pager itself\n- Out of scope: perf work\n\n\
## Done when\nThe suite exits 0 on ten consecutive runs\n\n\
## Acceptance threshold\n100% of ten runs exit 0\n";

fn run(state_dir: &std::path::Path) -> (bool, String) {
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir)
        .args(["sow", "--text", VALID, "--intent-hash", "abc123"])
        .output()
        .expect("binary runs");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).to_string())
}

#[test]
fn a_second_identical_sow_recalls_the_first_runs_memory() {
    let state_dir = tempfile::tempdir().expect("tempdir");

    let (ok1, first) = run(state_dir.path());
    assert!(ok1, "first run: valid SOW must be accepted\n{first}");
    assert!(
        !first.contains("This looks like a prior decision"),
        "first run must not recall anything -- memory is empty\n{first}"
    );

    let (ok2, second) = run(state_dir.path());
    assert!(!ok2, "second run: same SOW must be refused as prior decision\n{second}");
    assert!(
        second.contains("This looks like a prior decision"),
        "second run over the SAME state dir and SAME text must recall the memory the first \
         run wrote -- if this fails, retrieval is disconnected from the write side\
         \nfirst:\n{first}\nsecond:\n{second}"
    );

    assert_ne!(first, second, "second run must differ from first BECAUSE of memory recall");
}
