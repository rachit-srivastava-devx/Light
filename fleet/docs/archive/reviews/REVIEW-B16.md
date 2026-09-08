# Adversarial review — B16

## Verdict: REJECT

B16's performance fix is real: C2 is memoized per yielded path, finishes far below 30 seconds, preserves
the positive/negative hit behavior, prints only the first eight hits, and is integrity-sealed. The
deliverable still violates a repository hard rule: the exact C2 detector exits 0 after examining zero
source inputs and publishes no input denominator. That is a vacuous pass, so B16 cannot be accepted as
implemented until the non-vacuity contract is enforced.

## Contract reviewed

`handover/BACKLOG.md` B16 requires:

1. `temp_rooted(p)` computed once per unique yielded file, not once per line.
2. Same skipped-file set, hit predicate, and first-eight-hits output.
3. C2 well below 30 seconds uncontended.
4. Corpus detector/verdict denominator otherwise unchanged, with `timeout_persistent=0` for C2.
5. Deliberate detector-integrity update and a pasted `FLEET_MUTANTS=0 bash verify.sh` result.

The repository's higher-order hard rule also applies: a gate that examines zero inputs must fail, and a
verdict must publish its denominator.

## What was claimed

- Re-reads reduced from 939,731 to 888 by a path-keyed dict cache: 1,058x.
- C2 improved from an incomplete run beyond 900 seconds to 1.54 seconds, exit 0.
- Corpus improved from 97.84 seconds / `caught=8 timeout_persistent=1` to 37.47 seconds /
  `caught=7 timeout_persistent=0`; all other verdicts stayed the same.
- Four controls behaved correctly: one offender caught; temp-rooted, override-guided, and wrong-suffix
  negatives exempted.
- 105 detector manifest entries matched.
- Full verification remained honestly red: 17 passed, 1 failed, 1 skipped, denominator 19, exit 6.

## Commands actually run

All commands ran from the quoted repository path. Exit codes were captured immediately, without a
pipeline.

### 1. C2 as a user

```text
/usr/bin/time -p bash tests/corpus/C2.sh
C2 a test wrote production ledger rows
real 0.39
user 0.17
sys 0.06
C2_EXIT=0
```

Observed: C2 is currently 76.9x below its 30-second budget. The live time differs from the historical
1.54 seconds because later B17 work pruned more scratch input; it still strongly confirms the B16
performance property.

### 2. Detector integrity

```text
bash bin/detector-integrity.sh
detector-integrity: 105 detectors match the manifest (denominator: 105)
INTEGRITY_EXIT=0
```

The cited mutating `--update` command was also executed against an isolated, auto-cleaned copy because
this review is allowed to create only this file in the repository:

```text
bash bin/detector-integrity.sh --update
detector-integrity: manifest updated (105 detectors)
SCRATCH_INTEGRITY_UPDATE_EXIT=0
MANIFEST_ENTRY_SET_EQUALS_REPO=true repo_entries=105 scratch_entries=105
```

The generated and repository manifests have the same 105 entries. Their line order differs because of
glob collation; integrity verification is entry-based and passes in the real checkout.

### 3. C2 controls using the actual embedded detector program

```text
FIXTURE offender EXIT=1 OUTPUT='<TMP>/src/case.sh:1: printf x >> "$FLEET_LEDGER"'
FIXTURE temp_rooted EXIT=0 OUTPUT=''
FIXTURE override_guided EXIT=0 OUTPUT=''
FIXTURE wrong_suffix EXIT=0 OUTPUT=''
FIXTURE_AND_EQUIVALENCE_EXIT=0
```

A separate exact-script fixture with nine offenders produced hits 1 through 8 only:

```text
NINE_HIT_C2_EXIT=1 PRINTED_HITS=8
FIRST8_HARNESS_EXIT=0
```

Observed: the claimed positive, three negatives, exit codes, and first-eight output all reproduce.

### 4. Live tree walk and memoization arithmetic

```text
WALK unique_paths=711 yielded_lines=202084 noncontiguous=0
EQUIVALENCE mismatches=0 rooted_files=3 rereads_before=202084 rereads_after=711
FIXTURE_AND_EQUIVALENCE_EXIT=0
```

These are current-tree values after later B17 input pruning, not an attempt to rewrite B16's historical
888 / 939,731 measurement. The cache still computes once per yielded unique path, all path visits are
contiguous, and the same three files are temp-rooted.

### 5. Corpus as a user

```text
/usr/bin/time -p bash tests/corpus/run.sh
DENOMINATOR checked=34 total=34 excluded=69 caught=3 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0
real 9.31
user 6.00
sys 2.63
CORPUS_EXIT=1
```

Observed: the B16 denominator shape remains 34 checked and 69 excluded; C2 no longer times out. The
current caught count is 3, not 7, because B17 subsequently changed A1/S6/S9/M6 behavior. That drift is
visible in `handover/PROGRESS.md` and is not attributed to B16.

The saved B16 logs were independently diffed. Their only changes are:

- C2 main-pass timeout becomes a normal C2 pass.
- The serial C2 retry and `TIMEOUT-PERSISTENT` lines disappear.
- M2's file count changes 67,006 to 67,007 while remaining a refusal.
- `caught` changes 8 to 7, `timeout_persistent` changes 1 to 0, and timings change.

No other detector output differs between `var/b16-before-corpus.log` and
`var/b16-after-corpus.log`.

### 6. Required independent full verifier

```text
FLEET_MUTANTS=0 bash verify.sh
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
  ok   swarm
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
REAL_VERIFY_EXIT=6
```

The corpus tail in this independent verifier was:

```text
TIMEOUT-PERSISTENT M6.sh -- timed out again on an uncontended serial retry
DENOMINATOR checked=34 total=34 excluded=69 caught=4 timeout_contention=0 timeout_confirmed=0 timeout_persistent=1
```

C2 itself passed. M6's timeout is later-checkout/concurrent-load drift, not evidence that B16's C2 cache
regressed. The red result is preserved here rather than converted into a B16 green claim.

## Arithmetic checked by hand

- Re-read reduction: 939,731 / 888 = 1,058.26x; the claimed 1,058x is correct.
- Budget margin: 30 / 1.54 = 19.48x; the claimed 19.5x is correct.
- Historical speedup lower bound: 900 / 1.54 = 584.42x; “well over 500x” is correct.
- Corpus accounting: 34 checked + 69 excluded = 103 runnable detector scripts.
- Integrity accounting: 103 runnable + `run.sh` + `_selftest.sh` = 105 sealed scripts.
- Before caught count: 7 ordinary catches + 1 persistent C2 timeout = 8.
- After caught count: 7 ordinary catches + 0 persistent C2 timeouts = 7.
- Full verifier accounting: 17 + 1 + 1 = 19 stages.

No hard case was dropped in the recorded B16 before/after log pair. The M2 one-file drift was disclosed
and does not change its verdict.

## Findings

### [P1, blocking] C2 passes vacuously and emits no input denominator

I copied the exact current `C2.sh` and `_scan.py` into an otherwise empty auto-cleaned tree and ran the
script itself:

```text
bash <TMP>/tests/corpus/C2.sh
C2 a test wrote production ledger rows
EMPTY_TREE_C2_EXIT=0
EMPTY_TREE_HARNESS_EXIT=0
```

Failure → the detector reports success after `lines(root)` yields zero tuples, and its verdict contains
no `{checked,total}` input count.

Cause → C2 tracks only `hits`; it never counts examined lines/files, never refuses zero input, and its
shell wrapper recognizes only Python exits 0/1 while mapping every other exit to environment fault 3.

Impact → a scanner/prune/path regression can make C2 inspect nothing while the corpus runner counts C2
as a checked pass. The runner's 34-detector denominator proves scripts executed; it does not prove C2
examined any source input. This is the exact vacuous-green failure prohibited by AGENTS.md hard rule 6
and the KT denominator law.

### [P1, out of B16 scope] The full verifier also grants `recur` green at `checked=0`

`var/verify.log` contains `recur-gate: checked=0 flagged=0 (signatures=1: E1)` while the top-level
verifier reports `ok recur`. This is a separate, already-recorded verifier honesty defect. It does not
change the C2 memoization result, but it means “17 passed” includes one vacuous green stage.

## Known-cheat audit

- Detector weakened to pass: not found. The hit predicate is still the single line at C2.sh:31 and all
  four behavioral controls reproduce.
- Timeout raised: not found. The live C2 run is below the existing 30-second default.
- `0` substituted for unknown/null: not found in B16's measurements; the reported zeros are observed
  event counts.
- Detector fires on its own documentation: not found in the live run; shared scanning prunes `docs` and
  `tests`.
- Done while secretly partial/blocked: not found. B16 discloses the red full verifier and names the
  unrelated corpus catches.
- Vacuous pass: found and blocking, as described above.

## Exactly what must change

1. In `tests/corpus/C2.sh`, count examined input explicitly and print a C2 denominator on every verdict;
   define it in terms of yielded source lines and unique yielded paths.
2. If that checked count is zero, exit 6 as an invariant violation before returning success. Update the
   shell exit mapping so Python exit 6 remains 6 rather than becoming environment exit 3.
3. Keep `temp_rooted`, the skipped-file rule, line hit predicate, and first-eight-hits output unchanged.
4. Add a runnable scratch control proving empty input exits 6, alongside the existing offender and three
   negative controls; rerun integrity, corpus, and `FLEET_MUTANTS=0 bash verify.sh`, and paste real red.

Acceptance after repair: non-empty current C2 still exits 0 below 30 seconds; the offender still exits 1;
the three negatives still exit 0; nine hits still print exactly eight; empty input exits 6 with
`checked=0`; integrity reports 105/105 or the honest new sealed denominator; corpus publishes its full
denominator; full verification's real exit is recorded.
