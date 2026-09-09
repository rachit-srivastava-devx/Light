# MORNING — read this first

Five minutes, then you're caught up. This replaces nine separate session documents that piled up
overnight (`NIGHT-PLAN.md`, `DX-AUDIT.md`, `USER-JOURNEY.md`, `USER-JOURNEY-2.md`,
`VERIFY-OPUS-CHANGES.md`, `VERIFY-APPLY-HONESTY.md`, `GRAPH-NODE-AUDIT.md`, plus this file's own
previous draft) — that pile was itself a regression against your instruction to cut the docs down.
All nine are archived, evidence intact, under
[`docs/archive/night-2026-09-08/`](archive/night-2026-09-08/). `docs/NIGHT-PROGRESS.md` is still
being appended to live and is not touched here.

**Method note, stated up front because it governs everything below:** `opencode` could not be
used — it hangs and emits zero bytes, reproduced in three configurations (bare, `--pure
--print-logs`, and with a configured keyless provider) — so `aider` was used instead, keyless, per
your authorisation. The exact recipe and the keyless endpoint facts are now permanent guidance in
[`docs/DEPENDENCIES.md`](DEPENDENCIES.md#driving-a-keyless-coding-agent-against-this-repo), not
buried in a dated plan file.

## How to try it

Every command below was run just now, from this checkout, output pasted verbatim.

```
$ fleet --help
```
28 commands, all with a one-line description now (was 0/28 earlier this week). 7 of them say
`NOT IMPLEMENTED: <reason>` right in `--help` (`skills`, `attest`, `pr`, `contract`, `freeze`,
`console`, `mcp`); `adjudicate` is feature-gated instead of marked, which is either the intended
8th stub or an off-by-one in how "8 stubs" was originally counted — **needs your call**, see below.

```
$ fleet doctor --json
{
  "cargo": true,
  "git": true,
  "capacity_decision": "allow (concurrency_cap=2)",
  "version": "0.1.0",
  "commit_sha": "f94dc5857de1",
  "tree_state": "dirty",
  "build_time": "2026-09-09T03:11:20Z"
}
EXIT:0
```
`commit_sha`/`tree_state`/`build_time` are new tonight — closes the "which binary am I even
running" gap `USER-JOURNEY-2.md` hit (see the fixed-table below).

```
$ FLEET_STATE_DIR=/tmp/morning-fleet-state fleet gate --id detectors
    .... gate /var/folders/.../detector-integrity.sh -- running (budget 7.99s)
    PASS gate detectors 111/111
-- summary --
gates    1 attempted -- 1 passed, 0 failed, 0 skipped
checks   111/111 performed
EXIT:0
```

```
$ cd <fleet workspace> && FLEET_LOAD_FACTOR=10000 FLEET_VERIFY_BUDGET_SECS=60 \
    FLEET_STATE_DIR=/tmp/morning-fleet-state \
    fleet gate --id "unit tests" --repo /tmp/morning-verify-scratch
    .... gate cargo test --workspace -- running (budget 59.98s)
    PASS gate unit tests 1/1
-- summary --
gates    1 attempted -- 1 passed, 0 failed, 0 skipped
checks   1/1 performed
EXIT:0
```
This is the crux command from both user journeys: run from the **fleet workspace's own cwd**,
pointed with `--repo` at a throwaway scratch crate. It tested the scratch crate (1/1, ~2s), not
fleet's own multi-hundred-target workspace (which alone takes over 120s to time out). See "refuted
claims" below — this exact command used to be unreliable depending on which binary you had on
`PATH`.

## Current state (measured just now, not copied)

```
$ cargo test --workspace --no-fail-fast
... 493 passed / 0 failed
EXIT:0

$ cargo clippy --workspace --all-targets -- -D warnings
EXIT:0

$ find src crates -name '*.rs' | xargs wc -l | awk '$1>80 && $2!="total"'
(empty — 0 files over 80 lines)
```
A `selfcheck` command referenced in the archived plan was not found anywhere in this checkout
tonight (`find . -iname 'selfcheck*'` → nothing) — dropping that number rather than repeating an
unverifiable one. `git status --short` shows 18 modified/untracked paths under active,
concurrent work in `src/` and `crates/` (matches `docs/NIGHT-PROGRESS.md`'s own note about a
parallel lane) — nothing here was touched to produce the numbers above.

## What was wrong and is now fixed

| Defect | Evidence |
|---|---|
| `fleet rollback` deleted arbitrary paths, reported success | Exploited: scratch dir + file, `--worktree /tmp/victim` → `ok: worktree removed`, exit 0, gone. Now canonicalises both sides and refuses outside `.worktrees/`, exit 7 |
| `run`/`oracle`/`gate` hung forever, zero output | Was exit 124/zero bytes; now bounded, 6–8s with output (re-confirmed tonight: `gate --id detectors` returned in under a second) |
| `swarm --task` rejected as empty unless `--prompt` also passed | Cross-wire fixed; `swarm_task_alone_is_accepted.rs` passed in tonight's 493-test run |
| Ledger append was O(n²) | 4k appends 49.19s → 11.67s, ratio now ~2×/doubling, asserted by a test (unchanged tonight, not re-benchmarked) |
| Corpus/detector gate had stopped measuring (`0 of 25`) | Now loud on `checked==0`; detectors gate is `111/111` as measured tonight (count grew as more detectors were added — the earlier "25" is stale, not wrong) |
| `checked 0/8` mixed two different units into one misleading fraction | Split into separate `gates`/`checks` lines; `checks 0` is now printed as an explicit failure, not a neutral number |
| `fleet gate`/`fleet oracle` had no `--repo` at all; the two `cargo` gates silently tested the operator's cwd, the other six never looked at `--repo` under any circumstances | **Was found as GRAPH-NODE-AUDIT.md's S1, now fixed** — `--repo` exists on both (`--help` literally cites "S1 fix"), threaded to every gate's `.current_dir()`. Re-verified live tonight (see "how to try it" above): PASS 1/1 in ~2s from a foreign cwd |
| `fleet impact --repo <nonexistent path>` silently returned `matching_symbols: 0`, exit 0 | **Was VERIFY-OPUS-CHANGES.md's S2, now fixed** — re-verified tonight: `fleet: repo root "/nonexistent/does/not/exist" does not exist or could not be read: No such file or directory (os error 2)`, exit 3 |
| `fleet graph`'s `edges` count was always 0, even on repos with obvious calls | **Was flagged in USER-JOURNEY-2.md's S3, now fixed** — re-verified tonight on a 3-symbol scratch crate: `edges: 2` (`main`→`add`, test→`add`), matching a hand count |
| `Teach` computed a real `Lesson` from the failure and threw it away (`let _lesson = ...`) | **Was GRAPH-NODE-AUDIT.md's S2, now fixed** — `teach_persists_lesson.rs` (in tonight's green 493) proves a lesson written by one process is read back, off disk, after that process has exited |
| Change-honesty check had two independent zero-cost bypasses: a worker that committed real work was wrongly refused, and — the worse one — fleet's own `.fleet-lane.pid` bookkeeping file (written into the worktree the instant the child spawns) satisfied "something changed" on **every** lane regardless of what the worker did | **Was VERIFY-OPUS-CHANGES.md's S1 + VERIFY-APPLY-HONESTY.md's finding (7 of 7 `Done`, 0 `Refused`, including an explicit LLM refusal), now fixed** (commits `37a7562`, `f94dc58`) — re-verified live tonight: a genuine no-op lane now returns `Refused`, and the target repo is no longer littered with a stray `.fleet/` directory (scorecards moved to `FLEET_STATE_DIR`) |
| `cargo mutants` ran unconditionally after a migration silently dropped its opt-in guard | Restored (commit `7fc0e61`); re-verified live tonight — `fleet gate --id mutants` now `SKIP`s by default, only runs with `FLEET_MUTANTS=1` |
| `fleet doctor`/`fleet version` gave no way to tell which build you were running | **Was USER-JOURNEY-2.md's S2, now fixed** — both now print `commit_sha`/`tree_state`/`build_time`, re-verified live tonight |
| Empty help on all 28 commands | Every one described; 7 of 8 stubs say why inline (see "how to try it") |
| Docs described a CLI that never existed (`sow accept`, `ledger verify`, `agents list`, …) | `docs/USING-FLEET.md` rewritten against the real flat-flag CLI, every example executed |

### Refuted claims — where two nights' evidence disagreed, the later evidence wins

- **`docs/archive/night-2026-09-08/USER-JOURNEY.md` claimed fleet had verified aider's change.**
  That run happened to `cd` into the scratch repo first and called `fleet gate --id "unit
  tests"` with no `--repo` flag at all (it didn't exist on that binary) — the verdict was
  correct only because cwd happened to equal the target repo, not because `--repo` was honoured.
  `docs/archive/night-2026-09-08/USER-JOURNEY-2.md` proved this was fragile: the identical command
  from a **foreign** cwd, on a stale binary, silently tested fleet's own monorepo instead (120s
  timeout against the wrong tree). The current build fixes this for real — verified again tonight,
  independently, in "how to try it" above. Treat the original journey's "fleet verified aider's
  change" headline as **refuted as stated**; the corrected, reproducible version is the crux command
  above.
- **The same `USER-JOURNEY.md` also claimed `fleet swarm` "does not create the worktree its own
  description promises."** `docs/archive/night-2026-09-08/VERIFY-OPUS-CHANGES.md`'s process-level
  trace shows a worktree genuinely is created and used for the whole lane, then torn down by design
  the moment `join()` finishes its honesty check — the empty `.worktrees/` the journey observed was
  the normal **post-teardown** state, not a missing feature. Superseded, not a live defect.
- **`docs/archive/night-2026-09-08/GRAPH-NODE-AUDIT.md`'s S1 ("gate/oracle have no `--repo` flag at
  all")** was true when written and is fixed as of this build (see the fixed-table row above) — do
  not read it as still open.

## What is still open

### Needs the owner

1. **`git rm -r -f fleet/docs/pdfs`** — 179 files, 34MB, zero PDFs, tracked at `HEAD` under `tmp/`,
   referenced by nothing. Blocked twice by the permission classifier. Recover with
   `git checkout HEAD -- fleet/tmp/pdfs` if that's ever needed.
2. **`Principal Engineering/` is not a git repository**, so `registry/services/llm-gateway` is
   tracked by nothing and unreachable from any clone; it also has no build script while its
   `exports` map points at four `.js` files nothing can produce. Phase 0 blocker.
3. **`fleet-rs`: 364 of 570 commits** are an unattended loop committing "verify RED, findings kept,
   code not committed." Four of seven "active" days contain zero real commits. Six task tags each
   retried ~73× and never gave up. Fix the give-up condition; don't rewrite history.
4. **`fleet-merge::remove()`** returns `Err(NotFound)` for an already-removed worktree, breaking the
   documented idempotence contract. Your ruling needed.
5. **Which command is the intended 8th stub?** `--help` currently marks 7 (`skills`, `attest`,
   `pr`, `contract`, `freeze`, `console`, `mcp`) as `NOT IMPLEMENTED`; `adjudicate` is feature-gated
   instead. Confirm whether that's the intended 8th or the "8 stubs" count was off by one.
6. **Nothing here is committed.** `git status --short` shows 18 modified/untracked paths from
   active work in `src/`/`crates/`. Review before committing anything.

### Known-open defects

- **`Scan`/`Plan` pipeline stages are wired to permanently empty inputs**
  (`fleet_scan::merge_questions(Vec::new())`, a fixed template string) — re-read tonight in
  `src/pipeline/stages.rs`, still true. They can only ever trivially pass regardless of the real
  task or repo; self-flagged in the file's own doc comment.
- `fleet-memory/src/gate_check.rs:23` leaks via `Box::leak` per distinct category. Bounded by the
  no-daemon tenet today; a real bug the day anything long-running calls it.
- `fleet gate --id corpus` hangs through the binary while `bash gates/corpus/run.sh` itself
  finishes in ~2s — a `RealRunner` plumbing difference, not re-tested tonight.
- Context-engineering budget enforcement is proven only against synthetic strings, never real
  prose.
- A killed/crashed `fleet` parent process leaves orphaned worker processes (plus grandchildren) and
  a stale `.worktrees/<lane>` directory behind indefinitely — `join()`'s cleanup never runs if the
  parent itself dies. Not re-tested tonight.
- `ledger --json` emits `"checked": null`; honest, but `Number(null) === 0` in JS has bitten this
  repo before.
- `selfcheck`'s D2 false-positive on a deliberate bad-`mktemp` fixture, carried forward from the
  archived plan — **UNVERIFIED tonight**: no `selfcheck` command was found anywhere in this
  checkout, so this line is kept only as a historical pointer, not a confirmed current defect.

## The one pattern worth remembering

Nine separate defects this week were the same shape: **a check that had quietly stopped measuring
anything, while still reporting a verdict.**

1. a stub pipeline stage (`Scan`/`Plan` always-empty inputs)
2. a lying gate (one that reported success while every real check inside it had failed)
3. `0 of 25` corpus detectors passing because the meta-check had stopped measuring since a migration
4. a `checked 0/8` summary line that printed a neutral-looking fraction instead of screaming
5. `Verify` reading the wrong directory (the operator's own cwd, or the gates-root itself — never
   `--repo`)
6. `Teach` computing a real lesson and discarding it
7. a guard deleted along with its own watchdog (the mutants opt-in check silently dropped by a
   migration, with nothing left to notice it was gone)
8. fleet's own `.fleet-lane.pid` bookkeeping file satisfying its own "did anything change" honesty
   check, on every single lane, regardless of what the worker did
9. `edges` reported as a confident `0`, indistinguishable from "nothing calls this"

Every one of them looked like a passing check. Every one of them measured nothing. The fix, every
time: never let `checked == 0` print as a neutral number — make it a loud failure, publish the
denominator, and run at least one test through the real binary on the real path, not a substitute.

## Archived evidence

| Archived file | What it covers |
|---|---|
| [`archive/night-2026-09-08/NIGHT-PLAN.md`](archive/night-2026-09-08/NIGHT-PLAN.md) | The night's work order and baseline; its live guidance (aider recipe, keyless endpoint facts) moved to `docs/DEPENDENCIES.md`, its owner-only items folded into "Needs the owner" above |
| [`archive/night-2026-09-08/DX-AUDIT.md`](archive/night-2026-09-08/DX-AUDIT.md) | 112-check adversarial CLI audit: 13 S1 (broken), 9 S2 (unusable) findings that drove most of tonight's fix list |
| [`archive/night-2026-09-08/USER-JOURNEY.md`](archive/night-2026-09-08/USER-JOURNEY.md) | First end-to-end run with aider; contains the refuted "fleet verified the change" and "no worktree" claims, see above |
| [`archive/night-2026-09-08/USER-JOURNEY-2.md`](archive/night-2026-09-08/USER-JOURNEY-2.md) | Corrected re-run that caught a stale binary on `PATH` mid-session and proved the `--repo` fix on a fresh build |
| [`archive/night-2026-09-08/VERIFY-OPUS-CHANGES.md`](archive/night-2026-09-08/VERIFY-OPUS-CHANGES.md) | Independent adversarial verification of four claims; found the change-honesty false-negative and the `impact --repo` silent-zero defect |
| [`archive/night-2026-09-08/VERIFY-APPLY-HONESTY.md`](archive/night-2026-09-08/VERIFY-APPLY-HONESTY.md) | Traced the change-honesty check to its root cause: `.fleet-lane.pid` satisfying it on every lane; 7-of-7 false `Done`s |
| [`archive/night-2026-09-08/GRAPH-NODE-AUDIT.md`](archive/night-2026-09-08/GRAPH-NODE-AUDIT.md) | Exhaustive node-by-node audit of every crate and pipeline stage; found the `--repo`-blind Verify stage, the discarded Teach lesson, and the always-empty Scan/Plan stages |

Full evidence for everything still active is `docs/NIGHT-PROGRESS.md` (append-only, in progress).
