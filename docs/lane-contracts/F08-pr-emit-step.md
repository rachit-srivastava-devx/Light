# F08 — PR emit step (lane contract)

**Role:** lead-architect (contract only — no implementation in this lane's contract commit).
**Repo:** `fleet` (`fleet/keel/fleet`, the Rust keel crate).
**Branch:** `lane/F08-pr-emit-step`.
**Status when written:** contract red by construction (T1/T2 — the suite below is the spec, and it is
*supposed* to fail until the builder makes it green).

---

## 1. Restatement

Today a fleet task can be specified, built, independently verified, attested, and human-accepted — and
then it dies on the machine. `ORB-AND-FLEET-DELTA.md` capability C6 says the lifecycle "ends at
`Observed`", and I confirmed that independently: `grep -rn "gh pr\|pull request\|git push"` over
`fleet/keel/fleet/src/` returns **zero hits**. The work product never becomes something a human can
review. F08 closes that: an `Accepted` task must turn into a **real open pull request carrying the
exact diff that was attested**, and the lifecycle must **refuse** to emit that PR if the attestation
backing it is incomplete. The PR is where the agent stops — it never merges its own work.

---

## 2. C1 registry-first verdict — **extract (shape) + build-new (mechanism)**

I checked the sibling project's registry reference at
`Light/fleet/registry-reference/registry/` before designing anything, and then re-ran the negative
check myself rather than trusting the sweep.

**Evidence — what is NOT there.** A `grep -rn "gh pr\|gh api\|git push\|pull request\|--admin\|pr_url"`
across all 93 script/doc/source files of `registry-reference/` returns exactly **one** hit:

```
registry/modules/ops/setup.sh:95:  latest=$(gh api "repos/$repo/releases/latest" --jq .tag_name ...)
```

— a release-version lookup, not PR emission. There is **no installable PR-emit implementation**
anywhere in the reference registry. So "install" is off the table.

**Evidence — what IS there, and is worth extracting.** `registry/services/dispatch/dispatch.sh` has
the *first half* of the shape, and its shape is good:

- `commit_target_work()` (L488–517) stages **only declared/owned paths** (`path_is_owned "$rel"
  "$OWNS_LIST"`), skips caches, and refuses to let fleet-injected policy files become the agent's
  product commit.
- It carries a landing-status enum: `WORK_LANDING` ∈ `no-commit` | `committed` | `commit-failed`.
- The precondition guard (L977) is a **three-part** gate before anything lands:
  `[ "$RAW_EC" -eq 0 ] && [ "$VIOLATION_COUNT" -eq 0 ] && [ "$TARGET_READY" -eq 1 ]`.
- It emits **structured landing metadata** into the run record:
  `work_landed:{branch:…,commit:…,status:…,changed_files:…}` (L1052–1057).

And the policy that governs the missing half is stated literally at
`registry/modules/agents/lead-architect.md:16`:

> `Contracts/migrations/money = human-merge always (A15/D4) — ship to a PR and stop.`

**Verdict.** *Extract* the shape — precondition-gate → scoped commit → structured landing record →
stop-at-PR — and *build-new* the mechanism, because (a) the mechanism does not exist in any form to
install, and (b) the reference is Bash driving an untyped shell flow while this kernel is Rust with a
compile-enforced typestate. Porting the shell would throw away the one property this kernel has that
the reference does not: **an illegal transition here fails to compile.** The port is therefore
shape-only, as D1 (`ORB-AND-FLEET-DELTA.md` §3) intends.

I also extracted at the *function* level inside this repo, which matters more than the cross-repo
port: the completeness gate does **not** reimplement attestation checking. It calls the existing
`attest_verify_inner` (`src/main.rs:3931`). See §5.

---

## 3. Killed alternatives

### 3.1 KILLED — shell out to `gh pr create` from `main.rs`, no lifecycle edge

The cheapest option: add a `fleet pr emit` command that runs `git push` + `gh pr create` and prints a
URL. Roughly 40 lines. Rejected on three independent grounds:

1. **It proves nothing about attestation.** A bare shell-out is reachable from *any* state. Nothing
   stops it running on a task that was never verified. FEATURES.md's acceptance is not "a PR gets
   opened", it is "the lifecycle machine **refuses the edge** if attestation is incomplete." A
   refusal that lives in an `if` inside one CLI handler is bypassed by the next caller that forgets
   it; a refusal that lives in a consumed-`self` typestate edge cannot be bypassed at all, because
   the compiler will not produce a binary that skips it. This kernel already made that choice
   everywhere else (`tests/compile_fail/intake_to_verified.rs` exists precisely to pin it) and F08
   must not be the one edge that opts out.
2. **It cannot express "author != integrator."** `FLEET-LEARNINGS.md` §"SDLC gate model" states the
   structural rule: *"an agent that opens a PR must never self-approve/self-merge"*, and *"A5
   (migrations/money/auth) is never autonomous — human-merge always."* A shell-out has no notion of a
   terminal agent-reachable state, so nothing in the type system says where the agent must stop. With
   a typed edge, "the agent stops at the PR" becomes a *compile-time* fact (§4.4).
3. **Exit 0 is not a pull request.** A shell-out that treats `gh`'s exit status as success is the
   exact proxy failure this estate has paid for repeatedly — an API 200 is not a rendered page, and a
   zero exit is not an open PR. The typed edge forces the URL to be a *value that must exist* before
   the transition can be constructed (§4.5, `PR_URL_ABSENT`).

### 3.2 KILLED — auto-merge (or self-approve) once the gates are green

Tempting, and the machine is arguably *better* qualified than a tired human at 2am: the diff is
blake3-pinned, independently verified by a distinct verifier, mutation-adequacy measured, blast radius
enumerated, rollback recorded. Why stop at an open PR?

Rejected — and it is worth arguing rather than asserting, because the estate's law is the *conclusion*
of an argument, not an arbitrary rule:

- **The gates certify the change, not the decision to ship it.** Every element of the attestation is a
  statement about the diff's internal properties. None of them is a statement about whether this
  change should exist — product fit, timing, a migration that must not run during business hours, a
  contract another team depends on. That judgment has no oracle in the bundle, so the machine is not
  *under-trusted* here, it is *unqualified*, and adding a `Merged` state would silently claim
  otherwise.
- **A self-approving author collapses the only independent check that survives.** The whole evidence
  chain rests on `independent_verification.distinct == true` — builder ≠ verifier. If the same
  automated chain also merges, the last human in the loop is gone, and every downstream property is
  something the system asserted about itself. That is precisely the failure `author != integrator`
  encodes.
- **The estate's own law already settled it:** A15/D4, *"Contracts/migrations/money = human-merge
  always — ship to a PR and stop"*, and autonomy-ladder A5, *"never autonomous."*

**How the kill is enforced, not just documented:** the lifecycle gets **no `Merged` state and no
`merge` method, deliberately.** `Task<Proposed>` is the terminal state an agent can reach. Merging is
not a lifecycle edge and must never become one — it happens on the forge, by a human, under branch
protection (G1/G2). A `trybuild` compile-fail case (§6.3) pins this so that a future agent adding
`Task<Proposed>::merge` breaks the build with a recorded expectation, rather than quietly shipping it.

### 3.3 KILLED — make PR emission an *optional* branch off `Accepted`

i.e. `Accepted → {Proposed, Observed}`, so tasks with no diff can skip it. Rejected: an optional edge
is a bypass, and the bypass *is* the current defect. If `Accepted → Observed` survives, an agent
reaches `Observed` without ever emitting a PR and F08 is satisfied on paper while the lifecycle still
"ends at Observed" in practice. `Proposed` is therefore **inserted into the canonical chain**
(`Accepted → Proposed → Observed`) and `Task<Accepted>::observe` is **deleted**. A task with nothing
to propose does not skip the edge — it takes `Accepted → Refused` with `EMPTY_DIFF`, which is already
this codebase's stated position (`src/main.rs:1511`: *"A run that changes nothing must not produce an
attested artifact."*).

---

## 4. The new lifecycle edge

### 4.1 New state

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Proposed;
```

Added to `seal_states!(...)` alongside the others (`src/lifecycle.rs:80`).

**Name rationale:** the change is *not* delivered — it is proposed for human integration. Every
existing state is a past participle (`Attested`, `Accepted`, `Observed`); `Proposed` fits the
vocabulary and states the human-merge boundary in the name itself. Do not call it `Merged`,
`Landed`, or `Delivered` — each of those asserts something the agent did not do.

### 4.2 Graph changes (all four copies — see §8.1)

| Location | Change |
|---|---|
| `src/lifecycle.rs:15` `STATES` | `("Accepted", &["Proposed", "Refused"])`; new row `("Proposed", &["Observed"])`; `Observed` row unchanged |
| `src/lifecycle.rs:253` | `edge!(Accepted, observe, Observed)` → **`edge!(Proposed, observe, Observed)`** (the old one is deleted, not kept) |
| `src/lifecycle.rs:270-273` | add `refusal_edge!(Accepted)` |
| `src/lifecycle.rs:503` `canonical_next` | `"Accepted" => Some("Proposed")`, `"Proposed" => Some("Observed")` |
| `src/lifecycle.rs:537` `typed_advance` | `"Accepted"` arm returns the refusal in §4.6; new `"Proposed"` arm calls `observe` |
| `src/swarm.rs:11` `STATES` | insert `"Proposed"` between `"Accepted"` and `"Observed"` |
| `src/repl.rs:454` | the meter help string gains `-> Proposed` before `-> Observed` |

`src/sow.rs:105` `ReviewStatus::Accepted` is a **different enum** — do not touch it.

### 4.3 The edge signature

It matches the established pattern exactly: consumes `self`, takes evidence, takes the ledger, returns
`Result<Task<Proposed>, Refusal>`. It additionally takes the two things the transition is *about* — the
attestation bundle it must gate on, and the emitter that performs the side effect.

```rust
impl Task<Accepted> {
    /// `Accepted -> Proposed`: emit a real pull request carrying the attested diff.
    ///
    /// This is the agent's terminal act. There is no `merge` edge and no `Merged` state:
    /// author != integrator (FLEET-LEARNINGS.md, SDLC gate model). Merging is a human
    /// action on the forge, under branch protection — never a lifecycle transition.
    pub fn propose(
        self,
        bundle: &AttestationBundle,
        request: &ProposalRequest,
        emitter: &impl ChangeEmitter,
        ledger: &impl ReceiptLedger,
    ) -> Result<(Task<Proposed>, ProposedChange), Refusal> { ... }
}
```

**Why two injected traits and not direct I/O:** `src/lifecycle.rs` is in the **lib** crate
(`src/lib.rs`: `pub mod agent; pub mod lifecycle; pub mod skills;`), while `main.rs` is a **bin** that
does `use fleet::lifecycle;` (`main.rs:14`, `:53`). The dependency runs bin → lib and cannot be
inverted, so `lifecycle.rs` **cannot call `attest_verify_inner`** and must not shell out to `git`/`gh`
itself. This is not a workaround — it is the pattern this module already uses: `ReceiptLedger`
(`:119`) exists for exactly this reason, and the unit tests supply `MemoryLedger` (`:823`) as the
double. `ChangeEmitter` and `AttestationBundle` mirror that shape.

```rust
/// The only capability the propose edge needs from the outside world.
pub trait ChangeEmitter {
    /// Push `head` and open a pull request. MUST return the real PR URL — an
    /// emitter that "succeeded" without one must return Err, never Ok.
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, Refusal>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalRequest {
    pub repo: PathBuf,
    pub base: String,          // e.g. "main"
    pub head: String,          // e.g. "fleet/<task>" — matches worktree.rs:57
    pub artifact_id: String,   // 64 lowercase hex, blake3 of the diff bytes
    pub diff: Vec<u8>,         // the attested bytes, read from $FLEET_STATE/artifacts/<id>
    pub title: String,
    pub body: String,          // evidence bundle, see §4.7
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedChange {
    pub url: String,           // the real PR URL
    pub head: String,
    pub commit: String,        // sha of the pushed commit
    pub changed_files: u64,
}
```

`ProposedChange` is the extracted `work_landed:{branch,commit,status,changed_files}` record from
`dispatch.sh:1052-1057`, typed.

### 4.4 What triggers it

**Not** `fleet lifecycle advance`. That command (`src/lifecycle.rs:717`) carries only a `TaskId` and a
ledger — it has no artifact, no repo, no base branch, and therefore *cannot* emit a real PR. Rather
than widen it (which would let it emit a fake one), `typed_advance("Accepted")` **refuses** with a
message naming the right command. See §4.6.

The trigger is a new command:

```
fleet pr emit --task <ID> --artifact <ARTIFACT_ID> --repo <PATH> --base <BRANCH> [--head <BRANCH>]
```

dispatching (in `main.rs`, alongside `Some("attest") ...` at `:136`) to a function that:

1. loads persisted state; **refuses unless it is exactly `Accepted`**;
2. runs `attest_verify_inner(artifact_id, false)` — the existing, real, full check;
3. reads `$FLEET_STATE/artifacts/<artifact_id>`, re-derives `blake3_hex`, refuses on mismatch;
4. builds `AttestationBundle` from `$FLEET_STATE/attestations/<artifact_id>.json`;
5. calls `lifecycle::propose_change(...)` (the lib-side driver, which mints the internal token and
   runs the typed edge, exactly as `drive_run` does at `:747`);
6. `persist_state(root, &id, "Proposed")`.

A hidden `__pr_emit_probe` sibling (matching `__lanes_probe`, `main.rs:161`) calls **the same
production function** so the acceptance test drives real code, not a test-only path.

### 4.5 What makes it refuse

| Refusal code | Condition |
|---|---|
| `NOT_ACCEPTED` | persisted state ≠ `Accepted` |
| `INCOMPLETE_ATTESTATION` | any required element missing or failing — **message names the element** |
| `ARTIFACT_DIGEST_MISMATCH` | `blake3_hex(bytes) != artifact_id` |
| `EMPTY_DIFF` | artifact is zero bytes (carries forward `main.rs:1511`) |
| `EMPTY_TRANSITION_EVIDENCE` | inherited from `transition` (`:300`) |
| `PR_EMIT_FAILED` | push or `gh` failed |
| `PR_URL_ABSENT` | **`gh` exited 0 but printed no URL** — exit 0 is not a pull request |

**The elements that must be present — exactly the 8 the current code enforces**, read off
`attest_verify_inner` (`main.rs:4062-4074`), *not* the aspirational 9:

```
sow · blind_suite · independent_verification · adequacy · blast_radius · rollback · cost · oracle_independence
```

and, carried forward because presence alone is not the property `attest_verify_inner` actually
enforces:

- `independent_verification`: 5 keys, `distinct == true`, `reproduced == true`, `verdict == "ACCEPT"`
  (`main.rs:4050-4059`);
- `oracle_independence`: 6 keys, `o1_hash`/`o2_hash` valid artifact ids, `distinct == true`,
  `quadrant ∈ {ACCEPT, ORACLE_INADEQUATE, ORACLE_OVERCONSTRAINED, BUILDER_FAULT}`
  (`main.rs:3992-4017`);
- `blind_suite`: 5 keys incl. `suite_hash` (`main.rs:4025-4043`).

> **Note for the builder — this is the highest-value negative case.** `run_with_evidence` writes
> `"oracle_independence":{"status":"pending-adjudication"}` (`main.rs:1658`) and only
> `adjudicate_command_inner` fills it in later. So *an attestation that has not been adjudicated is a
> real, routinely-produced, incomplete attestation.* Use it as the §6.2 refusal case rather than
> inventing a synthetic hole.

`AttestationBundle::missing_element(&self) -> Option<&'static str>` lives in `lifecycle.rs` so the
naming logic is unit-testable in the lib without any filesystem.

### 4.6 The `lifecycle advance` refusal

`typed_advance("Accepted", ...)` returns:

```rust
Err(Refusal::new(
    "PR_EMIT_REQUIRES_EVIDENCE",
    "Accepted -> Proposed emits a real pull request and needs an artifact and a repo; \
     drive it with `fleet pr emit --task <ID> --artifact <ID> --repo <PATH> --base <BRANCH>`",
))
```

**Required observable behaviour** (the builder picks the wiring; these are the assertions):
`fleet lifecycle advance --task <accepted-task>` exits **7** (`EXIT_REFUSAL`, not `EXIT_ENVIRONMENT`),
writes a refusal receipt to `lifecycle-receipts.jsonl`, and prints a stderr line containing
`fleet pr emit`. Note `advance_to` (`:651`) currently routes `typed_advance` errors through
`report_environment` → exit 3; that path must be corrected for this code, or the case handled before
`typed_advance` is reached.

### 4.7 PR body — the evidence bundle (G8)

The body embeds the attestation so the human reviewing the PR sees what the machine actually proved:
`artifact_id` (blake3), builder id + resolved model, verifier id, `verdict`, `reproduced`, adequacy
`checked/total`, `blast_radius.count` + file list, `rollback`, `cost.wall_ms`, and the
`oracle_independence.quadrant`. Assembled as a string and passed via `--body-file` (never as an
interpolated argv string — the task text is untrusted and can contain anything;
`dispatch.sh:build_prompt` makes the same point: *"Stdin only — never interpolated."*).

The body must also carry an explicit line stating that the PR is **not to be self-merged**.

---

## 5. Reuse inside this repo (do not reimplement)

| Need | Use | Do NOT |
|---|---|---|
| attestation validity | `attest_verify_inner` (`main.rs:3931`) | write a second JSON validator |
| artifact digest | `blake3_hex` (`main.rs:4288`), `valid_artifact_id` (`:1997`) | hand-roll hex checks |
| branch creation | `worktree::create` (`worktree.rs:52`) — already `git worktree add -b fleet/<name>` | invent a branch scheme |
| receipts | `append_receipt` (`main.rs:3663`) | write ledger rows directly |
| atomic JSON write | `write_json_atomic` (`main.rs:2724`) | `fs::write` |
| ledger read/verify | `ledger_rows` (`:3638`), `verify_rows` (`:3749`) | re-parse the chain |

> **Landmine — `worktree::remove` deletes the branch.** `worktree.rs:145-153` runs
> `git branch -D <branch>` as part of cleanup. If a lane's worktree is removed after the PR is opened,
> the local head branch vanishes. **Push to the remote before any cleanup** — the remote ref survives
> the local delete. Order is: apply → commit → **push** → `gh pr create` → only then allow cleanup.

---

## 6. Acceptance suite (write these first; they are the spec)

Three files. The builder may **not** edit the assertions — only make them pass.

### 6.1 + 6.2 Integration test — `fleet/keel/fleet/tests/f08_pr_emit.rs`

Follows `tests/s3_lanes.rs` exactly: drives the **real compiled binary** via
`env!("CARGO_BIN_EXE_fleet")`, against a **real fixture git repo**, parsing structured `k=v` stdout.

**The test seam — what is real and what is faked.** This is the part to get right:

- **Real:** the git repo, the branch, `git apply` of the attested diff, the commit, and the **push** —
  because `origin` is a **real local bare repo** (`git init --bare`). The pushed ref is really there
  and the test reads the file content back out of it. Real too: the attestation, its receipts
  (written through the production `append_receipt`), and the blake3 digest.
- **Faked:** only the GitHub API. A `gh` shim is written into a temp dir prepended to `PATH`; it
  appends its own argv to a log and prints a URL on stdout, which is what real `gh pr create` does.
- **Therefore not proven by this test, and stated honestly:** that the live GitHub API accepts the
  call. Everything up to and including the pushed branch and the exact argv handed to `gh` is real.

```rust
//! F08 acceptance: an Accepted task becomes a real open PR carrying the attested diff,
//! and the lifecycle refuses the edge when the attestation is incomplete.
//!
//! Cargo INTEGRATION test — drives the real `fleet` binary (see tests/s3_lanes.rs for why a
//! unit test cannot exercise this path). `origin` is a real local bare repo, so the push is
//! real; only `gh` is shimmed, because the test must not require live GitHub credentials.

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

fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fleet-f08-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

/// A real work repo with a real local bare `origin`. Nothing here is stubbed.
fn init_fixture_repo() -> (PathBuf, PathBuf) {
    let bare = unique_dir("origin");
    run_git(&bare, &["init", "--bare", "-q"]);
    let repo = unique_dir("repo");
    run_git(&repo, &["init", "-q", "-b", "main"]);
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
```

### 6.3 Compile-fail cases — `fleet/keel/fleet/tests/compile_fail/`

Picked up automatically by the existing `tests/compile_fail.rs` glob. Generate the `.stderr` files with
`TRYBUILD=overwrite cargo test --test compile_fail`, then **read them** before committing.

`accepted_skips_pr.rs` — the bypass must not compile. This is the single strongest statement of F08:

```rust
// Accepted -> Observed must no longer exist. If this compiles, the PR-emit step is
// optional, and "the lifecycle ends at Observed" (ORB-AND-FLEET-DELTA C6) is still true.
use fleet::lifecycle::{ReceiptLedger, Refusal, Task, TaskId, TransitionReceipt};

fn main() {
    let task: Task<fleet::lifecycle::Accepted> = unreachable!();
    let _ = task.observe("skipped the pull request", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), Refusal> {
        Ok(())
    }
}
```

`merge_is_not_a_lifecycle_edge.rs` — author != integrator, as a compile error:

```rust
// There is deliberately no `Merged` state and no `merge` edge: an agent that opens a PR
// must never self-approve or self-merge (FLEET-LEARNINGS.md, SDLC gate model; A15/D4).
// If someone adds one, this test breaks and they must justify it in review.
use fleet::lifecycle::{Proposed, Task};

fn main() {
    let task: Task<Proposed> = unreachable!();
    let _ = task.merge();
}
```

### 6.4 Unit tests — in `src/lifecycle.rs mod tests` (existing `MemoryLedger` style)

1. `missing_element` returns `Some("oracle_independence")` for a `pending-adjudication` bundle,
   `Some("adequacy")` when adequacy is absent, `None` when complete.
2. A complete bundle + a recording in-memory `ChangeEmitter` advances `Accepted → Proposed` and
   appends exactly one receipt.
3. An incomplete bundle refuses **and the emitter is never called** (assert the recorder is empty) —
   the gate must run *before* the side effect, not after.
4. `pinned_element_names_match_the_enforced_set` — asserts the required-element list is exactly the 8
   names, with a comment pointing at `main.rs:4062-4074`. When the 9th element lands, this test fails
   and forces a conscious update instead of silent drift.
5. Update the existing `legal_path_consumes_each_state_and_records_every_edge` (`:832`): insert
   `.propose(...)` into the walk and change `assert_eq!(ledger.0.borrow().len(), 14)` to `15`.
   **Do not delete or weaken that assertion** — it is the count that proves every edge recorded.

---

## 7. Done-definition

- [ ] `cargo test -p fleet` green, including all four `compile_fail` pre-existing cases plus the two new ones.
- [ ] `f08_pr_emit.rs`: 4/4 pass, assertions unmodified.
- [ ] `fleet pr emit` works end-to-end against the fixture; `fleet lifecycle advance` on an `Accepted` task exits 7.
- [ ] Evidence file written with the real command output pasted (VERIFICATION doctrine) — verifier will re-derive independently.
- [ ] A dated entry appended to `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md` (append only — never delete another agent's entry).
- [ ] **Ships to an open PR and stops.** This lane touches a contract and a lifecycle machine: A15/D4 — human-merge always.

---

## 8. Assumptions, and what this lane deliberately does NOT do

### 8.1 Flagged, not fixed
The lifecycle state list now exists in **four** places — `lifecycle.rs:15` (`STATES`),
`lifecycle.rs:503` (`canonical_next`), `swarm.rs:11` (`STATES`), and a prose string at
`repl.rs:454`. `advance_to` cross-checks only the first two ("typed and runtime lifecycle graphs
disagree", `:657`); `swarm.rs` and `repl.rs` can drift silently. F08 must update all four, but
**collapsing them to one source of truth is out of scope** — it is a refactor of shared state
machinery that would collide with other lanes. Flagging it here for a follow-up.

### 8.2 Assumptions
- `gh` is on `PATH` and authenticated **in production**; the acceptance test never relies on this
  (§6.1) and `gh` was confirmed present at `/opt/homebrew/bin/gh` on this machine.
- The repo has an `origin` remote. If absent, `PR_EMIT_FAILED` — do not silently skip the push.
- Base branch is supplied by the caller (`--base`), not guessed.
- The attested artifact at `$FLEET_STATE/artifacts/<id>` is a git-applicable diff — this is what
  `run_with_evidence` writes, and the attestation subject is literally named `git-diff`
  (`main.rs:1640`).
- The 9-element attestation is aspirational; **8 is what the code enforces today** and 8 is what this
  edge requires. Closing that gap is not F08's job.

### 8.3 Explicitly out of scope
Model routing (F13) · worktree-lane parallelism (F24) · Orb↔Fleet handoff (F09) · SOW intake (F07) ·
the `lld-ready` gate (F06) · retro/learning loops (F38/F39) · any change to `sow.rs`'s unrelated
`ReviewStatus::Accepted` · a `Merged` state (§3.2 — deliberately never).

### 8.4 Landmines already paid for — do not rediscover
- **Never `git stash`** anywhere in this tree: `refs/stash` is one shared stack across every worktree
  of the repo, and other lanes are live right now. Use `git worktree add --detach` or
  `git diff > x.patch`.
- Whole-repo `cargo test` is slow under concurrent agent load — that is documented resource
  contention, not necessarily your regression. Isolate with `cargo test -p fleet --test f08_pr_emit`
  first before concluding anything.
- Never background a slow command and wait for a notification — a subagent will never receive one.
  Foreground, or `timeout N <cmd>`.
- `worktree::remove` runs `git branch -D`. **Push before cleanup** (§5).
