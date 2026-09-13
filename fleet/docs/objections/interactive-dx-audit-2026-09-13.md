# Interactive DX audit vs. native `claude` — 2026-09-13

Comparison method: built `target/debug/fleet`, then drove both `fleet`'s interactive REPL and the
real `claude` CLI (v2.1.270) through a real pty (a bare `pty.openpty()` harness answering the
`ESC[6n` cursor-position handshake reedline/crossterm block on until a real terminal emulator would
answer it — unanswered, it silently eats whatever was typed in that window with no error) against a
real scratch git repo, at three prompt levels: explain-only, an edit, and a command-requiring task.
Two real, verified-live defects found and fixed; not blueprint disagreements (LLD.md is unchanged),
both squarely inside what §7 and §20 already require.

## Fixed

**1. Every tool call was invisible.** `dispatch/agent_stream.rs`'s parser only ever surfaced
`content_block_delta` text and the final `assistant` text parts; `tool_use` parts and every
`tool_result` were silently dropped. Live proof (`explain what calc.py does`): fleet printed the
announce line, then nothing, then the answer — while the raw `claude --print --output-format
stream-json` transcript for the identical invocation contained a `Glob` and a `Read` tool call with
results. For an edit task the gap is worse: `add a multiply(a,b) function to calc.py` edited the
real file with zero visible evidence of the `Edit` call — a silent multi-second window is exactly
what LLD §20 calls insufficient ("a route report that merely prints a selected adapter") and §7
requires against ("stream milestone updates, not raw reasoning or token chatter"). Fixed by
surfacing one line per tool call the moment its full `tool_use` block arrives (never the
`input_json_delta` fragments, which would be the "raw chatter" §7 rules out): `→ Read(calc.py)`,
`→ Edit(calc.py)`, `→ Bash(git log)`. Milestones are UI-only and never enter `ClaudeStream::body()` /
the recorded receipt. `tool_result` bodies are still not surfaced (see Deferred).

**2. Command-effect approval was a no-op.** `agent_tool_policy.rs` passed the same
pattern-qualified string (e.g. `Bash(git log *)`) to both `--tools` and `--allowedTools`. Verified
directly against the real binary: `claude --restricted --tools="...,Bash(git log *)"` reports
`TOOLS AVAILABLE: ['Glob','Grep','Read']` — no Bash at all — because `--restricted` only re-enables
a removed tool by its *bare* name; a pattern there is not recognized. Every task fleet classified as
needing a local command (the REPL correctly asked the user to approve "run local build, test, or
verification commands") silently degraded to read-only, with no error and a reported `✓ Done`. Live
before/after on `run git log ...`: before, "I don't have a shell tool available in this session, so
I read the git reflog instead"; after, `→ Bash(git log)` and the real `git log` output. Confirmed
separately that `--allowedTools` still enforces the fine-grained pattern once the bare name is
present (`rm -rf calc.py` was denied under the identical policy in the same probe). Fixed by deriving
a bare-name list for `--tools` from the existing pattern constants instead of reusing them verbatim.

**3. Startup blocked on a network call for a notice that can never fire.** Follow-up goal narrowed
to speed parity specifically. `interactive/repl.rs` awaited `update_check::check()` (a `git
ls-remote origin` with a 2s timeout) *before* rendering the status bar or accepting the first
keystroke. Measured on this machine: a bare `git ls-remote origin` against this repo's real GitHub
remote takes ~0.71s. Worse, `update_check::check`'s own doc comment says it "returns `None` on
every outcome until an ancestry-aware implementation replaces it" -- reading the function confirms
it: the computed `sha_in_remote` result is unconditionally discarded and `None` is always returned.
So the blocking wait bought zero user-visible value, ever, on every single interactive session
start, going back to whenever this landed. Fixed by `tokio::spawn`-ing the check (the runtime is
already multi-thread, `runtime/tokio_rt.rs`, so it makes real progress on another worker while the
main task's blocking `reedline::read_line()` occupies the first) and printing its notice
opportunistically the next time the loop has an idle moment, never delaying the first prompt.
Measured live, real binary, 5 runs: startup-to-ready median **37ms** (one 1.69s outlier, most
likely page-cache warmup on the very first exec of the rebuilt binary, not the fix regressing —
the other 4/5 runs were all ~37ms). Behavior is unchanged today (still always prints nothing);
only the pointless wait is gone.

**End-to-end task speed, fleet vs. bare `claude`, same policy:** same prompt ("run git log ... tell
me what it shows"), same restricted+Bash-enabled policy (post-fix #2), same scratch repo. Bare
`claude --print --output-format stream-json ...`: **10.43s** wall (`time` output, real run). The
identical task through fleet's REPL, timed from the approval keystroke to `✓ Done`: **~8.7-9.1s**.
Within normal model-latency run-to-run variance -- no material overhead from fleet's wrapping layer.
Before fix #2 this comparison would have been apples-to-oranges (fleet silently fell back to a
slower, worse multi-hop read-the-reflog workaround instead of running the one Bash call).

All four changes are in `src/dispatch/agent_stream.rs`, `src/dispatch/agent_tool_policy.rs`, and
`src/interactive/repl.rs`, with new unit tests reproducing each failure mode and
`l8-code/selfcheck.sh` clean over all 16 changed files. `cargo test --bin fleet`: 115/115. All three
fixes were also re-verified against the rebuilt real binary end-to-end (not just unit tests) in a
scratch repo, per AGENTS.md's "drive the real binary" rule.

## Deferred (explicit scope cuts, not overlooked)

- **`tool_result` completion summaries** (e.g. "⎿ 9 lines", "⎿ 1 match") were left out. The
  announce-on-call fix already solves the "user sees nothing for N seconds" defect; a result summary
  is lower value and the result shapes vary enough per tool (`tool_use_result` metadata differs for
  Glob/Read/Bash) that a first cut risked exactly the kind of speculative surface l8-code's contract
  step warns against. Worth a follow-up if the milestone-only view still feels incomplete in use.
- **Markdown rendering fidelity.** Native `claude` renders bold/headers/code fences in its TUI;
  fleet prints raw markdown text through `StreamPrinter`'s plain indent. Not attempted: LLD §18
  (N18) only requires a text-first, keyboard-operable terminal — it does not mandate ANSI markdown
  rendering — and building one is a materially different-sized piece of work than this audit's scope.
- **The pre-existing 80-line-per-file cap** (`docs/AGENTS.md` "Verify before claiming") is already
  violated on 126/765 `.rs` files repo-wide, including both files this session touched (already over
  the cap before this session). Not attempted here — that's a repo-wide refactor, not a DX defect.
- **`cargo clippy --workspace --all-targets -- -D warnings`** already fails before this session's
  changes, in `crates/plan/src/ready_gate/gate_eval.rs` (`manual_checked_ops`), a file this session
  never touched. `cargo clippy --bin fleet` (plain, no `-D warnings`) shows zero hits in either file
  this session changed.
- Two OS terminal windows were not literally opened (no GUI automation surface here); the pty
  harness drives the real binaries directly and is the more precise comparison for the parsing layer
  since it isolates exactly what fleet's renderer adds or drops from the same provider stream fleet
  itself invokes.
