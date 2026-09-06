# Observed, live session 2026-08-28 — not yet Candidate (no fixture/pattern authored)

Per SCHEMA.md's funnel: these are raw `Observed` rows. None have a `pattern_regex` or fixture yet
— that's the gap between "we noticed this" and "a gate refuses it," and it's honest to leave them
here rather than call them Promoted before that work exists.

1. **`semgrep --config=auto` silently resolves zero rules without live network to semgrep.dev.**
   `--metrics=off --config=auto` errors outright ("cannot create auto config"), but a bare
   `--config=auto --quiet` run can exit 0 with zero findings and zero indication whether it ran
   any rule at all — indistinguishable from "clean." Same measured-nothing shape as
   `policy/measured_nothing.rego` already refuses elsewhere, just in a tool this repo didn't build.
   Mitigation used: pinned rulesets (`p/rust`, `p/secrets`, `p/security-audit`) instead of `auto`.

2. **A path threaded as a function parameter can still be shadowed by an env-var read deeper in
   the call chain.** `resolve_run_agent(..., state: &Path, ...)` correctly threads `state` into
   `route::for_plan`, but the refusal-receipt write inside it calls `append_receipt()`, which
   resolves the ledger via `ledger_paths()` reading `FLEET_STATE` directly — not the `state`
   parameter. In production the two agree (both trace to `state_dir()`); in an isolated unit test
   with `FLEET_STATE` unset, this surfaced as `EXIT_ENV` where a naive test expected
   `EXIT_REFUSAL`. Neither was a bug — the test's assumption was too narrow. Lesson: when a
   function takes an explicit path parameter, grep everything it calls for whether they ALL
   actually use that parameter, or whether one silently prefers ambient state.

3. **Caught mid-session, live: piped `| head` before reading `$?` is E1, done by the author of the
   E1 lesson.** Ran `fleet swarm dispatch ... 2>&1 | head -5; echo "rc=$?"` to check an exit code —
   `$?` was `head`'s exit status, not fleet's. AGENTS.md rule 3 names this exact pattern and this
   session's own recur-gate (`E1-pipe-exit-code.json`) exists to catch it in shipped code — it does
   not (and structurally cannot) catch it in an interactive command typed straight into a shell,
   which is exactly the class this session's own lesson store cannot reach (12-SELF-LEARNING.md's
   honest 42.6% judgement fraction, applied to the operator's own typing, not just committed code).

4. **`diff -u`'s `+++` header differs from `git diff`'s.** Raw POSIX `diff -u` appends a tab +
   file-modification-timestamp after the path on the `+++` line; `git diff` does not. A gate's
   file-matcher regex written against one format silently fails to match hunks from the other. Cost
   one real miss while first testing `bin/recur-gate.sh` against a synthetic `diff -u` fixture —
   caught only because the SAME test was re-run against a real `git diff` and gave a different
   (correct) answer. Lesson: test a diff-scoped gate against the diff tool it will actually receive
   input from in production, not a superficially-similar one.

5. **A newly-edited file that was never run through `cargo fmt` fails `verify.sh`'s `fmt` stage on
   the FIRST post-edit run, even though `cargo check`/`cargo test` were already green.** Green
   compile + green tests is not green `verify.sh` — `fmt` is a separate, silent-until-run check.
   Cost one avoidable red stage this session. Mechanical fix, but worth naming: run `cargo fmt`
   as the LAST step of any Rust edit, not an afterthought triggered by a red gate.
