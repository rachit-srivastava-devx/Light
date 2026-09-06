# Adversarial review — B13

Date: 2026-09-02  
Deliverable: `docs/delta.d/B13.md`  
Contract: `handover/BACKLOG.md:443-457`  
Verdict: **REJECT**

## Outcome first

B13's main retry mechanism is real: a first-pass timeout is retried, a persistent hang remains a
failure, a non-hanging broken detector remains a failure, timeout-only input does not pass, and an
empty corpus refuses. The change is still rejected because it violates a hard repository invariant
and its claimed timeout denominator is not exhaustive:

1. `tests/corpus/run.sh` returns invented/untyped exit codes `1` and `5`. `AGENTS.md:19-21`,
   `handover/KT-CODEX.md:18-20`, and `verify.sh:18-20` define the allowed gate-wall types; `5` is
   not one of them, and the gate wall does not preserve a child environment fault separately.
2. A detector that times out on the first pass and returns `77` on retry is counted only as
   `excluded`. The final line reports every `timeout_*` counter as zero even though a timeout was
   observed. This falsifies the claim that the three timeout buckets form a full breakdown.
3. The cited `ps aux | grep -E "verify\.sh|corpus|cargo (build|test)|cargo-mutants"` command cannot
   prove an uncontended run here: it matched its own grep, the invoking shell, and parent Codex
   processes whose argv contains the review prompt. The document's claim that this command was
   "empty" is not reproducible evidence.

The required independent verifier is red now: **16 passed, 2 failed, 1 skipped, denominator 19,
exit 6**. Both `swarm` and `corpus` failed. The B13 document did not falsely call its own verifier
green, but its pasted 17/1/1 snapshot is not the current result.

## Acceptance contract versus observed behavior

| Contract check | What I ran | Observation | Result |
|---|---|---|---|
| Timeout reported distinctly from caught regression | Synthetic timeout-then-pass, timeout-then-fail, persistent-hang, and timeout-then-`77` fixtures using a byte-identical copy of the real `run.sh` | The first three get distinct labels. Timeout-then-`77` has only the provisional line and disappears from all final timeout counters. | **FAIL** |
| Gate still fails on timeout | Pure timeout-then-pass fixture | `caught=0`, `timeout_contention=1`, non-zero exit `5` | Functional pass, **typed-exit FAIL** |
| Denominator published | Empty, synthetic, real corpus, and full verifier runs | A denominator prints on completed verdict paths, but timeout-then-`77` produces false zero timeout counters. | **FAIL** |
| Genuinely hanging detector still caught | `sleep 999` fixture under a 2-second limit | `TIMEOUT-PERSISTENT`, `caught=2` in combined run, exit `1` | Functional pass, **typed-exit FAIL** |
| Genuinely broken detector still caught | Immediate `exit 1` fixture | Printed as a plain failure, not a timeout; counted in `caught`; exit `1` | Functional pass, **typed-exit FAIL** |

## Commands actually run

No `git` command was run. No acceptance test, contract, deliverable, backlog, `DELTA.md`, or file
under `keel/` was edited.

### 1. Current detector integrity

```text
$ bash bin/detector-integrity.sh
detector-integrity: 105 detectors match the manifest (denominator: 105)
INTEGRITY_EXIT=0
SCRIPT_COUNT=105 MANIFEST_COUNT=105
```

The cited mutating command was exercised only in an isolated `mktemp -d` layout so this review did
not rewrite the shared checkout. The copied script's SHA-256 matched the repository script.

```text
$ bash bin/detector-integrity.sh --update
detector-integrity: manifest updated (1 detectors)
UPDATE_EXIT=0
$ bash bin/detector-integrity.sh
detector-integrity: 1 detectors match the manifest (denominator: 1)
CHECK_EXIT=0
```

This proves current integrity and update behavior. It cannot prove the historical claim that no
detector script was touched; the review brief forbids using Git, so no historical diff was taken.

### 2. Documented two-direction mutation run

The scratch `run.sh` SHA-256 was identical to `tests/corpus/run.sh`:

```text
REPO_RUN_SHA256=bb14f7540e0e8f57d81e950c1f87744dd5fa1604692aa10da3a7df26ce9f7411
COPY_RUN_SHA256=bb14f7540e0e8f57d81e950c1f87744dd5fa1604692aa10da3a7df26ce9f7411
```

The deliverable says the fixture scripts were pasted "in full", but `ZZ-CONTENTION.sh` is only
described, not pasted. I independently reconstructed the stated marker-file behavior and ran the
cited command:

```text
$ FLEET_DETECTOR_TIMEOUT=2 bash run.sh
ZZ-BROKEN: assertion failed (this is the fixture's intended failure)
  TIMEOUT ZZ-CONTENTION.sh exceeded 2s on the main pass -- provisional, queued for one serial retry
  TIMEOUT ZZ-HANG.sh exceeded 2s on the main pass -- provisional, queued for one serial retry
-- retrying 2 timed-out detector(s) serially, one at a time, once each --
ZZ-CONTENTION: clean on this invocation
  TIMEOUT-CONTENTION ZZ-CONTENTION.sh -- timed out on the main pass, PASSED CLEAN on serial retry (consistent with CPU contention, not a broken detector). Still fails the gate; rerun uncontended to confirm before trusting this label.

  TIMEOUT-PERSISTENT ZZ-HANG.sh -- timed out again on an uncontended serial retry: a genuinely hanging detector. Counted as a caught regression.
DENOMINATOR checked=3 total=3 excluded=0 caught=2 timeout_contention=1 timeout_confirmed=0 timeout_persistent=1
EXIT_CODE=1
```

Arithmetic: `checked=3` is one plain broken detector + one retry-clean detector + one persistent
timeout. `caught=2` is one plain broken detector + one persistent timeout. The two first-pass
timeouts equal `1 contention + 0 confirmed + 1 persistent`. This case reconciles.

### 3. Pure contention-flavoured timeout

```text
$ FLEET_DETECTOR_TIMEOUT=2 bash run.sh
  TIMEOUT ZZ-CONTENTION.sh exceeded 2s on the main pass -- provisional, queued for one serial retry
-- retrying 1 timed-out detector(s) serially, one at a time, once each --
ZZ-CONTENTION: clean on this invocation
  TIMEOUT-CONTENTION ZZ-CONTENTION.sh -- timed out on the main pass, PASSED CLEAN on serial retry (consistent with CPU contention, not a broken detector). Still fails the gate; rerun uncontended to confirm before trusting this label.
DENOMINATOR checked=1 total=1 excluded=0 caught=0 timeout_contention=1 timeout_confirmed=0 timeout_persistent=0
EXIT_CODE=5
```

The no-silent-pass requirement works. The exit code does not: `5` is absent from the repository's
typed exit-code contract.

### 4. Empty-input anti-vacuity probe

```text
$ FLEET_DETECTOR_TIMEOUT=2 bash run.sh
DENOMINATOR checked=0 total=0 excluded=0 caught=0 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0
EXIT_CODE=6
```

The runner does not pass vacuously.

### 5. Omitted hard case: timeout, then exclusion

I added two scratch fixtures: one timed out first and returned `1` on retry; the other timed out
first and returned `77` on retry.

```text
$ FLEET_DETECTOR_TIMEOUT=2 bash run.sh
  TIMEOUT ZZ-TIMEOUT-THEN-BROKEN.sh exceeded 2s on the main pass -- provisional, queued for one serial retry
  TIMEOUT ZZ-TIMEOUT-THEN-EXCLUDED.sh exceeded 2s on the main pass -- provisional, queued for one serial retry
-- retrying 2 timed-out detector(s) serially, one at a time, once each --
ZZ-TIMEOUT-THEN-BROKEN: assertion failed after retry
  TIMEOUT-CONFIRMED-CAUGHT ZZ-TIMEOUT-THEN-BROKEN.sh -- timed out on the main pass, and FAILED (not merely timed out) on serial retry: counted as a caught regression.
NOT MECHANISABLE: synthetic retry exclusion
DENOMINATOR checked=1 total=1 excluded=1 caught=1 timeout_contention=0 timeout_confirmed=1 timeout_persistent=0
EXIT_CODE=1
```

Observed first-pass timeouts: `2`. Published timeout classifications:
`0 + 1 + 0 = 1`. The second timeout is absent from every timeout counter. `excluded=1` explains the
eligible detector denominator, but it does not make the published timeout breakdown complete. The
`0` counters are therefore not an honest count of all observed timeout outcomes.

### 6. Claimed uncontended-process check

```text
$ ps aux | grep -E "verify\.sh|corpus|cargo (build|test)|cargo-mutants"
... codex exec ... review prompt containing corpus ...
... grep -E verify\.sh|corpus|cargo (build|test)|cargo-mutants
... invoking shell containing the same command ...
PS_PIPE_EXIT=0
```

The command is self-matching and prompt-matching. It cannot support the document's "empty, nothing
else running" assertion. This does not prove the historical run was contended; it proves the cited
measurement cannot establish that it was uncontended.

### 7. Real corpus run

```text
$ bash tests/corpus/run.sh
DENOMINATOR checked=34 total=34 excluded=69 caught=3 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0
CORPUS_EXIT=1
```

Independent per-detector recount:

```text
CAUGHT M2.sh rc=1
CAUGHT M7.sh rc=1
CAUGHT T6.sh rc=1
RECOUNT raw=103 checked=34 excluded=69 caught=3 other=0 reconciled=103
```

Arithmetic reconciles: the manifest has 105 shell files; `run.sh` and `_selftest.sh` are explicitly
excluded from the candidate loop, leaving `103 = 34 checked + 69 excluded`. No current detector
timed out. `T6.sh` reported actual `sed -i ''` lines in `bin/codex-fanout.sh` and
`bin/codex-tick.sh`; I found no B13 detector firing only on B13's own documentation.

The historical B13 numbers also add up internally: `34 + 69 = 103`, and
`caught 28 = 25 persistent timeouts + 3 ordinary caught detectors`. They are historical, not the
current repository result.

### 8. Required independent verifier

```text
$ FLEET_MUTANTS=0 bash verify.sh
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  FAIL swarm                      (see var/verify.log)
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants   (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 16 passed, 2 failed, 1 skipped (denominator: 19 stages) --
VERIFY_EXIT=6
```

The stage arithmetic is correct: `16 + 2 + 1 = 19`. `var/verify.log` named swarm's failure as
`CC1 six concurrent dispatches on a cold store all succeed: 1 of 6 failed`; corpus published the
same current `checked=34 total=34 excluded=69 caught=3` result shown above.

## Findings

### P1 — B13 invents exit code 5 and retains untyped exit code 1

Failure → direct users receive exit `5` for contention-flavoured timeout and exit `1` for a caught
invariant. `verify.sh` then flattens either child result, including a future child `3`, into a generic
stage failure and final exit `6`.  
Cause → `tests/corpus/run.sh:92-98` uses shell-conventional private codes instead of the repository's
typed exit contract, while `verify.sh:42-43` records every non-zero child as `FAIL`.  
Fix → use the existing types end to end. Recommended mapping: caught/persistent/broken → `6`
(`invariant/quality violation`), contention-only → `3` (`environment fault`). Preserve a stage's
`3` through `verify.sh` so it is not reported as an agent/invariant failure. Do not create another
new code.

### P1 — The timeout denominator drops the retry-77 outcome

Failure → two first-pass timeouts produce a final timeout sum of one; the timeout-then-`77` case is
represented only as `excluded=1`, with all relevant timeout counters falsely zero for that event.  
Cause → `tests/corpus/run.sh:83` increments only `excluded`; the claimed three terminal timeout
buckets omit the existing `77` retry arm.  
Fix → publish and assert an exhaustive equation. At minimum add `timeouts_initial` and
`timeout_excluded`, label the retry-`77` outcome, and require
`timeouts_initial == timeout_contention + timeout_confirmed + timeout_persistent + timeout_excluded`
for every completed verdict. Preserve `raw_total == checked + excluded` separately from the
mechanisable denominator.

### P1 — The uncontended-run evidence command is invalid

Failure → the exact cited process command returns matches generated by itself and the agent prompt,
so "empty" is not a reproducible observation.  
Cause → broad substring matching is used without excluding the probe process or identifying actual
executables/PIDs.  
Fix → rerun with an executable-aware process probe that prints PID, executable, and argv, explicitly
excludes the probe and parent orchestration process, and capture the probe immediately before and
during the corpus run. Correct `docs/delta.d/B13.md` to distinguish the new measurement from the
invalid historical claim.

### P2 — The mutation proof is ephemeral and incompletely specified

Failure → the document says fixture contents are pasted in full, but the contention fixture's exact
script/setup is absent, and no checked-in self-test covers B13's runner states.  
Cause → proof was left in a deleted scratch directory.  
Fix → add a non-acceptance automated runner test (do not edit `tests/acceptance/*`) covering clean,
ordinary broken, persistent timeout, timeout→pass, timeout→fail, timeout→`77`, and empty input.
Assert labels, exit types, and denominator equations. Paste its real output and exit code.

## Exactly what must change before acceptance

1. Replace corpus exits `1`/`5` with allowed typed exits and preserve environment exit `3` through
   the full verifier instead of flattening it into failure `6`.
2. Make timeout accounting exhaustive, including retry-`77`, with a checked arithmetic invariant
   for initial timeout count and raw/eligible detector totals.
3. Add a durable non-acceptance self-test for every retry arm and empty input; include the exact
   fixture source or a repository path, plus real exit-coded output.
4. Correct the invalid `ps aux | grep` evidence and rerun `bash tests/corpus/run.sh` plus
   `FLEET_MUTANTS=0 bash verify.sh`, publishing the complete current denominators and all red stages.

B13 must not remain accepted as `[x]` until those four changes are independently re-run.
