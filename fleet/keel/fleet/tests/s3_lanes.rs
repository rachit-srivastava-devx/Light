//! S3 acceptance: N parallel, worktree-isolated role lanes measurably overlap in wall-clock time.
//!
//! This is a Cargo INTEGRATION test: it drives the real, compiled `fleet` binary
//! (`env!("CARGO_BIN_EXE_fleet")`) as a subprocess via `__lanes_probe` (see that command's doc
//! comment in `src/main.rs` for why a unit test inside `cargo test`'s own harness cannot exercise
//! this path: `spawn_agent_with_args` re-execs `env::current_exe()`, which resolves to the test
//! harness binary itself when called from a `#[test]`, not to `fleet`).
//!
//! `__lanes_probe` calls the exact same production `run_role_lanes_concurrently` function that
//! `swarm dispatch --role <R>` uses, against the four non-Builder roles, with the deterministic
//! "stub" agent (no network/CLI credentials required, no flakiness from an external router
//! picking claude/codex/freelane). Every lane here is real work: a real `git worktree add -b
//! fleet/<name> .worktrees/<name> HEAD`, a real re-exec of `fleet __agent stub <worktree> <task>`
//! that really appends to that worktree's checked-out `main.rs`, and a real `git worktree remove
//! --force` afterward.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fleet_bin() -> &'static str {
    env!("CARGO_BIN_EXE_fleet")
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git must be on PATH for this test");
    assert!(status.success(), "git {args:?} failed in {repo:?}");
}

fn init_fixture_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fleet-s3-lanes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    run_git(&dir, &["init", "-q"]);
    run_git(&dir, &["config", "user.email", "s3-lanes-test@example.com"]);
    run_git(&dir, &["config", "user.name", "s3-lanes-test"]);
    std::fs::write(dir.join("main.rs"), b"fn main() {}\n").expect("write fixture main.rs");
    run_git(&dir, &["add", "-A"]);
    run_git(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

#[derive(Debug)]
struct LaneLine {
    role: String,
    agent: String,
    started_ms: u128,
    ended_ms: u128,
    exit: i32,
}

fn parse_lane_line(line: &str) -> Option<LaneLine> {
    let rest = line.strip_prefix("lane: ")?;
    let mut role = None;
    let mut agent = None;
    let mut started_ms = None;
    let mut ended_ms = None;
    let mut exit = None;
    for field in rest.split_whitespace() {
        let (key, value) = field.split_once('=')?;
        match key {
            "role" => role = Some(value.to_string()),
            "agent" => agent = Some(value.to_string()),
            "started_ms" => started_ms = value.parse().ok(),
            "ended_ms" => ended_ms = value.parse().ok(),
            "exit" => exit = value.parse().ok(),
            _ => {}
        }
    }
    Some(LaneLine {
        role: role?,
        agent: agent?,
        started_ms: started_ms?,
        ended_ms: ended_ms?,
        exit: exit?,
    })
}

#[test]
fn four_role_lanes_run_worktree_isolated_and_measurably_overlap_in_wall_clock() {
    let repo = init_fixture_repo();

    let output = Command::new(fleet_bin())
        .arg("__lanes_probe")
        .arg("--repo")
        .arg(&repo)
        .arg("--task")
        .arg("s3 concurrency acceptance probe")
        .output()
        .expect("failed to run fleet __lanes_probe");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "fleet __lanes_probe exited {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );

    let lanes: Vec<LaneLine> = stdout.lines().filter_map(parse_lane_line).collect();
    assert_eq!(
        lanes.len(),
        4,
        "expected 4 lane lines (lead, designer, verifier, meter), got {}: {stdout}",
        lanes.len()
    );
    let expected_roles = ["lead", "designer", "verifier", "meter"];
    for role in expected_roles {
        assert!(
            lanes.iter().any(|l| l.role == role),
            "missing a lane line for role {role}: {lanes:?}"
        );
    }
    for lane in &lanes {
        assert_eq!(lane.exit, 0, "lane {:?} did not exit 0: {stdout}", lane);
        assert_eq!(lane.agent, "stub");
        assert!(
            lane.ended_ms >= lane.started_ms,
            "lane {:?} ended before it started",
            lane
        );
    }

    // Every lane's worktree must be gone -- a leaked worktree is exactly the failure mode S3's
    // cleanup guarantee exists to prevent.
    let worktrees_dir = repo.join(".worktrees");
    let leaked: Vec<_> = if worktrees_dir.exists() {
        std::fs::read_dir(&worktrees_dir)
            .expect("read .worktrees")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect()
    } else {
        Vec::new()
    };
    assert!(
        leaked.is_empty(),
        "worktree(s) leaked after all lanes completed: {leaked:?}"
    );

    // The actual overlap proof: at least one pair of the four lanes has intersecting
    // [started_ms, ended_ms] windows, measured with one shared `Instant` epoch inside the
    // process (see `run_role_lanes_concurrently` in src/main.rs). Strictly sequential lanes
    // (the pre-S3 behaviour -- a milestone bump per role, no concurrent execution at all) could
    // never produce two intersecting windows.
    let mut any_overlap = false;
    for i in 0..lanes.len() {
        for j in (i + 1)..lanes.len() {
            let a = &lanes[i];
            let b = &lanes[j];
            let overlap_start = a.started_ms.max(b.started_ms);
            let overlap_end = a.ended_ms.min(b.ended_ms);
            if overlap_start < overlap_end {
                any_overlap = true;
            }
        }
    }
    assert!(
        any_overlap,
        "no two of the 4 role lanes overlapped in wall-clock time -- they ran sequentially, \
         not concurrently: {lanes:?}"
    );

    // Corroborating, coarser signal: real concurrency means total wall time for the batch is
    // less than the sum of the four lanes' own individual durations.
    let batch_span_ms = lanes.iter().map(|l| l.ended_ms).max().unwrap_or(0)
        - lanes.iter().map(|l| l.started_ms).min().unwrap_or(0);
    let sum_of_durations: u128 = lanes
        .iter()
        .map(|l| l.ended_ms.saturating_sub(l.started_ms))
        .sum();
    assert!(
        batch_span_ms < sum_of_durations,
        "batch span ({batch_span_ms}ms) was not less than the sum of individual lane durations \
         ({sum_of_durations}ms) -- lanes do not appear to have run in parallel: {lanes:?}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn lanes_probe_refuses_cleanly_on_missing_flags() {
    let output = Command::new(fleet_bin())
        .arg("__lanes_probe")
        .output()
        .expect("failed to run fleet __lanes_probe");
    assert!(
        !output.status.success(),
        "missing --repo/--task must not exit 0"
    );
}
