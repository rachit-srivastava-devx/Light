//! A REFUSED `sow` must leave memory untouched. It used to write the refused text, so the
//! caller's corrected resubmission collided with their own rejected draft:
//! `REFUSED: ambiguity (Memory): This looks like a prior decision -- which one governs?
//!  -- Similar to a remembered item: "<the draft that was just rejected>"`.
//! Only an ACCEPTED SOW is a prior decision worth remembering. Drives the real binary.

mod support;
use support::cmd;
use std::path::Path;

const DRAFT: &str = "fix the flaky pagination test";
const VALID: &str = "source_intent_hash: abc123\n\
request: fix the flaky pagination test\n\n\
## Request restatement\nMake the pagination test deterministic and exit 0.\n\n\
## Built for\nCLI users and the fleet team; the success metric is a green suite.\n\n\
## Must do\n- Seed the fixture ordering\n- Assert on a stable page cursor\n\n\
## Explicitly will not do\n- Not touching the pager itself\n- Out of scope: perf work\n\n\
## Done when\nThe suite exits 0 on ten consecutive runs\n\n\
## Acceptance threshold\n100% of ten runs exit 0\n";

fn run(state_dir: &Path, text: &str) -> (bool, String) {
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir)
        .args(["sow", "--text", text, "--intent-hash", "abc123"])
        .output()
        .expect("binary runs");
    (out.status.success(), String::from_utf8_lossy(&out.stderr).to_string())
}

fn store(state_dir: &Path) -> std::path::PathBuf {
    state_dir.join("memory").join("sow.json")
}

#[test]
fn a_refused_sow_writes_nothing_to_memory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok, err) = run(dir.path(), DRAFT);
    assert!(!ok, "a bare draft must still be refused\n{err}");
    assert!(
        !store(dir.path()).exists(),
        "memory must be unchanged after a refusal, found {}",
        store(dir.path()).display()
    );
}

#[test]
fn a_corrected_draft_does_not_collide_with_the_rejected_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok1, first) = run(dir.path(), DRAFT);
    assert!(!ok1, "step 1 must refuse\n{first}");
    let (ok2, second) = run(dir.path(), VALID);
    assert!(
        !second.contains("prior decision"),
        "the fixed draft must not be refused for matching the rejected one\n{second}"
    );
    assert!(ok2, "step 2 must be accepted\n{second}");
}

#[test]
fn an_accepted_sow_is_remembered() {
    // The write path stays wired -- the verdict, not the wiring, is what changed.
    let dir = tempfile::tempdir().expect("tempdir");
    let (ok, err) = run(dir.path(), VALID);
    assert!(ok, "a well-formed sow must be accepted\n{err}");
    assert!(store(dir.path()).exists(), "an accepted sow must be recorded");
}
