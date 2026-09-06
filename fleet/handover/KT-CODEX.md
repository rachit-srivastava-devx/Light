# KT — fleet-rs handover to codex

You are now the operator of this repo. The human owner is out of quota. Nobody is watching each
tick. **Everything below is load-bearing; the failures listed are ones that already happened here.**

## 1. What this repo is

`fleet-rs` — an agentic SDLC harness. A Rust kernel (`keel/fleet/`) that **refuses**, and a Python
side (`crew/`) that **proposes**. The split is the product: proposals are cheap and fallible,
refusals are typed and non-negotiable.

- Binary: `keel/fleet/` → `fleet`. Build: `cd keel && cargo build --release`.
- Gate: `FLEET_MUTANTS=0 bash verify.sh` — currently **15 passed, 0 failed, 1 skipped (denominator: 16)**.
- Detectors: `tests/corpus/*.sh` — **105**, integrity-sealed by `MANIFEST.sha256`.
- Honesty ledger: `docs/DELTA.md` — **D1–D60**. Every finding, including the ones that make the
  project look bad. This file is the point of the project. Keep writing it.

## 2. Typed exit codes — never invent a new one

`0` ok · `3` environment · `6` invariant violated · `7` refusal · `8` mismatch · `9` SOW_READY_AWAITING_REVIEW.

## 3. The laws that produced the 60 findings

1. **A proxy is not the property.** An API 200 is not a rendered page. A DOM node count is not a
   paint. A green gate is not a working product — `D58`, `D59`, `D60` were all found *past* a green
   gate, in the layer between working code and the person using it.
2. **Publish the denominator.** `"M7: 24 of 24"` is a result. `"M7: pass"` is not. **A check that
   measures nothing must FAIL, not pass vacuously.** `M6` caught its own broken input only because
   it refused instead of passing on an empty set.
3. **null is not 0.** An unmeasured value is `null`. Writing `0` for "we did not measure" produced
   a whole fake dataset once (`D48`).
4. **A check cheaper to fake than to satisfy will be faked.** 447 of 521 receipts once passed the
   core invariant with strings that were not models.
5. **Test both directions.** A gate that refuses everything proves as little as one that refuses
   nothing.
6. **Mutation-test every new guard.** Break the thing the detector watches; the detector must go
   red *and name it*; restore; confirm green. If you did not do this, the detector is not accepted.
   `bash bin/mutants-gate.sh`.
7. **A lint suggestion is a hypothesis, not a patch.** Applying `unwrap_or_else(Vec::new)` →
   `unwrap_or_default()` blindly broke the build with 12 errors (needed a turbofish).
8. **When a fix does not move the number, look for the second code path.** A concurrency fix moved
   5/64 → 7/64 and looked like noise; the real cause was that the *eager* seeding path had been
   left un-atomic while the *locked* one was fixed. Rate then went to 0 of 128.
9. **If the tests fail after your fix, the tests may be right.** A "completeness fix" silently
   re-seeded a deliberately-small 2-agent store and broke two unit tests. The assertions were
   right; the fix was wrong.
10. **Do not grade your own work.** Verify in a clean tree using only documented steps.

## 4. Traps specific to this machine

- **The repo path contains a space** (`.../Principal Engineering/...`). Quote every path. An
  unquoted `$DOCS` has broken this five times.
- **zsh does not word-split unquoted variables** — this bit twelve times. In `bash` scripts you are
  fine; if you hand a command to the user's shell, use arrays or `${=var}`.
- **`$?` after a pipe is the pipe's status.** Capture `rc=$?` on its own line, immediately.
- **`grep` in the agent shell is `ugrep`** with different regex semantics than `/usr/bin/grep`,
  which is what your scripts see. Probe the way the script actually runs.
- **Detectors firing on their own documentation** has happened five times. Skip comment lines.
  Convention in this repo: **backticks mean "this exists"**; quote anything unimplemented.
- **`npm install` from a worktree eats the node_modules symlink.**
- **NEVER create sibling worktrees.** 38 of them / 11GB once accumulated outside the repo. The rule
  is absolute: **whatever happens, happens inside `fleet-rs/`.**

## 5. Working agreement per task

1. Read the backlog item's acceptance criteria. They are the contract.
2. **SOW before code** for anything non-trivial: `fleet sow "<task>"` → exit `9` → `fleet sow accept`.
3. Smallest change that satisfies the criteria. One slice. Do not one-shot.
4. Run `FLEET_MUTANTS=0 bash verify.sh`. Paste the real result including red.
5. New guard? Mutation-test it (law 6) and run `bash bin/detector-integrity.sh --update`.
6. Record what you found in `docs/DELTA.md` as the next `D<n>` — **especially if it is unflattering.**
   The ledger's value is that it contains the failures.
7. Commit with `git -c core.hooksPath=/dev/null commit`. Leave the tree clean.
8. Append one line to `handover/PROGRESS.md`.

## 6. What is honestly NOT done

- **Requirement 5 (junior-with-fleet == senior-without) sits at 45%** and has for six attempts. The
  harness, the pre-registration and the refusals are real and proven. The *experiment* is blocked:
  the free lane is rate-limited, 99 of 128 observations scored zero, so the measured variance is
  service flakiness, not prompter effect. **Do not lower the margin to make it pass.** That is
  precisely what pre-registration exists to prevent. It needs a reliable lane. If none appears,
  leave it at 45% and say why.
- Sustained multi-agent delegation was *declared* and not *practised* — most execution this session
  was direct. You are the correction to that.
