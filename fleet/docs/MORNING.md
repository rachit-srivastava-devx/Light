# MORNING — read this first

One page. Full evidence in `docs/NIGHT-PROGRESS.md`, work order in `docs/NIGHT-PLAN.md`, the
user-perspective audit in `docs/DX-AUDIT.md`.

## Try it

```bash
fleet --help                 # 28 commands, all described; 8 say NOT IMPLEMENTED and why
fleet doctor                 # cargo/git + measured capacity, works even on a loaded machine
fleet doctor --json          # named-field JSON
fleet gate --id detectors    # a gate that really runs
```

`fleet` on your PATH is now this binary. It previously pointed at the **predecessor repo's shell
script** (symlink dated Aug 24), so every `fleet` you typed ran the old project. The old link is kept
at `~/.local/bin/fleet.predecessor.bak`. Per your instruction, every install now displaces whatever
is named `fleet` (displaced file preserved once, never clobbered).

## State

```
cargo test --workspace --no-fail-fast   → 450 passed / 0 failed   (was 351 at session start)
cargo clippy --all-targets -- -D warnings → exit 0
files over 80 lines                     → 0
selfcheck                               → 1 FAIL / 238 checked (known D2 false positive on a
                                          deliberate bad-mktemp FIXTURE; D2 is a line-match and
                                          does not honour justification comments)
```

## The two that mattered most

**`fleet rollback` deleted arbitrary directories and reported success.** Confirmed by exploiting it:
a scratch dir with a file in it, `--worktree /tmp/victim` → `ok: worktree removed`, exit 0, file and
directory gone. The recursive-delete fallback ran on the unvalidated CLI string with the error
swallowed by `let _ =`. Now guarded to `<repo>/.worktrees/` with both sides canonicalised (a bare
`starts_with` accepts `../../../etc`), and the legitimate path still works — both directions proven.

**`fleet` on PATH was the predecessor**, and `install.sh` could never have fixed it: it looked for the
built binary in `keel/target/release/fleet`, and `keel/` was deleted earlier in the session.

## Everything fixed tonight

| what | evidence |
|---|---|
| `__agent` child dispatch (your original bug) | real binary, no "unrecognized subcommand" |
| `fleet rollback` arbitrary deletion | exploit reproduced, then refused, exit 7 |
| `run`/`oracle`/`gate` hung forever | bounded budget; was exit 124/zero bytes, now 6–8s with output |
| `swarm --task` rejected as empty | cross-wire fixed; `outcome: Done` |
| Ledger append O(n²) | 4k appends 49.19s → 11.67s; ratio now ~2x/doubling, asserted by a test |
| Corpus detectors `0 of 25` proven | now `25 of 25`; the meta-check had measured nothing since the migration |
| Empty help on all 28 commands | every one described; 8 stubs say *why* they are stubs |
| Test suite load-dependent | 443 tests pass at load 28.8 (gate threshold is 16) |
| `CursorStore` had no durable impl | real atomic-rename `FileCursorStore` |
| LLM-as-a-judge absent | new `fleet-judge` crate, 8/8 offline, **90% on 50 human labels** |
| Flat unreadable output | structured, `next:` leads, denominators published, `NO_COLOR` honoured |
| Docs described a CLI that didn't exist | every example executed; new `QUICKSTART.md` |
| `FLEET_STATE` in all docs | real var is `FLEET_STATE_DIR`; silently did nothing |
| `fleet-memory` imported by nothing | now 7 files in `src/`; a lesson changes a later `sow` |
| `adjudicate` was a stub | wired to `fleet-judge`; errors clearly with no model, never fakes a verdict |
| Suite red when the free endpoint throttled | the 2 live-network tests are now opt-in `#[ignore]` |
| **`swarm` reported Done for changing nothing** | now `Refused` + **exit 7**, `changed_files: 0` in the receipt |
| `impact --repo` silently used the cwd | accepts `--repo`; proven `1` vs `11` on two repos |

## The end-to-end journey (`docs/USER-JOURNEY.md`)

A Sonnet ran all 10 steps as a new user, with **aider** making the code change. The crux question —
*can fleet verify a change made by a coding agent?* — is **YES**: aider added a documented
`multiply()` plus a test, and `fleet gate --id "unit tests"` re-ran `cargo test` and reported `2/2`,
picking up the test that had not existed a minute earlier.

That journey also found the night's worst defect, which no per-command audit could have caught:
`fleet swarm` returned `outcome: Done` with 473 tokens billed for a task that **changed nothing** —
the worker produced a tutorial about doc comments in seven languages and fleet filed it as completed
work. Now fixed: the receipt says `Refused`, records `changed_files: 0`, and exits 7. The adapter is
still chat-only (it advises rather than applies) — that is now reported truthfully rather than
dressed up as success, and porting keel's fenced-code-apply step is the follow-up that would make
`swarm` actually change code.

## Judge benchmark — the real number

```
correct/attempted/total = 45/48/50   (abstained=0, errored=2)
every mistake is the same one: form -> structured_api (3 of 3)
```
93.8% of attempted, 90% of total. The 2 non-answers were rate-limit exhaustion, counted as errors
rather than wrong answers. I did **not** tune the criteria wording to raise the number — that
one-sentence experiment is the obvious next step and is deliberately left for you, because tuning
and then reporting only the improved figure is the dishonesty this repo exists to prevent.

## Needs you

1. **`git rm -r -f fleet/docs/pdfs`** — 179 files, 34MB, zero PDFs, tracked at `HEAD` under `tmp/`,
   referenced by nothing. The permission classifier blocked me twice. Recover with
   `git checkout HEAD -- fleet/tmp/pdfs`.
2. **`Principal Engineering/` is not a git repository**, so `registry/services/llm-gateway` is
   tracked by nothing and unreachable from any clone. That package also has **no build script** while
   its `exports` map points at four `.js` files nothing can produce. Phase 0 — everything about the
   conversation path is a claim about a fake `memory` adapter until it is fixed.
3. **`fleet-rs`: 364 of 570 commits** are an unattended loop committing `verify RED, findings kept,
   code not committed`. Four of seven "active" days contain zero real commits. Six task tags each
   retried ~73 times and never gave up. Fix the loop's give-up condition, don't rewrite history.
4. **`fleet-merge::remove()`** returns `Err(NotFound)` for an already-removed worktree, breaking the
   documented idempotence contract. Your ruling needed.
5. **Nothing is committed.** I made no commits; `main` was not created and nothing was pushed. The
   tree is large and green — review `git status` before committing.

## Known-open, honest

- `fleet gate --id corpus` hangs through the binary (exit 124) while `bash gates/corpus/run.sh`
  finishes in ~2s — the difference is the binary's own `RealRunner` plumbing.
- `recur` and `trivy` gates fail for a materialised-gates-root missing-input reason, not a defect.
- Context-engineering budget enforcement is proven only against synthetic strings, never real prose.
  Gutenberg was confirmed reachable and deliberately **not** used — do not read "tested on real data"
  into it.
- `fleet-memory/src/gate_check.rs:23` leaks via `Box::leak` per distinct category. Bounded by the
  no-daemon tenet today; a real bug the day anything long-running calls it.
- The judge benchmark makes 50 sequential calls with no per-call timeout; it needs one.
- `ledger --json` emits `"checked": null`; honest, but `Number(null) === 0` in JS has bitten this
  repo before. `skip_serializing_if` would be safer.

## Two operational lessons worth keeping

- **A test seam that substitutes the binary hides missing entry points.** 351 tests were green while
  `fleet __agent` did not exist, because every test swapped in a fake child. Real-binary tests now
  use `env!("CARGO_BIN_EXE_fleet")`, and one test asserts the fake seam is unset.
- **Agents stall waiting on their own background jobs** — three did tonight, ~470k tokens, all with
  the work already on disk. Verify their output independently and stop them; do not re-brief. And
  stopping a parent kills its background `cargo` children, which silently produced 0-byte results
  twice.
