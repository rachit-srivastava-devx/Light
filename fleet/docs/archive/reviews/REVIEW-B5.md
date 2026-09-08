# REVIEW — B5 (REPL parity with the claude-code CLI)

Reviewer: independent verifier, adversarial by default. Worked only from `docs/delta.d/B5.md`,
`handover/BACKLOG.md` item B5, `keel/fleet/src/repl.rs`, and direct, live execution of the compiled
REPL — not the unit tests alone. B5's process (pid 82126) had fully exited before this review was
written. Note: an automated companion review pass for this same ID (`var/loop/review-B5.log`) hung
on stdin twice and produced no `docs/REVIEW-B5.md` of its own; this review is the only one for B5.

## What was claimed

Three fixes, each marked `[FIXED]` in `docs/delta.d/B5.md`: (1) streamed child output could drop
its tail because `drain_operation` finalized on child-exit before reader threads drained; (2)
Ctrl-C did nothing while a plan was pending confirmation; (3) the input line was append/backspace
only, with no Left/Right/Home/End/Delete or mid-line insertion. Two more gaps were *found but not
fixed* and disclosed as such (history narrower than Claude's; already-adequate slash/refusal
coverage). Proof cited: three named unit tests in `repl.rs`. `tests/acceptance/*` was deliberately
not touched, citing a repository rule against editing it. Final verify.sh: exit 6, only `corpus`
red, attributed to concurrent load.

Acceptance contract (BACKLOG.md item B5): the defect list is recorded with the 3 fixed ones marked;
each fix has an assertion in `tests/acceptance/`; `verify.sh` green.

## What I ran — I did not trust the unit tests; I drove the actual binary through a real pty

Unit tests confirm the internal logic; they do not prove a real terminal, in raw mode, sending real
bytes through a real `crossterm` event loop, actually reaches that logic. I wrote a Python
`pty`-based harness (`/private/tmp/.../scratchpad/repl_pty_test.py`) that spawns the built
`keel/target/debug/fleet` on a real pty, types real keystrokes, and reads the real byte stream back.

**Ctrl-C during a pending plan — real session, not synthetic KeyEvent:**
```
type "verify the ledger", Enter  -> screen renders:
  PLAN · intent: verify the ledger
  commands (in order): 1. fleet ledger verify
  ...
  Enter to run · e to edit the prompt/plan · Esc to cancel
send Ctrl-C (0x03) -> screen changes to:
  PLAN READY [Entr run · e edit · Esc cancel]
  plan cancelled; no command was run
```
The exact message `cancel_plan()` writes appeared, and no `fleet ledger verify` execution output
(`checked=...total=...`) appeared anywhere in the 7.7KB raw capture. Confirmed: fix #2 works
end-to-end in a real terminal, not only in the unit test.

**Home + insert-at-cursor line editing — real session:**
```
type "erify the ledger" (deliberately missing the leading v)
send Home (ESC[H)
type "v"
Enter -> echoed prompt and PLAN header both read "verify the ledger", not "erify the ledgerv"
```
Confirms the cursor genuinely moved to column 0 and the insertion happened there, not at the end of
the buffer. Fix #3 confirmed live.

I did not attempt a live reproduction of fix #1 (the stream-EOF race) — forcing that exact timing
non-deterministically in a black-box pty session is impractical in the time available. I instead
read the fix directly: `drain_operation` now tracks `operation.streams_remaining` (decremented only
on `StreamEvent::Closed`) and will not finalize the outcome/output until the child has exited *and*
`streams_remaining == 0`. That is a real, targeted fix for the described race, not a cosmetic
change, and the new unit test (`completed_child_waits_for_stream_eof_before_dropping_tail`, visible
in the final lines of `codex-B5.log`) exercises exactly that ordering with a channel sender.

**Confirmed the 3 named tests exist and pass, independently:**
```
$ cargo test --manifest-path keel/Cargo.toml repl::
test repl::tests::line_editing_keeps_cursor_and_history_consistent ... ok
test repl::tests::ctrl_c_cancels_a_pending_plan_before_execution ... ok
test repl::tests::completed_child_waits_for_stream_eof_before_dropping_tail ... ok
test result: ok. 6 passed; 0 failed
```

**Checked the `tests/acceptance/` claim directly rather than accepting the stated rule.** Current
directory: still only `p0.sh`, `readme.sh`, `swarm.sh` — B5 did not add a file. I read the actual
per-file headers. Only `swarm.sh` carries "LEAD-AUTHORED. Builders MUST NOT edit **this file**." —
that is a per-file marker on three specific, pre-existing blind-suite files, not a directory-wide
prohibition. `readme.sh` itself is proof a *new* acceptance file is the normal way this repo adds
acceptance coverage: its own header says it was written because "`D39` put a planning gate in front
of `run`... nothing tested it, so nothing noticed," i.e. it was added by a past builder for exactly
this kind of gap, not handed down pre-written. B5's justification for skipping `tests/acceptance/`
entirely does not hold up against the repo's own precedent.

## Findings

1. **Acceptance criterion not met, and the reason given is incorrect.** "Each fix has an assertion
   in `tests/acceptance/`" was not delivered. The rule B5 cites forbids editing the three existing
   files, not adding a fourth. A `tests/acceptance/repl.sh` that drives the built binary through a
   pty (the pattern I used above is a working template) and asserts the Ctrl-C-cancels and
   line-editing behavior black-box would satisfy this and is exactly what the acceptance-level bar
   in this repo is for — proving it from outside the crate, the way `readme.sh` proves the
   quickstart from outside the crate.
2. **`verify.sh` is not green** (exit 6; `corpus` red). Same failure class as B1/B4/B6 in this
   session — I independently reproduced `TIMEOUT *.sh` corpus failures running the corpus stage
   alone under the same concurrent load, at 13% average CPU (waiting, not computing). Plausibly
   environmental, not a `repl.rs` regression, but not proven clean on a quiescent tree in this
   review window.
3. **The "least self-code, most stitching" constraint (prefer rustyline/reedline) was knowingly not
   followed**, with a stated, defensible reason: those crates are line-editors for a scrolling
   terminal, and this REPL is a ratatui full-screen alternate-buffer app — swapping in rustyline
   would mean dropping the existing plan-preview/queue UI, which is a materially larger change than
   "fix the top 3 defects." This is a disclosed trade-off, not a hidden shortcut, but it is a real
   deviation from a standing owner constraint and should be called out as unresolved, not quietly
   accepted.
4. No scope violations: only `keel/fleet/src/repl.rs` and `docs/delta.d/B5.md` were touched.

## Verdict

**ACCEPT-WITH-FINDINGS**

The three fixes are real. I did not take the unit tests' word for it — I drove the compiled binary
through a real pty and watched two of the three behave exactly as claimed in an actual terminal
session, and read the third's implementation directly. The defect list is honest and includes gaps
B5 chose not to fix, correctly disclosed rather than omitted, and B5 correctly reported its own
verify.sh as red rather than green.

It is not a clean ACCEPT: the literal "assertion in `tests/acceptance/`" requirement was not met,
and the justification given for skipping it is contradicted by this repo's own precedent
(`readme.sh`). Before this item is marked `[x]`: add a `tests/acceptance/` script that drives the
built binary (pty or equivalent) through the Ctrl-C-cancel and line-editing paths from outside the
crate, and re-confirm `verify.sh` green on a quiescent tree.
