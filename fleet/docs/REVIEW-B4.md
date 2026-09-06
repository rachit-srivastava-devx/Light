# REVIEW — B4 (`fleet plan` intent coverage)

Reviewer: independent verifier, adversarial by default. Worked only from `docs/delta.d/B4.md`,
`handover/BACKLOG.md` item B4, `tests/fixtures/plan-prompts.txt`, `keel/fleet/src/intent.rs`, and
direct execution of the built binary. B4's process (pid 81967) had fully exited before this review
was written.

## What was claimed

25 prompts collected in `tests/fixtures/plan-prompts.txt` (20 in-scope, 5 out-of-scope). Baseline
(before): 17/25 sensible routes, 2/25 refusals-with-useful-candidates, 6/25 useless refusals. After
adding three bounded phrases to the intent table (`introduce a/an`, `available` paired with
quota/tokens, and a question-form-plus-failure-symptom diagnosis rule): 20/25 routes, 0/25 useful
refusals, 5/25 useless refusals, sandwich still refuses. `cargo-mutants` did not reach measurement
(343s isolated-build failure, disclosed as an environment limitation). Final `FLEET_MUTANTS=0 bash
verify.sh`: exit 6, 14 passed/1 failed/1 skipped; `fmt` green after a fix; `corpus` still red,
explicitly said to be outside B4's files. B4 does not claim a green full-repository gate.

Acceptance contract (BACKLOG.md item B4): fixture file exists; before/after numbers are recorded;
sandwich still refuses; `verify.sh` green.

## What I ran

**Independently re-ran the entire fixture against the actual binary**, before knowing B4's claimed
after-numbers in detail:

```
$ while read -r p; do FLEET_STATE=... keel/target/debug/fleet plan "$p"; done < tests/fixtures/plan-prompts.txt
[1..20] all rc=0, sensible intents (implement a change / verify the ledger / show token quota / diagnose what broke)
[21] rc=7 Make me a sandwich          -> candidates: fix the parser bug, diagnose fleet, is the ledger ok
[22] rc=7 Write a poem about deployment
[23] rc=7 What is the weather today?
[24] rc=7 Schedule a meeting with Alex
[25] rc=7 Translate this sentence to French
```
20 routes / 5 refusals, matching B4's claimed after-state exactly, including the identical sandwich
refusal text (same three candidates, in the same order) that B4 quotes. Re-ran the sandwich prompt
a second time on a later invocation — identical output, identical exit 7.

**Caught a live defect in progress, then confirmed it was fixed.** Earlier in this review session,
B4's own transcript flagged `cargo fmt --check` failing on a new test tuple it had just added in
`intent.rs`. I reproduced that failure myself at the time:
```
Diff in .../intent.rs:243: ... &[...][..] wanted multi-line wrapping ...
```
B4's final fragment says `fmt` is green after formatting. I re-ran it independently after B4's
process exited:
```
$ cargo fmt --manifest-path keel/Cargo.toml --all -- --check
(no output)
fmt_exit=0
```
Confirmed fixed.

**Sanity-checked the "before" number logically**, since I do not have an untouched pre-B4 binary to
replay against (the shared `keel/target/debug/fleet` had already been rebuilt with B4's change by
the time I first tested it — a genuine limitation of reviewing concurrent workers with no git
access). The claimed before/after delta is internally consistent: 17+2+6=25 and 20+0+5=25; the two
named "useful refusal" recoveries (`introduce a dry-run mode`, `quota ... available`) map exactly to
the two new phrases added; the third recovered prompt (`why did the last dispatch fail`) requires
literally the "question form + failure symptom" rule B4 describes, since the old keyword list
(`failing`,`failure`,`broken`,`diagnose`,`diagnostic`,`what broke`) does not contain the bare
substring `fail` that this prompt ends in. This is not independent proof of the exact pre-change
binary's behavior, but it is a real, checkable derivation from the diff, not just trust in B4's
prose.

**Confirmed the underlying tests exist and pass**, not just claimed:
```
$ cargo test --manifest-path keel/Cargo.toml --quiet intent::
running 3 tests ... test result: ok. 3 passed; 0 failed
```
and the three new phrasings appear verbatim as rule examples/test fixtures in `intent.rs`
(lines 100, 244, 253, 258).

## Findings

1. **`verify.sh` is not literally green**, and the acceptance criterion says it must be. The one red
   stage is `corpus`, which B4 attributes to files outside its slice. I have independent evidence
   this is plausible: I ran `tests/corpus/run.sh` standalone under the same four-way-concurrent load
   and got the same class of failure — `TIMEOUT *.sh exceeded 30s` at 13% average CPU utilization,
   i.e. a process mostly waiting on lock/IO contention, not a logic failure. I could not get a fully
   quiescent tree in the review window to prove this conclusively, so this criterion is **not yet
   satisfied as written**, even though the cause looks environmental rather than a B4 regression.
2. The "before" baseline is B4's self-report, not independently replayed against a preserved
   pre-change binary (see above) — logically consistent, but not the same strength of evidence as
   the after-state, which I reproduced byte-for-byte.
3. No scope violations found: B4 touched `tests/fixtures/plan-prompts.txt`, `keel/fleet/src/
   intent.rs`, and its own `docs/delta.d/B4.md`. `handover/BACKLOG.md` and `docs/DELTA.md` mtimes
   are unaffected by B4 (the BACKLOG.md changes present in the tree belong to a different, out-of-
   scope process that appended items B8/B9, not to B4).
4. The verb table was NOT widened past the sandwich case — the explicit regression guard holds,
   confirmed twice by direct execution.

## Verdict

**ACCEPT-WITH-FINDINGS**

The actual feature work is solid and independently reproduced exactly: the fixture exists, the
after-state numbers are real (I got the identical split and the identical refusal text on a fresh
run), the sandwich regression guard holds, and the one contention-attributable formatting defect I
caught live was fixed before the worker stopped. This is not a clean ACCEPT because the literal
acceptance bar — `verify.sh` green — is not met at the time of this review, and B4 says so itself
rather than claiming otherwise. Before this item is marked `[x]`: re-run `verify.sh` (or at least
`tests/corpus/run.sh`) alone on a quiescent tree and confirm `corpus` passes once the other workers'
concurrent verify.sh runs are no longer competing for the build lock and CPU.
