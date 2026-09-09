# Verify: does the apply step re-open the "Done with zero diff" defect?

## Verdict: **NEITHER (A) nor (B) as framed — a third, more severe mechanism. Practically: (A).**

`enforce_change_honesty` **is reached**, on the **correct worktree path**, **before** teardown
(confirmed by reading `crates/fleet-worker/src/spawn/join_impl.rs:26-29`: the honesty check runs
at line 29, `fleet_merge::remove` — the actual teardown — runs at line 38, strictly after). So (A)
as literally stated ("not reached", "wrong path", "after teardown") is **false**.

The apply step (`crates/fleet-worker/src/freelane/apply/`) is also **not** manufacturing a diff in
any of the 7/7 reproductions below — `apply::try_apply` returned `Err(AmbiguousTarget)` every time
(no `path:`/`file:` line, no `path:` info-string on the model's fenced blocks) and **wrote nothing**
to the worktree. So (B) as framed ("apply extracts a fence and writes it") is also **false** for
this repro.

**What is actually true, proven below:** `crates/fleet-worker/src/reap.rs::record_worker_pid`
writes `<worktree>/.fleet-lane.pid` directly into the worktree root the moment the worker process
spawns (`crates/fleet-worker/src/spawn/mod.rs:64`), and **nothing ever removes it** before
`enforce_change_honesty` runs — `join_impl.rs` only removes `.fleet-sandbox/`
(`join_impl.rs:26-28`), never `.fleet-lane.pid`. `change_detect::detect::dirty_file_count` calls
`git status --porcelain --untracked-files=all`, which counts `.fleet-lane.pid` as an untracked
file. **This makes `dirty_file_count() > 0` true on literally every lane, independent of whether
the worker did anything at all.** The honesty check is reached, looks at the right directory,
runs before teardown — and passes every single `Done` anyway, because fleet's own bookkeeping
file is indistinguishable, to `git status`, from real work.

This is worse than (B): (B) at least requires the model to emit content that gets written. This
bug requires nothing from the model — a total no-op, silent-exit, or outright refusal ("I can't
assist with that") all read as `Done` with a clean diff on the actual task, because `.fleet-lane.pid`
alone satisfies the check.

## 1. Reproduction — commands and literal output

Build (from repo root, clean state confirmed via `git status --short` before starting; a
concurrent, unrelated process was independently editing `src/Cargo.toml`/`src/build.rs` in this
same checkout mid-session — see §6 — but never touched anything under `crates/fleet-worker/`,
so it does not affect this repro):

```
$ cargo build --bin fleet                          # EXIT:0 (already built, sha1 9f9def2f86e17d1ae7dc3991f8150cf454315f85)
$ shasum target/debug/fleet
9f9def2f86e17d1ae7dc3991f8150cf454315f85  target/debug/fleet
$ shasum ~/.local/bin/fleet
9f9def2f86e17d1ae7dc3991f8150cf454315f85  /Users/rachitsrivastava/.local/bin/fleet   # same build, not stale
```

Scratch repo (`$SCRATCH`), a trivial Rust crate with one passing test:

```
$ cargo init --name scratchcrate --vcs none && git init -q && git add -A && git commit -q -m "Initial scratch crate"
$ git log --oneline
f01c534 Initial scratch crate
```

**Attempt 1** (all commands run with `FLEET_LOAD_FACTOR=10000` per the load-refusal note):

```
$ FLEET_LOAD_FACTOR=10000 target/debug/fleet swarm --repo "$SCRATCH" \
    --task "Add a doc comment above the add function explaining what it does" --role builder
EXIT:0
    [builder-45409-0] spawned
    [builder-45409-0] outcome: Done { resolved_model: Some("codestral-latest"), tokens: None,
      body: Object {"agent": String("freelane"), "log": String("[...tried=api.llm7.io:answered
        usage={\"prompt_tokens\": 15, \"completion_tokens\": 154, \"total_tokens\": 169}]"),
        "resolved_model": String("codestral-latest"),
        "response": String("Here's the `add` function with a doc comment explaining its
          purpose:\n\n```python\ndef add(a, b):\n    \"\"\"Adds two numbers and returns the
          result.\n\n    Args:\n        a (int or float): ...\n    \"\"\"\n    return a + b\n```
          \n\nThis doc comment follows standard Python documentation conventions..."),
        "status": String("done"), "tokens": Number(169)} }

$ git -C "$SCRATCH" status --porcelain
?? .fleet/                 # scorecard bookkeeping written AFTER honesty check, to repo root
$ cat "$SCRATCH/src/main.rs"
fn add(a: i32, b: i32) -> i32 { a + b }     # byte-identical to before the run — untouched
fn main() { println!("2 + 3 = {}", add(2, 3)); }
#[cfg(test)] mod tests { ... }
```

The model answered in **Python** ("```python ... def add(a, b): ...") in a Rust crate, with no
`path:`/`file:` declaration anywhere in the reply — `apply::apply` therefore returns
`Err(ApplyError::AmbiguousTarget)` (confirmed by code reading, `crates/fleet-worker/src/freelane/apply/mod.rs:33`
and `parse.rs`'s exact grammar) and writes nothing. `src/main.rs` is byte-for-byte unchanged.
**Outcome was still `Done`.**

**Attempts 2-5**, same task text (numbered to vary the prompt slightly, same behavior each time):

```
attempt 1: EXIT:0  outcome: Done   git status: ?? .fleet/state/scorecards/builder.{json,lock}
attempt 2: EXIT:0  outcome: Done   git status: ?? .fleet/state/scorecards/builder.{json,lock}
attempt 3: EXIT:0  outcome: Done   git status: ?? .fleet/state/scorecards/builder.{json,lock}
attempt 4: EXIT:0  outcome: Done   git status: ?? .fleet/state/scorecards/builder.{json,lock}
attempt 5: EXIT:0  outcome: Done   git status: ?? .fleet/state/scorecards/builder.{json,lock}
```
`src/main.rs` verified byte-identical to the original after all 5 (`cat` shown above, unchanged).

**Attempts 6-7**, adversarial prompts meant to provoke a refusal or a network failure:

- Attempt 6, task "trigger a refusal": model replied *"I can't assist with that..."* (zero fences
  at all → `ApplyError::NoFence`) — still **`Done`**.
- Attempt 7, task with `https_proxy=http://127.0.0.1:9 http_proxy=http://127.0.0.1:9` set (trying
  to break the network lane): had **no effect** — `hermetic_env::apply` (`crates/fleet-worker/src/sandbox/hermetic_env.rs:49`)
  calls `command.env_clear()` and only forwards `PATH/HOME/XDG_*/LANG/CARGO_TARGET_DIR`, so
  `https_proxy` never reached the child. Model answered normally (fenced ```bash/```python, no
  path declared) — still **`Done`**.

### Tally: **7/7 `Done`, 0/7 `Refused`, out of 7 attempts.**

I could not produce a `Refused` `LaneOutcome` from this endpoint within budget — every attempt
(including a deliberate LLM refusal and a network-disruption attempt that fizzled because the
hermetic env strips proxy vars) came back `Done`. This is itself informative: with the pid-file
bug in place, it is very hard to observe the honesty check ever actually firing, because almost
any HTTP 200 with any body text becomes `Done` and stays `Done`.

For every one of the 7 `Done`s: **a file was written** (`.fleet-lane.pid` inside the worktree,
plus `.fleet/state/scorecards/builder.{json,lock}` at the repo root afterward) — but never the
one the task asked for. `src/main.rs` never changed. No Python (or other) content ever landed in
the Rust crate, because `apply` correctly refused every single reply (no declared path) — the
defect is not apply writing garbage, it's the honesty check being blind to the fact that nothing
useful was written.

## 2. Direct proof the worktree really is clean at the moment `join()` inspects it

Ran attempt with concurrent polling of the live worktree (`.worktrees/builder-49422-0`) every
20ms while the lane was in flight, from spawn to teardown (52 polls captured, iterations 30-52
shown below are representative — full log is in the session, `?? .fleet-lane.pid` present on
**every single poll**, `M src/main.rs` **never** appears):

```
=== iter 30 ===
(worktree contents: Cargo.toml, .fleet-lane.pid, .fleet-sandbox/, .git, src/main.rs)
=== iter 31 ===
?? .fleet-lane.pid
...
=== iter 52 ===
?? .fleet-lane.pid
=== final output ===
outcome: Done { ... same Python-prose response ... }
=== final status (post-teardown, repo root) ===
?? .fleet/state/scorecards/builder.json
?? .fleet/state/scorecards/builder.lock
```

`.fleet-lane.pid` is present in the worktree from the moment the child process is spawned
(`record_worker_pid`, `spawn/mod.rs:64`) until teardown removes the whole worktree — i.e. for the
entire window `enforce_change_honesty` has to work with. It is the **only** untracked/dirty
artifact in every poll. `src/main.rs` never shows `M`.

## 3. Call-order finding

`crates/fleet-worker/src/spawn/join_impl.rs::join`:
1. line 16-24: wait for the child, get `raw_outcome` from fd-3.
2. line 28: `std::fs::remove_dir_all(&handle.sandbox_root)` — removes `.fleet-sandbox/` only.
3. **line 29: `enforce_change_honesty(raw_outcome, &handle.worktree.path, &handle.base_commit)`**
   — correct worktree path, correct base commit, runs while the worktree still fully exists.
4. line 35-36: scorecard write to `handle.repo.join(".fleet").join("state")` — this is the
   **repo root**, not the worktree, and it happens **after** the honesty check, so it cannot be
   what's polluting the check (I originally suspected this; ruled out by inspecting worktree
   paths directly — see §2, the worktree itself never gets a `.fleet/state` entry, only the repo
   root does, post-teardown).
5. line 38: `fleet_merge::remove(&handle.repo, &handle.worktree)` — the actual teardown, strictly
   **after** the honesty check.

So the call order is correct by the design intent stated in the module doc comment
(`change_detect/mod.rs:1-15`) — `.fleet-sandbox/` genuinely is excluded before the check runs, as
documented. The bug is a **sibling artifact the author of that fix didn't also exclude**:
`.fleet-lane.pid`, written by a completely different module (`reap.rs`, called from `spawn()`,
not `join()`) directly into the worktree root, never into `.fleet-sandbox/`, and never cleaned up
by `join()` at all — only ever removed as part of the whole-worktree deletion in step 5, i.e.
after the honesty check has already been fooled by it.

**This also means the bug is not specific to `freelane`/`apply` at all.** `record_worker_pid` is
called by `spawn()` for every adapter (`Freelane`, `Claude`, `Codex` alike — `spawn/mod.rs:64` is
adapter-agnostic). Any lane that gets as far as a spawned child process and returns any `Done`
body will have `.fleet-lane.pid` in its worktree by the time `join()` checks, regardless of
adapter. The freelane/apply story is how I found it (S1 in `docs/USER-JOURNEY-2.md`), but the
mechanism defeats the honesty check for the CLI adapters too.

## 4. Existing test coverage: masks rather than catches this

- `crates/fleet-worker/src/spawn/change_detect/tests.rs::a_true_no_op_is_still_refused` **passes**,
  and correctly proves `enforce_change_honesty` is logically sound *in isolation* — but it calls
  the pure function directly against a bare `tempfile::tempdir()` repo that never had
  `record_worker_pid` write anything into it. It never exercises the real `spawn()`→`join()` path.
- `crates/fleet-worker/tests/spawn_join_happy.rs::spawn_join_stub_agent_round_trips_cleanly` DOES
  go through the real `spawn()`/`join()` pipeline, but its fixture scenario (`"done"` →
  `fixture_scenarios::send_done_with_change`, `tests/fixtures/fixture_scenarios.rs:17-21`)
  **deliberately writes a real file** (`fixture-change.txt`) before returning `Done` — so this
  test is not evidence either way for the no-op case; it's testing the legitimate-change path.
- `crates/fleet-worker/tests/fixtures/fixture_agent.rs` defines a `"done_no_change"` scenario
  (`send_done` with **no** file write — the actual no-op case) but its **only** caller,
  `crates/fleet-worker/tests/spawn_join_git_env_fault.rs`, deliberately breaks `git` itself via
  `FLEET_WORKER_TEST_CHANGE_DETECT_GIT` before the honesty check runs, specifically so it never
  reaches `dirty_file_count`'s real `git status` call on a healthy repo.
- **There is no test anywhere in the suite that runs a true no-op `Done` through the real,
  healthy `spawn()`→`join()` pipeline and asserts it gets downgraded to `Refused`.** That is
  exactly the gap `.fleet-lane.pid` lives in, and exactly why `cargo test -p fleet-worker` is
  fully green (verified: `cargo test -p fleet-worker apply::` → 7/7 apply unit tests pass;
  `cargo test -p fleet-worker --test spawn_join_happy` → 1/1 pass) while the real binary fails
  the property those tests were meant to guarantee.

## 5. Does the apply step's own refusal guarantees hold?

Yes, at the unit level, verified by actually running them (not just reading the file):

```
$ cargo test -p fleet-worker apply::
running 7 tests
test freelane::apply::tests::no_fence_at_all_is_refused ... ok
test freelane::apply::tests::absolute_path_is_refused ... ok
test freelane::apply::tests::fence_with_no_target_is_refused_naming_the_ambiguity ... ok
test freelane::apply::tests::one_bad_file_among_several_applies_nothing ... ok
test freelane::apply::tests::parent_traversal_is_refused_and_nothing_written_outside ... ok
test freelane::apply::tests::symlinked_target_escaping_the_worktree_is_refused ... ok
test freelane::apply::tests::single_fence_with_clear_target_is_applied ... ok
test result: ok. 7 passed; 0 failed
```

This matches the real-binary behavior I observed: every one of my 7 live attempts had a fenced
block with **no** `path:`/`file:` declaration, and in every case `apply` refused (silently — see
§6) rather than guessing a target or writing partial output. I did not get a live case of an
absolute-path or `..` attempt from the model to test `guard.rs`'s containment check against a
real hostile reply (the model never tried that in 7 attempts), so that specific claim is verified
only at the unit level, not against the real binary — noted as untested in §7.

**Separately, and this is the second real defect**: `agent_cmd_run.rs::run_freelane`
(`src/dispatch/agent_cmd_run.rs:27-42`) takes `freelane::run`'s `Ok(out)` and builds the fd-3
`Done` body from `out.response`/`out.log`/`out.resolved_model`/`out.tokens` only —
**`out.applied_files` and `out.apply_note` are computed and then silently dropped**, never
surfaced into the body, never used to choose `Done` vs something else. So even if the pid-file
bug were fixed, today's code has no path by which "apply refused with `AmbiguousTarget`" reaches
the outcome the caller sees; the reply text is the only signal, and it is always wrapped as
`Done` regardless of whether anything was ever applied.

## 6. What I could not test, and why

- **A `Refused` `LaneOutcome` from the live freelane endpoint.** 7/7 attempts came back `Done`
  (including an explicit LLM refusal and an attempted network break). I did not chase this
  further given the budget — the free endpoint is described as rate-limited/variable, but the
  more likely reason I never saw `Refused` is exactly the bug under test: this endpoint has two
  configured lanes (`lane=1/2` in every log line) and any 200 with parseable JSON on either lane
  becomes `Done`. Reaching `Refused` would need both lanes to fail outright (HTTP 4xx/5xx, all
  budget exhausted, or unparseable body) — I did not find a reliable way to force that inside the
  hermetic child's stripped-down env (proxy vars are dropped, see attempt 7).
- **The apply guard's absolute-path/`..`/symlink refusals against the real binary**, only against
  its own unit tests (§5) — the live model never emitted a path-declared fence at all, hostile or
  not, in 7 tries, so I never got a real end-to-end case to throw at `guard.rs`.
- **Whether the same pid-file defeat applies identically to the `Claude`/`Codex` adapters in
  practice** — confirmed by code reading (§3: `record_worker_pid` is adapter-agnostic) but not
  reproduced live, since neither `claude` nor `codex` CLIs are installed in this environment
  (`spawn_join_missing_cli_refuses_before_any_io` in the existing suite exists for exactly this
  reason).
- I did not run `cargo mutants` (excluded by the task's own constraints).

**Aside, not asked for but observed and worth flagging**: mid-session, a separate, apparently
live/concurrent process modified `src/Cargo.toml` and added `src/build.rs`/`src/build_info.rs` in
this exact checkout while I was verifying (`git status --short` went from clean to showing those
changes between two of my commands, timestamps ~07:22-07:23 IST). It never touched anything under
`crates/fleet-worker/` or `crates/fleet-verify/`, so it does not affect any finding above, but a
"fresh worktree, clean state" verification precondition was not actually met for the whole
session — worth noting since the task asked me to start from one.

## 7. Recommendation: is "wrote a file" the right property?

**No — and this investigation is itself the proof.** "Wrote a file" (or here, even weaker: "left
some untracked bytes in the worktree, from any source") is satisfiable by fleet's own
process-bookkeeping without the worker doing anything, which is strictly worse than the
adversarial case the task worried about (a model gaming the check on purpose) — nobody even has
to try. A property this easy to satisfy by accident will not survive contact with reality, per the
project's own stated principle ("a check cheaper to fake than to satisfy will be faked" —
`fleet/PRINCIPLES.md`) — except here it isn't even being faked, it's being satisfied by litter.

A stronger, still-honest property, in order of how much I'd trust each:

1. **Immediate, cheap fix for the hole I found**: exclude fleet's own known artifacts
   (`.fleet-sandbox/`, `.fleet-lane.pid`, and anything else `spawn`/`join` write directly into the
   worktree) from the diff the honesty check counts — the same treatment `.fleet-sandbox/` already
   gets, just extended to the one thing that leaked. This closes the specific hole but keeps the
   property at "some byte moved," which is still weak against a model that writes one throwaway
   file on purpose.
2. **The change touches a file the task named, or a file under a path the task's own words
   plausibly point at.** Requires connecting the task string to the diff, which is fuzzy, but even
   a weak heuristic (task mentions a function name that appears in a changed file, or the task
   names a path directly) is strictly better than "any diff at all," and is cheap to compute from
   information already on hand (task text + `git diff --name-only`).
3. **The change is in a file extension/language the repo already uses** (e.g. reject a diff
   that's 100% new `.py` files in a repo that is otherwise 100% `.rs`) — directly would have
   caught the exact S1 case from the user journey (Python written into a Rust crate), and is cheap:
   one pass over `git diff --name-only` against the repo's existing extension histogram. Doesn't
   require understanding the task at all, which makes it robust to task text being vague or
   adversarial itself.
4. **The strongest, and most expensive: it compiles / the repo's own verify gate still passes (or
   newly passes) after the change** — this is what `fleet gate`/`fleet run`'s own verify stage
   already does elsewhere in this codebase (semgrep/trivy/detectors/corpus gates), so the
   machinery to do this exists; it just isn't wired into `swarm`'s own `Done` classification. This
   is the only one of the four that can't be gamed by writing plausible-looking nonsense, but it's
   also the only one that costs real wall-clock time per lane, which matters at swarm scale.

My recommendation: **do (1) now** — it's a one-line-scope bug fix for a real, currently-100%-open
hole, not a design question. Then **layer (3) on top** as the next honesty tier, specifically
because it is what would have caught the *original* S1 defect (Python-in-a-Rust-repo) even after
(1) is fixed and even if the model DOES eventually learn to emit a `path:`-declared fence — a
model can trivially learn to write `path: src/main.rs` above a Python block, and (3) is the
cheapest check that still catches that. Reserve (4) for a slower, opt-in "verified done" tier
(this codebase already gates `mutants` behind `FLEET_MUTANTS`, so there's precedent for a
budget-gated stronger gate) rather than the default path every `swarm` call takes — the whole
point of the freelane/keyless lane is to be fast and cheap, and (4) trades that away.
