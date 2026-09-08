# Independent Adversarial Verification of the Four Opus Claims

Verifier: independent session, no access to the builder's transcript. Repo verified:
`/Users/rachitsrivastava/youtube/Principal Engineering/Light/fleet`, HEAD `75ce54e`.

## 0. Setup note (read this first)

The builder's actual changes are **uncommitted** working-tree edits on top of `75ce54e`
(`git status` shows `M src/dispatch/swarm_cmd.rs` and `?? crates/fleet-worker/src/spawn/change_detect.rs`,
plus ~930 unrelated status lines from an in-flight repo-wide migration). A plain
`git worktree add` from HEAD would **not** include the builder's actual work. To get a genuinely
fresh, isolated copy of what was actually built, I:

1. `git worktree add --detach /tmp/fleet-verify-wt2 75ce54e`
2. `rsync -a --delete` the live `fleet/` directory (excluding `.git`, `target`, `mutants.out*`,
   `.worktrees`, `node_modules`) into that worktree's `fleet/` subdirectory.
3. Diffed the two copies of `swarm_cmd.rs` and `change_detect.rs` byte-for-byte (`diff` → no
   output) to confirm the copy is faithful before touching anything.

All commands below ran from `/tmp/fleet-verify-wt2/fleet` unless stated otherwise, with a fresh
`cargo build`/`cargo clippy`/`cargo test` (no reused target cache from the original tree).

`FLEET_LOAD_FACTOR=10000` was used to get past the load-based capacity refusal (`fleet: system
capacity check failed`, exit 7) on several `fleet swarm` invocations below — noted at each use.

## Independent baseline (measured by me, from scratch)

```
$ cargo build --workspace --all-targets            → exit 0
$ cargo clippy --workspace --all-targets -- -D warnings   → exit 0
$ cargo test --workspace --no-fail-fast             → exit 0, 450 passed / 0 failed
$ find src crates -name '*.rs' | xargs wc -l | awk '$1>80 && $2!="total"' | wc -l   → 0
```

Baseline claim (450 passed / 0 failed, clippy 0, 0 files >80 lines) **REPRODUCES EXACTLY**.

---

## Claim 1 — `fleet swarm` exit code matches its outcome

**Verdict: CONFIRMED.** All three arms are reachable and exit with the documented code, via the
real `fleet` binary, not just unit tests.

```
# Done -> 0 (fixture agent, "done" scenario)
$ FLEET_LOAD_FACTOR=10000 FLEET_STATE=/tmp/fleet_verify_state \
  FLEET_WORKER_TEST_CHILD_EXE=target/debug/fw-fixture-agent \
  target/debug/fleet swarm --repo "$DEMO" --task "done test task" --role builder
    outcome: Done { resolved_model: Some("stub-1"), tokens: Some(Tokens(42)), body: Object {"ok": Bool(true)} }
EXIT:0

# Refused -> 7 (fixture agent, "refuse" scenario)
$ FLEET_WORKER_TEST_CHILD_EXE=target/debug/fw-fixture-agent \
  target/debug/fleet swarm --repo "$DEMO" --task "refuse test task" --role builder
    outcome: Refused { reason: "fixture refusal" }
EXIT:7

# EnvironmentFault -> 3 (child exits without ever writing fd 3: FLEET_WORKER_TEST_CHILD_EXE=/usr/bin/true)
$ FLEET_WORKER_TEST_CHILD_EXE=/usr/bin/true \
  target/debug/fleet swarm --repo "$DEMO" --task "hello world test" --role builder
    outcome: EnvironmentFault { detail: "agent exited without an fd-3 result" }
fleet: environment fault: agent exited without an fd-3 result
EXIT:3
```

The `EnvironmentFault` arm is **not dead code** — I reached it with a real child process
(`/usr/bin/true` via `FLEET_WORKER_TEST_CHILD_EXE`) that exits 0 without writing anything to fd 3,
exactly the "impossible/blocked adapter" attack the brief suggested. `DispatchError::exit_code()`
maps `EnvFault(_) => ExitCode::Env` and `ExitCode::Env = 3` (`crates/fleet-types/src/exit_code.rs:12`).

Denominator: 4 attacks run (mapping-table read, Done, Refused, EnvironmentFault), all 4 executed
successfully end-to-end. None were blocked.

---

## Claim 2 — a no-change `Done` is downgraded to `Refused`

**Verdict: PARTIALLY CONFIRMED — and it contains a real, reproducible defect (see Defect S1).**

The happy/no-op paths work as claimed. But the DETECTION mechanism has a genuine false-negative:

### 2a. Detection failure mode 1 (CONFIRMED DEFECT): a worker that commits its own work is wrongly refused

`changed_file_count` is exactly `git status --porcelain --untracked-files=all` line-count. A
worker that performs real work and **commits it** inside the worktree leaves `git status`
clean — 0 lines — so `enforce_change_honesty` downgrades a genuine `Done` to `Refused`.

Raw git repro (exact command the code runs):
```
$ git init -q && git commit -qm init   # baseline file committed
$ echo changed > f.txt && git add f.txt && git commit -qm "worker change"
$ git status --porcelain --untracked-files=all | wc -l
0
```

End-to-end repro through the real `fleet swarm` binary, using a standalone (non-workspace)
fixture I wrote that performs real work AND commits it
(`/tmp/fleet-verify-wt2/committing_agent.rs`, compiled with plain `rustc`, wired in via
`FLEET_WORKER_TEST_CHILD_EXE` — no repo files touched):
```
$ FLEET_WORKER_TEST_CHILD_EXE=/tmp/fleet-verify-wt2/committing_agent \
  target/debug/fleet swarm --repo "$DEMO" --task "commit test task" --role builder
    outcome: Refused { reason: "worker reported done but left the worktree unchanged
      (changed_files: 0) -- the adapter returned advice, not an applied change.
      worker response: {\"ok\":true}" }
EXIT:7
```
The worker DID apply a change (it wrote a file and committed it) — the refusal message
("the adapter returned advice, not an applied change") is **false**. This is a new false
negative, introduced by the very fix meant to catch false positives.

### 2b. Detection failure mode 2 (CONFIRMED at the unit level, narrower reachability end-to-end): git failure downgrades to Refused, not EnvironmentFault

`changed_file_count`'s own doc comment states: "`git` failing to run at all ... is treated as zero
changes." The shipped unit test `no_git_repo_counts_zero_and_downgrades_done` proves this directly:
a `Done` outcome over a non-git directory is downgraded to `Refused`. This is exactly the rule-7
violation the brief asked about (`DispatchError::EnvFault`'s own doc comment: "This is an
ENVIRONMENT fault and must never be reported as an agent refusal (AGENTS.md rule 7)") — an
environment failure (git missing/broken) is silently folded into "the worker did nothing," which
is a different claim with a different (wrong) exit code (7 instead of 3).

I attempted a full end-to-end repro (stripped `git` from `PATH` via a shim directory containing
symlinks to every `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin` binary except `git`, then ran real
`fleet swarm`). Result: the flow fails **earlier**, at worktree creation (`fleet_merge` needs git
too), with `EXIT:6 fleet: worktree creation failed after retries` — so total git unavailability is
caught upstream, before ever reaching `change_detect`. I could not force the narrower "git present
but this one `git status` call fails" scenario (fork failure / mid-flight `.git` corruption) within
my time budget. The defect is confirmed at the function level (unit test proves it, and is honest
about proving it — the mirror-image comment literally describes the intended semantics), but I
could not independently demonstrate it end-to-end through a live `fleet swarm` run. Flagging this
as **partially tested** rather than claiming full E2E reachability I did not observe.

### 2c. Fake-Done attack (checked, REFUTED — no defect found)

- `.fleet-sandbox/` removal before counting: confirmed by code (`join_impl.rs:28-29`, removed
  immediately before `enforce_change_honesty`) and empirically — my committing-agent repro above
  showed `changed_files: 0` even though `.fleet-sandbox/` necessarily existed during the run.
- `scaffold_fleet_dir` (writes `.fleet/agents.toml`, `.fleet/skills.toml`) is exported from
  `fleet-worker`'s `lib.rs` but **is never called anywhere in `src/` (the CLI dispatch layer)** —
  `grep -rln scaffold_fleet_dir src/` finds nothing. It is dead code, not wired into the
  `swarm`/`join` path at all, so it cannot inflate the count. (Worth flagging separately as dead
  code, but it does not affect Claim 2's honesty.)
- The repo-root `.fleet/state` scorecard write (`scorecard_io::record_scorecard_outcome`) happens
  in `join_impl.rs` **after** `enforce_change_honesty` (line 29 computes the outcome, line 36
  writes the scorecard) — too late to affect the count.

Denominator: 6 distinct attacks attempted (git-status-of-committed-work repro via raw git; E2E
repro via a custom fixture binary; PATH-stripped-of-git E2E attempt; sandbox-removal-ordering
check; scaffold dead-code check; scorecard-write-ordering check). 5 succeeded in fully confirming
or refuting their target; 1 (2b's full E2E git-failure path) was blocked by an earlier refusal and
is reported as inconclusive at the E2E layer, confirmed only at the unit layer.

---

## Claim 3 — the mirror-image test genuinely guards the `Done` path

**Verdict: CONFIRMED.**

Two independent injected defects, both caught, both restored byte-identical afterward
(verified with `md5sum` + `diff`, not just visual inspection):

**Mutation 1** — force `changed_file_count` to always return 0 (`return 0;` as the first line):
```
$ cargo test -p fleet-worker --lib change_detect
test ...::a_real_change_is_still_reported_as_done ... FAILED
  assertion `left == right` failed: the untracked file must be seen
  left: 0
  right: 1
test result: FAILED. 2 passed; 1 failed
```
(The other two tests still pass — this mutation only breaks the mirror-image guard, exactly as
its own doc comment predicts.)

**Mutation 2** — invert the comparison (`if changed_files > 0` → `if changed_files == 0`):
```
$ cargo test -p fleet-worker --lib change_detect
test ...::no_git_repo_counts_zero_and_downgrades_done ... FAILED
test ...::a_real_change_is_still_reported_as_done ... FAILED
test result: FAILED. 1 passed; 2 failed
```

Restore verified both times:
```
$ md5sum change_detect.rs /tmp/change_detect.rs.orig
b15e450ff50dfa84df9a9aa0ee269cbd  (both, identical)
$ diff change_detect.rs /tmp/change_detect.rs.orig   → no output
$ cargo test -p fleet-worker --lib change_detect     → 3 passed; 0 failed
```

The mirror-image test is real, not decorative — it independently catches two different classes
of injected defect (constant-zero and inverted-comparison), and nothing else in the file was
altered by my restore.

Denominator: 2 independent mutations tried, both caught, both cleanly reverted. No mutation
attempted was missed by the test suite.

---

## Claim 4 — `fleet impact --repo` targets the named repo

**Verdict: CONFIRMED for the core claim, with one real defect on the edge case (Defect S2).**

**Builder's exact claimed numbers, reproduced independently:**
```
$ target/debug/fleet impact --repo /tmp/swj3 --symbol main --json     → {"matching_symbols": 1}
$ target/debug/fleet impact --repo ".../Light/fleet" --symbol main --json  → {"matching_symbols": 11}
```
Both match the builder's report exactly (`/tmp/swj3` still existed on disk from the builder's own
session — I did not create it).

**Hand-counted two-repo cross-check (repos I built myself, counts by inspection):**
- R1: `src/a.rs` has `fn alpha(){}`, `fn beta(){}`, `fn alpha_helper(){}`, `struct Alpha;`;
  `src/b.rs` has `fn gamma(){}`. Exact-name match on `alpha` → hand count 1.
  `fleet impact --repo R1 --symbol alpha --json` → `{"matching_symbols": 1}`. **Matches.**
- R2: `src/x.rs` has two functions both literally named `fn alpha(){}` plus `fn delta(){}`.
  Hand count for `alpha` → 2, for `delta` → 1.
  `fleet impact --repo R2 --symbol alpha --json` → `{"matching_symbols": 2}`. **Matches.**
  `fleet impact --repo R2 --symbol delta --json` → `{"matching_symbols": 1}`. **Matches.**
- Cross-repo isolation: `fleet impact --repo R1 --symbol delta --json` → `{"matching_symbols": 0}`
  (delta only exists in R2) — confirms `--repo` is not silently unioning or leaking across repos.
- Default (`.`) behavior: ran from inside R1 with no `--repo` flag → `{"matching_symbols": 1}`,
  identical to the explicit `--repo R1` result. Default works.
- `--json` shape: identical `{"matching_symbols": N}` object across every invocation above and
  with `fleet graph --json` → `{"files_scanned", "symbols", "edges"}`, unchanged shape.

**Defect found (S2): a nonexistent `--repo` does NOT fail clearly — it silently succeeds with 0.**
```
$ target/debug/fleet impact --repo /nonexistent/does/not/exist --symbol foo --json
{
  "matching_symbols": 0
}
EXIT:0
```
Root cause: `src/dispatch/walk.rs::collect` — `let Ok(entries) = std::fs::read_dir(dir) else {
return Ok(()) };` treats "directory does not exist / unreadable" identically to "directory has
no more matching files," silently producing an empty (not erroring) result. A user who typos
`--repo` gets a clean, confident-looking `matching_symbols: 0` that is indistinguishable from a
real zero-hit query against a real repo — no warning, no nonzero exit. It does **not** fall back
to cwd (the brief's other hypothesis) — it is worse: a silent false "success."

Denominator: 6 distinct attacks (2 builder-number reproductions, 2 hand-counted repos with 3
symbol queries between them, 1 cross-repo isolation check, 1 default-`.`-behavior check, 1
nonexistent-path check, 1 `--json` shape check). All ran to completion; 5 confirmed the claim as
correct, 1 (nonexistent path) surfaced a real defect.

---

## Also checked — honesty of the whole `swarm` command

**Finding (folds into Defect S1 as the same root cause): `docs/USING-FLEET.md` is now stale and
contradicts the live binary.**

`fleet --help` and `fleet swarm --help` make no claim that `swarm` applies code changes — they are
honest by omission (say "Spawn a worker lane," nothing about editing files). That part is fine.

But `docs/USING-FLEET.md` §7 (lines 255–270) has a worked example claiming:
```
$ fleet swarm --repo /tmp/scratch-repo --task "hello world task" --role builder
lane: builder-60626-0
outcome: Done { resolved_model: Some("codestral-latest"), ..., body: Object {"response": String("# Hello World Task\n\n...") ...} }
EXIT:0
```
I re-ran the **literal documented command** for real (real network call to the same keyless
`codestral-latest` endpoint via `api.llm7.io`, per the doc's own note) against a fresh disposable
repo:
```
$ FLEET_STATE=/tmp/fleet_verify_state target/debug/fleet swarm --repo "$DEMO" --task "hello world task" --role builder
    outcome: Refused { reason: "worker reported done but left the worktree unchanged
      (changed_files: 0) -- the adapter returned advice, not an applied change.
      worker response: {... "response":"# Hello World Task\n\nHere's a simple \"Hello World\"
      task in several programming languages: ... " ...}" }
EXIT:7
```
Same model, same kind of chat-prose response the doc shows — but the doc says `EXIT:0`/`Done` and
the real binary says `EXIT:7`/`Refused`. This is not a flaky network difference; it is Claim 2's
new honesty check correctly catching the exact scenario the docs use as their "it works" example
(a chat response with no file diff). **The docs were not updated when the change-honesty check was
added.** Per the verification protocol, running the README/docs's own documented command and
getting a different result than documented is a genuine "docs are wrong" finding — I am not
failing the whole task over it (the underlying binary behavior is *correct*, per Claim 2), but the
docs actively mislead a reader into expecting `EXIT:0` from a command that will now legitimately
exit 7 on every un-augmented chat-only adapter run, which is most of them.

`docs/QUICKSTART.md`'s one `swarm` mention is a neutral list of refusal-capable commands; no false
claim there.

---

## Break-it round (empty/wrong input, restart mid-op, run twice)

```
$ fleet impact --repo /tmp/swj3 --symbol "" --json          → {"matching_symbols": 0}, EXIT:0 (fine)
$ fleet impact --repo <empty dir> --symbol foo --json        → {"matching_symbols": 0}, EXIT:0 (fine)
$ fleet swarm --repo "$DEMO" --task "" --role builder        → "task_id must not be empty", EXIT:7 (fine)
$ fleet swarm --repo /nonexistent/nope --task x --role builder → "worktree creation failed after retries", EXIT:6 (fine, and NOTE: inconsistent with impact's silent-0 behavior on the same kind of bad path — swarm fails loud, impact fails silent)
$ fleet swarm ... (run twice back-to-back, same repo/task)   → both EXIT:0, distinct lane ids, worktrees cleaned up each time (fine, idempotent)
```

**Restart-mid-operation (new defect, S3, outside the four claims but found via the mandated
"restart mid-operation" attack):** started `fleet swarm` with the fixture's `timeout` scenario
(which forks a sleeping grandchild), let it spawn, then `kill -9`'d the **parent `fleet` process
itself** (simulating a crash) 2 seconds in:
```
$ kill -9 $FLEET_PID
$ git -C "$DEMO" worktree list
.../tmp.nwFzdz5sjo/.worktrees/builder-72683-0  71914ac  [fleet/builder-72683-0]    # never cleaned up
$ ps aux | grep -E 'sleep 300|fw-fixture-agent'
... fw-fixture-agent __agent freelane .../builder-72683-0 timeout test task   (still running)
... sh -c echo $$ > '.../builder-72683-0/.grandchild-pid'; sleep 300           (still running)
... sleep 300                                                                  (still running)
```
A re-run of `fleet swarm` against the same repo immediately afterward still succeeds (new lane
name, no collision) — so this is not a correctness break of the four claims — but the orphaned
worker process, its grandchild, and the stale worktree directory are never reaped, because
`terminate_group`/worktree-removal only run inside `join()`, which never executes if the parent
itself dies. This is a real operational leak (disk + process) on any `fleet` crash or `kill -9`,
worth a follow-up but out of scope for the four claims under test. I killed the orphans and
removed the stale worktree manually after observing this.

---

## Defects found, ranked

- **S1 — `enforce_change_honesty` produces false negatives for workers that commit their own
  work**, mislabeling a genuine `Done` as `Refused` with a false "returned advice, not an applied
  change" message, and (same root cause) **`docs/USING-FLEET.md` §7's worked `swarm` example is
  now factually wrong** (claims `EXIT:0`/`Done`, live binary gives `EXIT:7`/`Refused` for the
  literal documented command). Repro: `FLEET_WORKER_TEST_CHILD_EXE=/tmp/fleet-verify-wt2/committing_agent
  target/debug/fleet swarm --repo "$DEMO" --task "commit test task" --role builder` → `EXIT:7`.
- **S2 — `fleet impact --repo <nonexistent path>` silently succeeds with `matching_symbols: 0`**
  instead of failing clearly; masks a typo'd/wrong `--repo` as a legitimate zero-hit result.
  Root cause: `src/dispatch/walk.rs::collect`'s `let Ok(entries) = std::fs::read_dir(dir) else {
  return Ok(()) };`. Repro: `fleet impact --repo /nonexistent/does/not/exist --symbol foo --json`
  → `{"matching_symbols": 0}`, `EXIT:0`.
- **S3 — not-a-git-repo / git-failure inside `change_detect` downgrades an environment fault to a
  `Refused` (exit 7) instead of `EnvironmentFault` (exit 3)**, violating the project's own stated
  rule 7. Confirmed at the unit level (`no_git_repo_counts_zero_and_downgrades_done`); I could not
  independently force this exact branch through a live end-to-end `fleet swarm` run in my time
  budget (total git unavailability fails one step earlier, at worktree creation, with exit 6
  instead) — narrower real-world reachability than S1, but the code and its own test both attest
  to the behavior existing as designed.
- **S4 — a killed/crashed `fleet` parent process leaves orphaned worker processes (plus their
  grandchildren) and a stale `.worktrees/<lane>` directory behind indefinitely.** Outside the four
  claims, found via the mandated "restart mid-operation" attack. Repro: start `fleet swarm` with
  the fixture's `timeout` scenario, `kill -9` the parent 2s in, observe `ps aux` and
  `git worktree list` afterward.
- (Informational, not a scored defect) `crates/fleet-worker/src/sandbox/scaffold.rs::scaffold_fleet_dir`
  is exported but unreferenced anywhere in `src/` — dead code, does not affect any of the four
  claims (verified it cannot inflate Claim 2's change count since it is never called on the
  `swarm`/`join` path).

## Which claims rest on unreachable/untested code paths

- **Claim 1: fully reachable**, all three outcome arms demonstrated via the real binary.
- **Claim 2: the defect-relevant path (committed work) is fully reachable and confirmed broken
  end-to-end (S1)**; the narrower "git command itself fails" path is confirmed only at the unit
  level in this session, not end-to-end (blocked by an earlier, correct refusal upstream) — see S3.
- **Claim 3: fully reachable**, both injected mutations caught by the real test suite.
- **Claim 4: fully reachable and correct for the core claim**; the nonexistent-path edge case is
  reachable and confirmed to silently misbehave (S2).

## Denominator summary

| Claim | Distinct attacks run | Could not run |
|---|---|---|
| 1 | 4 (mapping read, Done, Refused, EnvironmentFault, all via real binary) | none |
| 2 | 6 (committed-work raw-git repro, E2E custom-fixture repro, PATH-stripped-of-git E2E attempt, sandbox-removal-ordering, scaffold dead-code check, scorecard-write-ordering) | full E2E repro of "git command itself fails mid-run" (upstream refusal intercepts first) |
| 3 | 2 mutations (force-zero, inverted comparison), each with catch + byte-identical restore verified | none |
| 4 | 6 (2 builder-number reproductions, 2 hand-counted repos across 3 symbol queries, cross-repo isolation, default-`.` check, nonexistent-path check, `--json` shape check) | none |

## Independent baseline numbers (repeated for the record)

- `cargo build --workspace --all-targets`: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0
- `cargo test --workspace --no-fail-fast`: exit 0, **450 passed / 0 failed** (matches claim exactly)
- Files over 80 lines in `src/`+`crates/` (`*.rs`): **0** (matches claim exactly)
