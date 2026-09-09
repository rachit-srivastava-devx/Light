# HANDOFF — 2026-09-09, mid-session

Written because the session was about to run out. **Everything below is uncommitted work in
progress on branch `master`.** Last pushed commit: `00ce9fc` on `main`. Read this top to bottom
before touching anything; the last section says exactly what to do next.

---

## 0. STATUS UPDATE — everything below section 1 is now COMMITTED AND PUSHED

Section 1's warning is obsolete. The work landed on `main` at **`ff125e9`**, and the suite was
verified green afterwards, without any `FLEET_LOAD_FACTOR` override:

```
cargo test --workspace --no-fail-fast   -> 505 passed / 0 failed, exit 0
cargo clippy --workspace --all-targets -D warnings -> exit 0
files over 80 lines                     -> 0 of 645
selfcheck.sh --all                      -> 825 files, 0 FAIL, 1 advisory warn
corpus/_selftest.sh                     -> 25 of 25 detectors proven
fleet gate --id detectors               -> PASS 111/111
shasum -c MANIFEST.sha256               -> exit 0
```

So **skip steps 1, 2 and 4 of section 5.** The one genuinely open item is **step 3**: drive
`fleet run` with a task that produces a real change, so the merge stage is exercised on its
success path rather than only on the honest-refusal path. Section 4's known-open list still
stands as written.

---

## 0b. THE BIGGEST OPEN FINDING: `merge_lane` has zero production callers

Investigated because section 5 step 3 (drive the merge stage's success path) was the last
unexercised node. The answer is that **the CLI cannot reach it at all.**

`fleet_merge::merge_lane` is fully implemented and genuinely works — proven end to end by a
harness that depends on the real, unmodified crate: real merge commit `5ceedab` landed on
`master` in a scratch repo, containing the worker's actual edit (`git show --stat` → `src/lib.rs |
4 ++++`), lane worktree cleaned up, `git status` clean afterwards, `cargo test` still green.
The function is not the problem.

The wiring is. Verified by hand, not taken from a subagent's report:

```
grep -rn "merge_lane" src/ | grep -v "^src/tests/"     -> no matches
grep -rn "merge_lane" crates/*/src/                    -> only fleet-merge's own definition/docs
```

- `src/pipeline/merge_stage.rs:21` — the `Merge` stage calls only
  `fleet_merge::check_stage_nonempty(staged_files, branch)` against whatever happens to be staged
  in `--repo`. It never calls `merge_lane`, never receives a worktree, never runs `git merge`.
- `src/pipeline/ctx.rs` — `StageCtx` has no worktree/branch field, so there is no channel through
  which a worker's lane could reach the merge stage.
- `src/pipeline/stages_dispatch.rs` — the `Dispatch` stage runs the router decision, clears the
  lifecycle gate, and pushes the task through an in-process channel pair and back. **It never
  spawns a worker.** So in the `fleet run` path no worktree and no code change exist to merge.
- `crates/fleet-worker/src/freelane/apply/mod.rs:26` — `apply(worktree, reply)` writes the
  worker's edits into the **worktree**, never the repo.
- `crates/fleet-worker/src/spawn/prepare.rs:19` + `reap_sweep.rs:29` — the worker path creates
  and removes worktrees, and nothing between those two calls merges the lane branch.

**Consequence, stated plainly:** `fleet swarm` spawns a worker, the worker makes a real code
change in an isolated worktree, the change-honesty check confirms a change happened, the worktree
is torn down — and the change is discarded. Work is performed and then thrown away. `fleet run`'s
`Merge` stage is a staged-file precondition check, not a merge.

**This is a feature to complete, not a defect to patch, and it was deliberately NOT built
unattended.** Wiring it means: Dispatch spawns a real worker, the worktree+branch thread through
`StageCtx`, and Merge calls `merge_lane`. That is a pipeline-contract change, which
`CLAUDE.md` (A15/D4) makes human-merge always — and getting it wrong writes to someone's real
branch. It needs the owner's sign-off on the contract before code.

---

## 1. What was uncommitted when this file was first written (now landed — see section 0)

Run `git status --short` first. Expected: modifications to `src/dispatch/verify_ports.rs`,
`src/dispatch/verify_ports_tests.rs` (new), `src/print/{summary,run_report,verify_report}.rs`,
`src/print/summary_tests.rs`, `src/pipeline/graph.rs`, `src/tests/teach_persists_lesson.rs`,
`src/tests/install_marker_matches_binary.rs` (new), `install.sh`,
`crates/fleet-verify/src/{classify,denominator,parsers}.rs`,
`crates/fleet-verify/gates/recur-gate.sh`, `crates/fleet-verify/gates/corpus/run.sh`,
`crates/fleet-verify/gates/corpus/MANIFEST.sha256`,
`crates/fleet-verify/tests/{denominators_go_to_stdout,recur_publishes_denominator}.rs` (new).

**A full `cargo test --workspace --no-fail-fast` was running in the background when the session
ended and its result was never seen.** Task id `bi5q3xutk`, output at
`/private/tmp/claude-501/-Users-rachitsrivastava-youtube-Principal-Engineering-Light/2d06eda9-9da6-4f2e-8701-fa7204979c11/tasks/bi5q3xutk.output`.
That file may be stale or gone — **do not trust it, re-run the suite yourself.** Nothing here is
committed, so a red suite costs you nothing but a fix.

Last known-green full suite was **500 passed / 0 failed** at commit `00ce9fc`, before the changes
in section 2.

---

## 2. The defect chain fixed this session (six defects, four of them hiding each other)

The trigger was one user-visible symptom: `fleet run --repo <any scratch repo> --task noop`
exited 7 with seven of eight gates reporting timeout 124. Peeling that apart found a chain.

### 2.1 Shared verify deadline starved every gate but the first — `src/dispatch/verify_ports.rs`

`RealRunner` held ONE `Instant` deadline for the whole invocation, default **8 seconds**. Gate #1
(`cargo test --workspace`) cannot finish in 8s on a cold repo, so it consumed the entire budget
and all seven remaining gates got an instant `NonZeroExit(124)`. The gate table's ORDER decided
who got any time at all, and the report blamed the starved gates rather than the greedy one.

Fix: total budget default 8s → **300s**, plus a **per-gate ceiling** of `total/2`
(`per_gate_budget`). Each gate's deadline is now `min(now + per_gate, overall)`.

Evidence: same command, per-gate budget printed as `149.99s` for every gate, 6 of 8 gates
genuinely PASS, whole run 16s. Tests in `verify_ports_tests.rs`, including one asserting short
test budgets still leave a spawnable slice (a zero slice would make the timeout tests green for
the wrong reason).

### 2.2 `recur-gate.sh` printed its denominator to **stderr** — never published a count, ever

`parsers::recur` reads stdout only. The gate's `printf ... > "/dev/stderr"` meant it returned
`Unparseable` on **every run since it was written**. It was invisible because 2.1 starved it into
a 124 first. Every peer gate (semgrep, trivy, detector-integrity, policy) prints to stdout; this
was the lone outlier.

Fix: print to stdout. Pinned by **two** new tests in `crates/fleet-verify/tests/`:
- `denominators_go_to_stdout.rs` — static scan, no gate script may send a denominator-bearing
  line to `/dev/stderr`; publishes its own denominator and refuses to pass on an empty scan set.
- `recur_publishes_denominator.rs` — actually runs the gate and asserts the registry's parser
  reads its stdout, for both an empty repo and a repo with a commit.

### 2.3 Two more `recur` paths published nothing — found *by* the test in 2.2

- **No commits yet**: prose-only early exit, `Unparseable`.
- **Clean working tree**: empty diff → `checked=0` → `MeasuredNothing` → **fail**.

The second is the important one: it meant `fleet run` failed on *any unmodified repo*. Passing
would have been the empty-input green `fleet-verify` exists to forbid; failing was wrong too,
because a clean tree is not a defect.

Fix: a third state. New `DenominatorResult::NotApplicable`, mapped by `classify` to a **visible**
`Verdict::Skip { reason, was_required }`. A gate reaches it only by printing its own explicit
`recur-gate: not-applicable` marker, so it cannot be used to dodge `MeasuredNothing` by staying
silent. `report.rs` already counts skips separately and flags a *required* gate that skipped, so
this cannot vanish from a summary.

### 2.4 M-series corpus detectors failed on every repo that is not fleet — `corpus/run.sh`

The M-series asserts invariants about **fleet's own source tree** (`mutants_probe.rs` still holds
the `FLEET_MUTANTS` guard, `install.sh` still displaces a foreign binary, ...). After the earlier
S1 retarget pointed every detector at `$FLEET_TARGET_REPO`, running them against a *user's* repo
asked whether the user's repo contains fleet's files. It does not, so they exited 1 "caught" and
`fleet run --repo <user repo>` failed with two findings that were category errors.

Fix: `run.sh` gates `M[0-9]*.sh` on `is_fleet_tree()` (presence of
`crates/fleet-verify/src/registry.rs` in the target). Not applicable → exit-77-equivalent,
counted AND printed (`NOT-APPLICABLE M1.sh -- ...`), plus a new
`fleet_own_not_applicable=N` field on the DENOMINATOR line.

**The fleet-ness test is deliberately narrow.** Inside fleet's own tree a missing anchor file must
still be CAUGHT, not excused — that is the M4 defect (a detector that skipped itself when its own
source moved, masking real breakage) and it must not come back. Verified both directions:
- foreign repo → `checked=27 excluded=82 caught=0 fleet_own_not_applicable=13`, exit 0
- fleet's own tree → `checked=30 caught=2 fleet_own_not_applicable=0`, exit 1

`MANIFEST.sha256` line 111 regenerated for `run.sh`
(`8b3e4fb8aa8e3c054ffae48099c4db5ca6a3d5b2d3b38f87def7da977314bfb8`). `shasum -c` exit 0,
`_selftest.sh` → `25 of 25 detectors proven`, `fleet gate --id detectors` → PASS 111/111.

### 2.5 `graph.rs` overwrote the failing stage with `Teach` — pointed users at the wrong stage

`final_stage = PipelineStage::Teach;` ran unconditionally, destroying the only record of where a
failure happened. A run that died in `verify` told the user
`next: investigate stage 'teach'` — naming a stage that printed no output and failed nothing.

Fix: only set `final_stage = Teach` when `result.is_ok()`. Teach is an always-runs epilogue, not
where the run got to.

`src/tests/teach_persists_lesson.rs` **was asserting the bug** (`"final_stage": "Teach"` on a run
that failed at merge, commented "must reach Teach"). Its real property is the next assertion — the
lesson names the failing stage. Changed to assert `"Merge"`.

### 2.6 The summary said "gates" while counting stages — `src/print/summary.rs`

`fleet run` printed `gates 1 attempted -- 0 passed, 1 failed` directly underneath six passing
gates. The numbers were true for *stages*; the label named a different unit. This is the same
unit conflation the module's own header warns about for gates-vs-checks, in the line doing the
warning.

Fix: `Summary` gained a required `unit: &'static str` field (required, not defaulted — the
compiler then found the other call site for me). `fleet run` passes `"stages"` with real counts
derived from `PipelineStage::ALL`; `fleet gate`/`oracle` pass `"gates"`.

### 2.7 `install.sh` classified fleet's own binary as foreign — committed in `00ce9fc`

`foreign_bin()` greped `--help` for `'frozen, attested change'`, a string present **nowhere in
this repo**, so every re-install treated fleet's own prior install as foreign and displaced it.
Now greps `version` for `^fleet-cli:`, pinned by `src/tests/install_marker_matches_binary.rs`
which reads the pattern out of `install.sh` itself and asserts the real binary prints it. Proven
both directions: own binary → `WOULD install`, shell script → `WOULD displace`.

---

## 3. Current end-to-end state of `fleet run`

```
NO_COLOR=1 FLEET_LOAD_FACTOR=10000 FLEET_STATE_DIR=<tmp> \
  fleet run --repo <clean scratch git repo> --task "noop"
```

Before this session: **exit 7**, seven gates at timeout 124, `next:` naming the wrong stage.

Now: **verify stage PASSES** —
```
PASS gate unit tests 1/1
SKIP gate mutants -- mutants skipped: set FLEET_MUTANTS=1 to run it
PASS gate semgrep 29/29
PASS gate trivy 1/1
SKIP gate recur -- recur: no applicable input in this repo
PASS gate detectors 111/111
PASS gate policy 3/3
PASS gate corpus 27/27
PASS stage verify (12.94s)
▶ stage merge
FAIL stage merge (0.04s)
REFUSED merge: Merge(EmptyStage { branch: "master" })
stages   7 attempted -- 6 passed, 1 failed, 0 skipped
```
Still exit 7, but now for the **correct** reason: `--task noop` produced no change, and the
change-honesty invariant refuses to report success for work that did not happen.

**This has NOT been driven with a task that produces a real change.** That is the single most
valuable next verification (see section 5).

`git status --porcelain` in the scratch repo was identical before and after (2 entries both
times — `Cargo.lock` and `target/`, both created by the `cargo test` gate legitimately running
there, not by fleet writing state).

---

## 4. Known-open, honest

- **M2 is a true positive and still fails corpus inside fleet's own tree**: 413,281 files in the
  tree because `CARGO_TARGET_DIR` is inside the repo. Owner action: set it outside. This is the
  gate working correctly, not a bug to suppress.
- **M11 prints `bin/freelane.sh missing or not executable`** but does not exit 1. `freelane.sh`
  moved to `crates/fleet-worker/assets/freelane.sh` during cleanup; M11 still names the old path.
  Retarget it, and check whether its non-1 exit is deliberate or an accident.
- `Merge(EmptyStage { branch: "master" })` is raw `Debug`, not a human sentence. DX gap.
- Teach runs but prints no `▶ stage teach` line, so its execution is invisible in the stream.
- `recur-gate.sh` uses `/tmp/recur-gate-diff.$$` rather than `mktemp` — pid-unique so not
  currently exploitable, but it is the exact shape the C1/S4 detectors flag.
- MCP is an honest typed stub (`Err(NotYetImplemented)`). **No platform connector code exists.**
  **No continuous-learning or recording crate exists.** These are unbuilt features, not defects —
  do not report them as either working or broken.
- `kill -9` on the parent still orphans workers and leaks a worktree.
- `fleet-memory`'s `gate_check.rs:23` `Box::leak`; `ledger --json` emits `"checked": null`.
- Detector precision on a mature codebase (A1/S6/S9/T1) is unmeasured.

---

## 5. What to do next, in order

1. **Re-run the full suite yourself.** `cargo test --workspace --no-fail-fast`, then
   `cargo clippy --workspace --all-targets -- -D warnings`, then
   `find src crates -name '*.rs' -not -path '*/target/*' -not -path '*/.venv/*' | xargs wc -l | awk '$1>80 && $2!="total"'`
   (must be empty — the 80-line cap is absolute), then
   `bash ~/.claude/skills/l8-code/scripts/selfcheck.sh`.
   Do NOT set `FLEET_LOAD_FACTOR` for the suite — it passed 500/0 without it at `00ce9fc`, so
   needing it now would itself be a regression.
2. **Commit** section 2's work if green. If red, fix rather than revert — every change in
   section 2 is a defect fix with its own evidence, and each has a test.
3. **Drive `fleet run` with a task that produces a real change**, so the merge stage is exercised
   on the success path rather than only on the honest-refusal path. This is the biggest untested
   gap in the whole chain. Use `aider` (see `docs/DEPENDENCIES.md` for the command; `opencode` is
   non-functional — `run` exits 124 with zero bytes in three configurations).
4. **Push to `main`**, not `master`. This tree's checked-out branch is `master` and `main` is a
   separate ref: `git branch -f main master && git push origin main`, then verify with
   `git rev-parse --short origin/main`. A bare `git push origin main` from `master` reports
   "Everything up-to-date" with **exit 0 while pushing nothing** — this already happened once.

## 6. Process notes that cost real time

- **Never read `$?` after a pipe** (AGENTS.md hard rule #3, violated 6× by the predecessor and
  twice more by me this session). Redirect to a file, read `$?`, then inspect the file.
- **zsh does not word-split unquoted `$c`** — a `for c in "run --repo ..."` loop passes one giant
  argument and produces a fabricated exit code.
- Subagents repeatedly **stall ending their turn "waiting for the background test run"** and never
  self-resume. Verify their output independently, then `TaskStop`. Note that stopping a parent
  kills its background `cargo` children, which has silently produced 0-byte results.
- A subagent verifier reported `state_dir_default_outside_repo` FAILING; I re-ran it and it passed
  500/0. Its failure was a snapshot of a lane mid-edit. **Re-measure before believing a report.**
- The recurring defect family in this repo, now at **fifteen** instances, is *a check that has
  quietly stopped measuring*: gates green on empty input, detectors whose source moved, a marker
  string that exists nowhere, a denominator printed to the wrong stream, a test asserting the bug
  it should have caught. When something passes, ask what it would have taken to fail.
