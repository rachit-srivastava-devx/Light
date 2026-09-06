# Adversarial review — B17

## Verdict: REJECT

The headline runtime evidence is reproducible: the corpus is red on exactly M2, M7, and T6, and
the full verifier exits 6 with 17 passed, 1 failed, and 1 skipped. B17 is nevertheless not safe to
accept because the A1/S6/S9 "fix" suppresses every file below `tmp/`, including real source that
matches all three detectors. That contradicts the acceptance claim that pruning `tmp/` "only
removes noise, never hides a real finding." The shared scanner that implements this suppression is
also outside the detector-integrity manifest.

## What was claimed

1. A1, S6, and S9 were false positives confined to PDF/reference material under `tmp/pdfs/`; adding
   `"tmp"` to `_scan.py::PRUNED_DIRS` was claimed to be a safe input-only fix.
2. M6 was fixed by changing absent-command prose from backticks to quotes; it should report 25/25.
3. M7 should still reject only the real hidden `fleet contract` command at 24/25; test-module
   literals should no longer be counted as commands.
4. T6 and M2 are real and intentionally scoped out. The corpus should therefore report exactly
   three caught detectors, and `verify.sh` should remain red.
5. The named artifact directories were claimed to contain 66,114 of 67,007 paths, leaving 778 real
   source paths.

## What I actually ran

No git command was run. No acceptance test, contract, deliverable, backlog, DELTA, or `keel/`
source/configuration file was edited; the required verifier used the existing build-output trees.

| Command / probe | Exit | Observed result |
|---|---:|---|
| `bash tests/corpus/A1.sh` | 0 | No hit |
| `bash tests/corpus/S6.sh` | 0 | No hit |
| `bash tests/corpus/S9.sh` | 0 | No hit |
| `bash tests/corpus/M6.sh` | 0 | `25 of 25` (denominator 25) |
| `bash tests/corpus/M7.sh` | 1 | `fleet contract` absent from help; `24 of 25` |
| `bash tests/corpus/T6.sh` | 1 | Three `sed -i ''` hits at the claimed lines |
| `bash tests/corpus/M2.sh` | 1 | 67,027 paths, above 15,000 |
| `bash tests/corpus/run.sh` | 1 | 9.27s; `checked=34 total=34 excluded=69 caught=3` |
| `bash bin/detector-integrity.sh` | 0 | 105/105 manifest entries match |
| `keel/target/debug/fleet --help` | 0 | `contract` is not rendered in COMMANDS |
| `keel/target/debug/fleet contract lane-status validate` against an empty temporary state | 6 | Dispatch arm is reachable, but produces no user output on empty state |
| GNU sed 4.10 `gsed -i '' 's|...|...|' <scratch-file>` | 2 | `can't read s|...|: No such file or directory`; file unchanged |
| macOS `/usr/bin/sed -i '' 's|...|...|' <scratch-file>` | 0 | File changed as expected |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | Corpus is the only failed stage |

I did not run the mutating `bin/detector-integrity.sh --update` against the shared checkout because
the review is authorized to create only this file. I ran the exact update command against a
disposable copy: exit 0, 105 entries. Its hash mapping matched the live files; byte order differed
because the live manifest keeps `_selftest.sh` and `run.sh` appended while a fresh shell glob sorts
them into the main list. The live non-mutating integrity check is green, and the recorded M7 digest
matches the file.

## Independent corpus result

```text
M2: 67027 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
M6: 25 of 25 documented commands exist (denominator: 25)
M7: reachable but absent from --help: fleet contract
M7: 24 of 25 top-level commands appear in --help (denominator: 25)
T6 Linux CI executed BSD-only commands
DENOMINATOR checked=34 total=34 excluded=69 caught=3 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0
```

The denominator reconciles: 105 `*.sh` files = 2 harness files (`run.sh`, `_selftest.sh`) + 34
mechanised checks + 69 exit-77 exclusions. The three caught units are detectors, not hit lines:
M2 + M7 + T6 = 3. Relative to B16, A1 + S6 + S9 + M6 leaving the caught set explains 7 -> 3.

## Finding 1 — the shared scanner was weakened and hides real violations

`_scan.py` discards every directory named `tmp` before any detector sees its files. An independent
scan of the current tree produced:

```text
default:      A1=0 S6=0 S9=0
tmp-unpruned: A1=3 S6=2 S9=12
```

The unpruned examples are the same reference artifacts B17 names, so their classification as false
positives is plausible. The implementation is still broader than that classification: it suppresses
all future and current source under every `tmp/`, not merely the reviewed PDF artifacts.

I mutation-tested this with two byte-identical source fixtures, `src/bad.py` and `tmp/bad.py`, each
containing a hand-rolled SHA-256 round/compress function, divergent remember/enforce stores, and an
offline embedding model under `/tmp`. The real shared scanner yielded all 7 lines from `src/bad.py`
and the detector predicates reported A1=1, S6=2, S9=1. It yielded 0 lines from the identical
`tmp/bad.py`; all three violations disappeared solely because of the directory name.

Failure -> a real violation below `tmp/` passes A1/S6/S9. Cause -> blanket directory pruning happens
before the predicates. Fix -> remove the blanket `"tmp"` prune and resolve the reviewed false
positives with a narrow, evidence-backed mechanism that cannot suppress arbitrary source.

This is also an integrity gap: `bin/detector-integrity.sh` hashes only `*.sh`, while A1/S6/S9 all
delegate their scan surface to `_scan.py`. A change to this shared enforcement point can therefore
weaken many detectors while the integrity gate still reports 105/105.

## Finding 2 — the published M2 arithmetic is wrong

The six published component counts do add to 66,114. They do **not** leave 778 paths:

```text
67,007 - 66,114 = 893
```

The current independent count is 67,027 total, the same 66,114 covered paths, and 913 residual
paths. The claimed 98.7% artifact share is a valid rounding of the old figures, but the asserted
778-file "real source surface" is unsupported by the listed denominator. At review time,
`tmp/pdfs/` also contains 200 entries (182 files, 18 directories), not the documented 41; this may
be concurrent checkout growth, but the deliverable is not a current measurement.

## Independent `verify.sh` result

Command: `FLEET_MUTANTS=0 bash verify.sh`

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
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
```

`REAL_VERIFY_EXIT=6`. The same run's corpus verdict was
`checked=34 total=34 excluded=69 caught=3 timeout_contention=0 timeout_confirmed=0 timeout_persistent=0`.

## Exactly what must change before re-review

1. Remove the blanket `"tmp"` entry from `tests/corpus/_scan.py::PRUNED_DIRS`. Fix the reviewed
   A1/S6/S9 false positives with path/content rules limited to known generated reference artifacts,
   or relocate those artifacts into an explicitly generated and integrity-governed surface. An
   arbitrary source file must not become invisible merely by living below a directory named `tmp`.
2. Integrity-seal `tests/corpus/_scan.py` (either include it in the detector manifest or add an
   equivalent non-vacuous shared-scanner manifest), because it controls the effective input of many
   hashed detector scripts.
3. Add and run positive/negative mutation controls for A1, S6, and S9 that prove real violations are
   still caught after the false-positive fix; publish their checked/total denominators.
4. Correct the M2 subtraction and refresh or timestamp the drift-prone `tmp/pdfs` and tree counts,
   then rerun the seven detectors, the uncontended corpus, detector integrity, and
   `FLEET_MUTANTS=0 bash verify.sh`. Keep B17 `[~]` while the contract-authorized M2/M7/T6 scope-outs
   leave the verifier red.
