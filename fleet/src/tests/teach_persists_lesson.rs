//! S2 proof: `Teach` used to derive a real `Lesson` and drop it (`let _lesson = ...`) -- every
//! failing run reached `Teach` and taught nothing. `pipeline::teach_stage::teach` now persists it
//! via `dispatch::memory::record_sow_refusal` (`memory/sow.json` under `state_dir`). This proves
//! it survives the process: run the real binary once (a `--repo` pointed at a non-git directory
//! makes `Merge` fail for real, which reaches `Teach` with a genuine failure to teach), let it
//! exit, then -- in a separate step, after the child process is gone -- read the lesson back off
//! disk.

use std::fs;

mod support;
use support::cmd;

#[test]
fn a_failed_run_writes_a_lesson_that_outlives_the_process() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap(); // deliberately NOT a git work tree: Merge must fail
    let task_id = "teach-persists-lesson";

    let output = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .args(["__pipeline_probe", "--task-id", task_id, "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    assert!(!output.status.success(), "a non-worktree --repo must make Merge fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"final_stage\": \"Teach\""), "must reach Teach: {stdout}");

    // The child process above has already exited by the time `.output()` returns -- this is a
    // separate read, off disk, of a file no longer-running process could still be holding open.
    let lesson_path = state_dir.path().join("memory").join("sow.json");
    let persisted = fs::read_to_string(&lesson_path)
        .unwrap_or_else(|e| panic!("lesson must survive the process at {lesson_path:?}: {e}"));
    assert!(persisted.contains("merge"), "lesson must name the failing stage: {persisted}");
    assert!(persisted.contains("mitigation="), "lesson must carry a mitigation field: {persisted}");
}
