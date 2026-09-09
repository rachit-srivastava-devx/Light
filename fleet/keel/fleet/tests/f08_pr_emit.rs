//! F08 acceptance: an Accepted task becomes a real open PR carrying the attested diff,
//! and the lifecycle refuses the edge when the attestation is incomplete.
//!
//! Cargo INTEGRATION test — drives the real `fleet` binary (see tests/s3_lanes.rs for why a
//! unit test cannot exercise this path). `origin` is a real local bare repo, so the push is
//! real; only `gh` is shimmed, because the test must not require live GitHub credentials.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// `git init` / `git init --bare` copy Homebrew git's hook/template directory
/// (`--template=$(brew --prefix)/share/git-core/templates`, git's own compiled-in default) on
/// every invocation. Under `cargo test`'s default parallelism this test's own fixture setup
/// calls `git init` from multiple threads near-simultaneously, and two can race on copying the
/// same template file: `fatal: cannot copy '.../git-core/templates/hooks/....sample' to
/// '...': File exists`. That is a transient race in git's template-copy step, not a real setup
/// failure -- retry with a short jittered backoff, mirroring `worktree.rs::create`'s handling
/// of the same class of git administrative-file contention (`src/worktree.rs:58-76`). Scoped
/// to exactly the two `git init` calls in `init_fixture_repo` below; every other call in this
/// file still goes through the plain, non-retrying `run_git` above.
fn run_git_init(repo: &Path, args: &[&str]) {
    let mut last_stderr = String::new();
    for attempt in 0..8u32 {
        if attempt > 0 {
            // Same cheap, dependency-free jitter as `worktree.rs::create`: mix the pid, the
            // attempt number, and a coarse timestamp to desynchronise racing threads.
            let now_nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let jitter_ms = (std::process::id() ^ now_nanos ^ attempt) % 40;
            std::thread::sleep(std::time::Duration::from_millis(10 + u64::from(jitter_ms)));
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git must be on PATH for this test");
        if output.status.success() {
            return;
        }
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        // Only retry the exact transient template-copy race; any other failure is real and
        // must fail immediately, exactly as `run_git` does for every other call in this file.
        if !(stderr.contains("cannot copy") && stderr.contains("File exists")) {
            panic!("git {args:?} failed in {repo:?}: {stderr}");
        }
        last_stderr = stderr;
    }
    panic!(
        "git {args:?} failed in {repo:?} after 8 retries (template-copy race never cleared): {last_stderr}"
    );
}

/// This file's 4 tests run as threads inside one `cargo test` process, so `std::process::id()`
/// is constant across all of them, and this clock's resolution is coarse enough that two threads
/// calling `unique_dir` in the same tick can compute identical nanos too -- observed directly: a
/// failing run's panic messages showed two different test threads both citing the byte-identical
/// path `fleet-f08-repo-59515-1788729836773281000`, racing inside the same "unique" directory.
/// Same fix as `worktree.rs::unique_name` for the identical race: fold in a monotonic in-process
/// counter so two threads can never produce the same path.
static NEXT_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_dir(tag: &str) -> PathBuf {
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fleet-f08-{tag}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        seq
    ));
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

/// A real work repo with a real local bare `origin`. Nothing here is stubbed.
fn init_fixture_repo() -> (PathBuf, PathBuf) {
    let bare = unique_dir("origin");
    run_git_init(&bare, &["init", "--bare", "-q"]);
    let repo = unique_dir("repo");
    run_git_init(&repo, &["init", "-q", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "f08-test@example.com"]);
    run_git(&repo, &["config", "user.name", "f08-test"]);
    std::fs::write(repo.join("main.rs"), b"fn main() {}\n").expect("write fixture main.rs");
    run_git(&repo, &["add", "-A"]);
    run_git(&repo, &["commit", "-q", "-m", "init"]);
    run_git(&repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
    run_git(&repo, &["push", "-q", "-u", "origin", "main"]);
    (repo, bare)
}

/// A `gh` shim: records argv, prints a PR URL exactly as `gh pr create` does.
/// It is the ONLY faked component in this test.
fn install_gh_shim(log: &Path) -> PathBuf {
    let bin = unique_dir("bin");
    let shim = bin.join("gh");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> {log}; done\n\
             printf 'https://github.test/acme/repo/pull/4242\\n'\n",
            log = log.display()
        ),
    )
    .expect("write gh shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

/// A `gh` shim that fails its FIRST invocation (exits 0, prints nothing on stdout -- the
/// exact `PR_URL_ABSENT` shape of a transient `gh` hiccup) and succeeds on every invocation
/// after, tracked with a counter file (a real subprocess per call, so there is no in-process
/// state to carry this). Records every call's argv into `log`, same as `install_gh_shim`.
fn install_flaky_gh_shim(log: &Path, counter: &Path) -> PathBuf {
    let bin = unique_dir("bin-flaky");
    let shim = bin.join("gh");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> {log}; done\n\
             n=$(cat {counter} 2>/dev/null || echo 0)\n\
             n=$((n + 1))\n\
             printf '%s' \"$n\" > {counter}\n\
             if [ \"$n\" -eq 1 ]; then exit 0; fi\n\
             printf 'https://github.test/acme/repo/pull/4242\\n'\n",
            log = log.display(),
            counter = counter.display(),
        ),
    )
    .expect("write flaky gh shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

fn field(stdout: &str, prefix: &str, key: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(prefix))?
        .split_whitespace()
        .find_map(|f| f.split_once('=').filter(|(k, _)| *k == key))
        .map(|(_, v)| v.to_string())
}

/// Seeds a REAL attestation through production primitives (append_receipt / write_json_atomic /
/// blake3_hex) inside the probe. `--oracle <status>` is the ONLY thing that varies between the
/// pass and refusal cases below; `--corrupt-artifact` varies only the artifact bytes. One seeder,
/// three outcomes — so the seeder cannot be what is producing the PASS.
fn probe(state: &Path, repo: &Path, gh_bin: &Path, extra: &[&str]) -> std::process::Output {
    let path = format!(
        "{}:{}",
        gh_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(fleet_bin())
        .arg("__pr_emit_probe")
        .arg("--repo")
        .arg(repo)
        .arg("--task")
        .arg("f08-acceptance")
        .arg("--base")
        .arg("main")
        .args(extra)
        .env("FLEET_STATE", state)
        .env("PATH", path)
        .output()
        .expect("failed to run fleet __pr_emit_probe")
}

// ---------------------------------------------------------------- (a) the PR is real

#[test]
fn accepted_task_with_complete_attestation_opens_a_real_pr_with_the_attested_diff() {
    let (repo, bare) = init_fixture_repo();
    let state = unique_dir("state");
    let gh_log = unique_dir("ghlog").join("argv.txt");
    let gh_bin = install_gh_shim(&gh_log);

    let out = probe(&state, &repo, &gh_bin, &["--oracle", "adjudicated"]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "probe exited {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        out.status.code()
    );

    // The lifecycle really advanced, and it advanced to Proposed — not Observed.
    assert_eq!(field(&stdout, "pr_emit: ", "state").as_deref(), Some("Proposed"));
    let head = field(&stdout, "pr_emit: ", "branch").expect("branch field");
    let url = field(&stdout, "pr_emit: ", "pr_url").expect("pr_url field");
    assert!(url.starts_with("https://"), "pr_url must be a real URL, got {url}");

    // The branch really exists on the real remote, with a real commit.
    let refs = Command::new("git")
        .arg("-C").arg(&bare).args(["for-each-ref", "--format=%(refname:short)"])
        .output().expect("for-each-ref");
    let refs = String::from_utf8_lossy(&refs.stdout).into_owned();
    assert!(refs.lines().any(|r| r == head),
        "pushed branch {head} is not on the remote; refs were:\n{refs}");

    // The diff is REAL: the file content on the pushed ref differs from the base.
    let shown = Command::new("git")
        .arg("-C").arg(&bare).args(["show", &format!("{head}:main.rs")])
        .output().expect("git show");
    assert!(shown.status.success(), "pushed ref has no main.rs");
    let base = Command::new("git")
        .arg("-C").arg(&bare).args(["show", "main:main.rs"])
        .output().expect("git show base");
    assert_ne!(shown.stdout, base.stdout,
        "the pushed branch is identical to base -- that is not a real diff");

    // The commit is non-empty and reported honestly.
    let changed: u64 = field(&stdout, "pr_emit: ", "changed_files")
        .expect("changed_files").parse().expect("numeric");
    assert!(changed >= 1, "changed_files must be >= 1, got {changed}");

    // `gh pr create` really ran, with the right head and base.
    let argv = std::fs::read_to_string(&gh_log).expect("gh was never invoked");
    for expected in ["pr", "create", "--head", &head, "--base", "main"] {
        assert!(argv.lines().any(|l| l == expected),
            "gh argv missing {expected:?}; argv was:\n{argv}");
    }
    // The evidence bundle travels with the PR (G8), by file, never interpolated argv.
    assert!(argv.lines().any(|l| l == "--body-file"),
        "PR body must be passed with --body-file, not an interpolated string");

    // And nothing merged it.
    assert!(!argv.lines().any(|l| l == "merge"),
        "author != integrator: the agent must never merge its own PR");

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&bare).ok();
    std::fs::remove_dir_all(&state).ok();
}

// ------------------------------------------- (b) incomplete attestation is refused, by name

#[test]
fn incomplete_attestation_refuses_the_edge_and_names_the_missing_element() {
    let (repo, bare) = init_fixture_repo();
    let state = unique_dir("state");
    let gh_log = unique_dir("ghlog").join("argv.txt");
    let gh_bin = install_gh_shim(&gh_log);

    // The ONLY difference from the passing case: oracle_independence is left as
    // {"status":"pending-adjudication"} -- the real shape run_with_evidence writes
    // (src/main.rs:1658) before adjudicate_command_inner fills it in.
    let out = probe(&state, &repo, &gh_bin, &["--oracle", "pending-adjudication"]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();

    assert!(!out.status.success(), "an incomplete attestation must not exit 0:\n{stdout}");
    assert_eq!(
        field(&stdout, "pr_emit_refused: ", "code").as_deref(),
        Some("INCOMPLETE_ATTESTATION")
    );
    assert_eq!(
        field(&stdout, "pr_emit_refused: ", "missing").as_deref(),
        Some("oracle_independence"),
        "the refusal must name WHICH element is missing"
    );

    // The refusal is real, not cosmetic: no branch was pushed and gh was never called.
    let refs = Command::new("git")
        .arg("-C").arg(&bare).args(["for-each-ref", "--format=%(refname:short)"])
        .output().expect("for-each-ref");
    let refs = String::from_utf8_lossy(&refs.stdout).into_owned();
    assert_eq!(refs.lines().filter(|r| *r != "main").count(), 0,
        "a refused proposal must push nothing; remote refs were:\n{refs}");
    assert!(!gh_log.exists(), "a refused proposal must never invoke gh");

    // And the task did NOT advance.
    let persisted = std::fs::read_to_string(
        state.join("lifecycle").join("f08-acceptance.state")).unwrap_or_default();
    assert_ne!(persisted.trim(), "Proposed");

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&bare).ok();
    std::fs::remove_dir_all(&state).ok();
}

// --------------------------------- control: the seeder is not what makes the pass happen

#[test]
fn corrupted_artifact_bytes_refuse_even_with_a_complete_attestation() {
    let (repo, bare) = init_fixture_repo();
    let state = unique_dir("state");
    let gh_log = unique_dir("ghlog").join("argv.txt");
    let gh_bin = install_gh_shim(&gh_log);

    let out = probe(&state, &repo, &gh_bin,
        &["--oracle", "adjudicated", "--corrupt-artifact"]);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(!out.status.success());
    assert_eq!(
        field(&stdout, "pr_emit_refused: ", "code").as_deref(),
        Some("ARTIFACT_DIGEST_MISMATCH")
    );
    assert!(!gh_log.exists(), "a digest mismatch must never invoke gh");

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&bare).ok();
    std::fs::remove_dir_all(&state).ok();
}

// --------------------------------- `lifecycle advance` must not fake a PR

#[test]
fn lifecycle_advance_refuses_the_accepted_edge_and_points_at_pr_emit() {
    let state = unique_dir("state");
    std::fs::create_dir_all(state.join("lifecycle")).unwrap();
    std::fs::write(state.join("lifecycle").join("f08-advance.state"), "Accepted\n").unwrap();

    let out = Command::new(fleet_bin())
        .args(["lifecycle", "advance", "--task", "f08-advance"])
        .env("FLEET_STATE", &state)
        .output()
        .expect("run fleet lifecycle advance");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    assert_eq!(out.status.code(), Some(7),
        "a refusal must exit 7 (EXIT_REFUSAL), not 3 (EXIT_ENVIRONMENT); stderr:\n{stderr}");
    assert!(stderr.contains("fleet pr emit"),
        "the refusal must name the command that CAN do this; stderr:\n{stderr}");
    let receipts = std::fs::read_to_string(state.join("lifecycle-receipts.jsonl"))
        .expect("a refusal must write a receipt");
    assert!(receipts.contains("Refused"));

    std::fs::remove_dir_all(&state).ok();
}

// --------------------------- retry after a partial failure (push ok, gh not) self-heals

/// Reproduces the exact retry dead-end this task fixes: a `gh` hiccup on attempt 1 (exits 0,
/// prints no URL -- `PR_URL_ABSENT`) leaves the branch already pushed but the task still
/// `Accepted`. Attempt 2 re-applies the identical attested diff onto the identical base --
/// same tree, new commit, same deterministic branch name -- which is exactly the sibling-commit
/// shape that a plain (non-force) push rejects as non-fast-forward. Before the fix, attempt 2
/// failed with `PR_EMIT_FAILED` and the task was wedged until a human ran
/// `git push origin --delete fleet/<task>` by hand; this test asserts attempt 2 instead
/// succeeds on its own, with no manual step in between.
#[test]
fn retry_after_gh_partial_failure_succeeds_without_manual_branch_deletion() {
    let (repo, bare) = init_fixture_repo();
    let state = unique_dir("state");
    let gh_log = unique_dir("ghlog").join("argv.txt");
    let counter = unique_dir("ghcounter").join("n");
    let gh_bin = install_flaky_gh_shim(&gh_log, &counter);

    fn proposal_branches(bare: &Path) -> Vec<String> {
        let refs = Command::new("git")
            .arg("-C").arg(bare).args(["for-each-ref", "--format=%(refname:short)"])
            .output().expect("for-each-ref");
        String::from_utf8_lossy(&refs.stdout)
            .lines()
            .filter(|r| *r != "main")
            .map(str::to_string)
            .collect()
    }

    // Attempt 1: gh exits 0 with no URL. The refusal is real (PR_URL_ABSENT), but the push
    // already happened -- that ordering is deliberate (F08 contract §5, landmine note).
    let first = probe(&state, &repo, &gh_bin, &["--oracle", "adjudicated"]);
    let first_stdout = String::from_utf8_lossy(&first.stdout).into_owned();
    assert!(!first.status.success(), "attempt 1 must refuse:\n{first_stdout}");
    assert_eq!(
        field(&first_stdout, "pr_emit_refused: ", "code").as_deref(),
        Some("PR_URL_ABSENT"),
        "expected PR_URL_ABSENT from the gh-silent first attempt:\n{first_stdout}"
    );

    let branches_after_first = proposal_branches(&bare);
    assert_eq!(
        branches_after_first.len(), 1,
        "exactly one proposal branch must exist after attempt 1; refs were:\n{branches_after_first:?}"
    );

    let persisted_after_first = std::fs::read_to_string(
        state.join("lifecycle").join("f08-acceptance.state")).unwrap_or_default();
    assert_eq!(
        persisted_after_first.trim(), "Accepted",
        "a refused pr emit must leave the task Accepted (not advanced, not corrupted) so a retry is legal"
    );

    // Attempt 2: identical inputs, same still-Accepted task, gh now works. This is the retry a
    // human or the fleet would issue after seeing attempt 1 fail -- no branch deletion, no
    // state edit, nothing manual in between.
    let second = probe(&state, &repo, &gh_bin, &["--oracle", "adjudicated"]);
    let second_stdout = String::from_utf8_lossy(&second.stdout).into_owned();
    let second_stderr = String::from_utf8_lossy(&second.stderr).into_owned();
    assert!(
        second.status.success(),
        "retry must succeed on its own, with no manual branch deletion in between:\n\
         stdout:\n{second_stdout}\nstderr:\n{second_stderr}"
    );
    assert_eq!(field(&second_stdout, "pr_emit: ", "state").as_deref(), Some("Proposed"));
    let url = field(&second_stdout, "pr_emit: ", "pr_url").expect("pr_url field");
    assert!(url.starts_with("https://"), "pr_url must be a real URL, got {url}");

    let persisted_after_second = std::fs::read_to_string(
        state.join("lifecycle").join("f08-acceptance.state")).unwrap_or_default();
    assert_eq!(persisted_after_second.trim(), "Proposed", "the retry must really advance the task");

    // Still exactly one proposal branch -- the retry updated it in place, it did not leak a
    // second dangling ref alongside the first.
    let branches_after_second = proposal_branches(&bare);
    assert_eq!(
        branches_after_second.len(), 1,
        "retry must not leave a second, dangling branch behind; refs were:\n{branches_after_second:?}"
    );
    assert_eq!(
        branches_after_second, branches_after_first,
        "the retry must land on the SAME deterministic branch, not a new one"
    );

    // gh really was invoked twice -- once for each attempt.
    let argv = std::fs::read_to_string(&gh_log).expect("gh was never invoked");
    let create_calls = argv.lines().filter(|l| *l == "create").count();
    assert_eq!(create_calls, 2, "gh pr create must have been invoked exactly twice; argv was:\n{argv}");

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&bare).ok();
    std::fs::remove_dir_all(&state).ok();
}
