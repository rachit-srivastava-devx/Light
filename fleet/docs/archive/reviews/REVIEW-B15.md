# Adversarial review — B15

Date: 2026-09-02  
Deliverable: `docs/delta.d/B15.md`  
Contract: `handover/BACKLOG.md` item B15  
Verdict: **REJECT**

## What was claimed

B15 claims acceptance path (b): repeated per-detector `_scan.py` walks were not the dominant cost;
the actual cost was descending into the unpruned `target-shared/` build tree. It reports a one-line
`PRUNED_DIRS` change, one walk dropping from 77,112,396 lines / 68.75s to 939,209 lines / 1.154s,
and corpus wall time dropping from 1,506s to 97s with `checked=34` unchanged. It also claims no
detector check changed, the timeout was not raised, and the before/after classifications were
unchanged in meaning.

The contract allows only:

1. one shared walk across the `_scan.py` detectors, with substantially lower wall time and unchanged
   detector count and pass/fail verdicts; or
2. timing proof that per-detector re-walking was not dominant, plus the actual dominant cost.

## What I ran

The checkout contains later B16/B17 changes, including the later `tmp` prune and C2 memoization.
Therefore current corpus output is not represented as a byte-for-byte replay of the B15 snapshot.
I ran the current user commands, inspected the preserved B15 logs, and replayed the B15 scanner
states without editing any file.

### 1. Cited process probe

```sh
ps aux | grep -E "verify\.sh|run\.sh|corpus|cargo (build|test)|cargo-mutants|codex exec"
```

Exit: `0`. It was not empty. It matched the review's `codex exec`, the shell command, the concurrent
scanner process, and `grep` itself. The literal probe cited by B15 cannot produce an empty result
because it self-matches.

### 2. Detector integrity

```sh
bash bin/detector-integrity.sh
```

Exit: `0`.

```text
detector-integrity: 105 detectors match the manifest (denominator: 105)
```

But the changed shared implementation is outside that denominator:

```sh
rg -n '_scan\.py' tests/corpus/MANIFEST.sha256
```

Exit: `1`, no matches. Separately, `rg` found **25** detector scripts importing `_scan`; those are
exactly the 25 detectors in the B15 pre-fix timeout set. The green 105-entry shell manifest therefore
does not prove that the effective detector implementation was unchanged.

### 3. Scanner timing replay

The deliverable's literal ``time python3 ... lines(root)`` contains an ellipsis and is not executable.
I ran equivalent inline Python using `_scan.lines` and a non-mutating replay of its prune set.

Current scanner, exit `0`:

```text
yielded_lines=202084
```

B15-state post-prune replay (`PRUNED_DIRS - {'tmp'}`), exit `0`:

```text
DENOMINATOR yielded_lines=939731 files_read=891 bytes_read=46793098 elapsed_s=0.917
```

B15-state pre-prune replay (`PRUNED_DIRS - {'target-shared', 'tmp'}`), exit `0`:

```text
DENOMINATOR yielded_lines=77374813 files_read=20953 bytes_read=4432814263 elapsed_s=61.920
target-shared: lines=76435081 files=20062 bytes=4386021165
tmp:           lines=737647   files=177   bytes=35288096
```

The small drift from B15's 77,112,396 / 68.75s and 939,209 / 1.154s is expected on a live checkout.
The raw diagnosis is independently reproduced: `target-shared` was 98.8% of yielded lines and 99.0%
of bytes read.

### 4. Preserved B15 evidence and arithmetic

`stat` reproduced the claimed log intervals:

```text
var/b15-before.log birth=2026-09-02 01:18:22 modified=2026-09-02 01:43:28
var/b15-after.log  birth=2026-09-02 01:44:28 modified=2026-09-02 01:46:05
```

Parsing the files produced:

```text
before: main timeouts=25, persistent=25
DENOMINATOR checked=34 total=34 excluded=69 caught=28 timeout_contention=0 timeout_confirmed=0 timeout_persistent=25

after: main timeouts=1, persistent=1
DENOMINATOR checked=34 total=34 excluded=69 caught=8 timeout_contention=0 timeout_confirmed=0 timeout_persistent=1
```

The published counts add up:

- `34 checked + 69 excluded = 103` runnable detector scripts; `run.sh` and `_selftest.sh` make the
  integrity manifest's 105 shell files.
- Before: `25` persistent timeouts + `3` other failures (M2/M6/M7) = `28 caught`.
- After: A1/S6/S9/T6 + M2/M6/M7 + C2 = `8 caught`.

No denominator is missing, no unmeasured value was written as zero, and the observed run was not
vacuous.

### 5. Corpus as a user

```sh
bash tests/corpus/run.sh
```

Exit: `1`, wall time 9.2s on the later current checkout.

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=3 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0
```

The three current catches were M2, M7, and T6. T6 reported three real `sed -i ''` occurrences under
`bin/`; this was not a detector firing on its own documentation.

### 6. Independent full verifier

```sh
FLEET_MUTANTS=0 bash verify.sh
```

Exit: `6`.

```text
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
```

The red corpus stage published `checked=34 total=34 excluded=69 caught=3` in `var/verify.log`.

## Findings

### F1 — Path (b) is contradicted by B15's own timing — blocking

All 25 pre-fix timeouts are the 25 `_scan` consumers. Each was allowed 30s on the main pass and 30s
on serial retry. Therefore the repeated timed-out runs alone account for:

```text
25 detectors × 2 attempts × 30s = 1,500s
1,500s / 1,506s observed wall = 99.6%
```

The independent full walk also takes 61.9s, so each 30s attempt can expire inside the scan. The
unpruned build tree explains **why each repeated walk was slow**; it does not show that repeated
per-detector walking was non-dominant. It shows the opposite. B15 neither shared one walk (path a)
nor proved re-walking non-dominant (path b).

### F2 — The one-line fix changed effective detector input while the integrity proof ignored it — blocking

Adding `target-shared` to `PRUNED_DIRS` removed 98.8% of yielded lines from all 25 consumers. That is
an effective detector change even though no `[A-Z]*.sh` predicate changed. `MANIFEST.sha256` does not
seal `_scan.py`, so “105 detectors match” cannot support “no detector's check was altered.” A shared
helper could make all 25 detectors pass while the current integrity gate remained green.

This scope change may be sensible for generated Cargo artifacts, but it is a third solution, not
either solution in the B15 contract.

### F3 — “Unchanged pass/fail verdicts” was not demonstrated — blocking for path (a)

Twenty detectors went from a gate-level caught timeout to pass. B15 calls their pre-fix state a
“timeout mask,” which is accurate, but then asserts their true verdict was pass. No true pre-fix
verdict exists for those 20 in `var/b15-before.log`; only A1 received the separately described
pre-prune exact-scan check. The hard cases were classified without before-state verdict evidence.

### F4 — The uncontended-process evidence is not reproducible as cited — finding

The exact `ps | grep` command self-matches and its output is not preserved in either B15 log. The
historical logs substantiate duration and detector results, but not the claim that the process probe
was empty. This does not invalidate the 77M-line diagnosis; it invalidates that supporting assertion.

## Required changes for acceptance

1. Reopen B15. Do not describe the `target-shared` prune as acceptance path (b).
2. Satisfy the existing contract: preferably run the 25 `_scan` consumers from one shared immutable
   scan result, then publish wall time, `checked/total/excluded`, and a per-detector before/after
   output table. Alternatively, provide timing that actually shows repeated scanning below the
   dominant share and names a larger measured cost; the current 99.6% result cannot support that.
3. Establish real pre-change verdicts for all 25 consumers without timeout masks and demonstrate
   unchanged verdicts. A timeout counted as caught is not evidence of the underlying detector result.
4. Seal `_scan.py` (or all executable detector dependencies) in the integrity mechanism and prove by
   mutation that changing the shared helper makes integrity fail.
5. Replace the self-matching process probe with a recorded, non-self-matching process inventory before
   calling a run uncontended.

