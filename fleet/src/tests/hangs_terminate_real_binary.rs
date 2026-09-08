//! Pins the three S1 hangs from `docs/DX-AUDIT.md` fixed: `fleet run`/`fleet oracle`/`fleet gate`
//! used to block indefinitely because `fleet_verify::GATES` includes `cargo test --workspace`
//! (measured ~94s) and `cargo mutants` (minutes-to-hours), run via a plain, unbounded
//! `Command::output()` in `dispatch::verify_ports::RealRunner`. Every test here drives the REAL
//! compiled binary (`env!("CARGO_BIN_EXE_fleet")`), never a substituted fake -- the project
//! shipped a broken `__agent` past 351 green tests precisely because tests used a fake-binary
//! seam. `FLEET_VERIFY_BUDGET_SECS` is set short so these stay fast; `run_bounded` below is the
//! test's OWN wall-clock cap, so a regression back to an unbounded hang fails this test instead
//! of hanging the suite.

mod support;
use support::cmd;

use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Run `bin() args...` with `stdin` closed, bounded to `budget` wall-clock time. `None` means the
/// deadline was hit and the child was killed -- a regression back to a hang, not a pass.
fn run_bounded(args: &[&str], envs: &[(&str, &str)], budget: Duration) -> Option<(i32, String, String)> {
    let mut c = cmd();
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        c.env(k, v);
    }
    let mut child = c.spawn().expect("binary spawns");
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            let mut out = String::new();
            let mut err = String::new();
            if let Some(mut o) = child.stdout.take() {
                let _ = o.read_to_string(&mut out);
            }
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Some((status.code().unwrap_or(-1), out, err));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn fleet_run_terminates_with_a_real_exit_code_instead_of_hanging() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let repo_arg = repo.path().to_string_lossy().into_owned();
    let state_env = state_dir.path().to_string_lossy().into_owned();

    let result = run_bounded(
        &["run", "--repo", &repo_arg, "--task", "add a test"],
        &[("FLEET_STATE_DIR", &state_env), ("FLEET_VERIFY_BUDGET_SECS", "2")],
        Duration::from_secs(25),
    );
    let (code, out, err) = result.expect("fleet run must terminate, not hang, within 25s");
    assert_ne!(code, 0, "an empty scratch repo cannot pass real verify gates: stdout={out} stderr={err}");
    assert!(err.contains("stage"), "expected stage progress on stderr, got: {err}");
}

#[test]
fn fleet_oracle_with_no_args_terminates_with_guidance_and_nonzero_exit() {
    let result = run_bounded(&["oracle"], &[("FLEET_VERIFY_BUDGET_SECS", "2")], Duration::from_secs(20));
    let (code, _out, err) = result.expect("fleet oracle must terminate, not hang, within 20s");
    assert_ne!(code, 0, "stderr: {err}");
}

#[test]
fn fleet_gate_with_no_args_terminates_with_guidance_and_nonzero_exit() {
    let result = run_bounded(&["gate"], &[("FLEET_VERIFY_BUDGET_SECS", "2")], Duration::from_secs(20));
    let (code, _out, err) = result.expect("fleet gate must terminate, not hang, within 20s");
    assert_ne!(code, 0, "stderr: {err}");
}
