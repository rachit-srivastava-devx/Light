# REVIEW — B1 (Blueprint coherence audit)

Reviewer: independent verifier, adversarial by default. Did not read B1's transcript; worked only
from `docs/delta.d/B1.md`, `docs/BLUEPRINT-COHERENCE.md`, `handover/BACKLOG.md` item B1, and the
live repository. B1's process (pid 81778) had fully exited before this review was written, so the
deliverable content is final, not mid-write.

## What was claimed

`docs/delta.d/B1.md` claims: the 22 numbered blueprint files (`00`–`21` in
`../blueprints/Fleet-L8-Deep-Dive/`) contain 155 atomic falsifiable claims; 45 are IMPLEMENTED
(29.0%), 61 PARTIAL, 49 ABSENT, 0 WITHDRAWN; every file has a per-file row; the denominator is
published; `docs/DELTA.md` and peer deliverables were not touched; and `FLEET_MUTANTS=0 bash
verify.sh` came back `14 passed, 1 failed, 1 skipped (denominator: 16 stages)` with `corpus`
failing — reported as red, not laundered into a green result. B1 self-declared **PARTIAL**, not
DONE.

Acceptance contract (BACKLOG.md item B1): the doc exists; every one of the 22 files has a row; the
totals arithmetic is checkable by hand; no claim is marked IMPLEMENTED without a citation that
actually resolves.

## What I ran

**Arithmetic, not just eyeballing the summary table.** I parsed the entire claim ledger with a
script (not by reading the "Result"/"Per-file denominator" tables and trusting them) and
cross-summed every one of the 22 `### NN · NAME` sections against its stated per-file row:

```
GRAND stated: [45, 61, 49, 0, 155]
GRAND actual: [45, 61, 49, 0, 155]
=== MISMATCHES ===
(none)
```

All 22 per-file rows matched the actual claim rows beneath them exactly. Note: I caught the
document in a transient inconsistent state early (a version reading 153/58/50 total that didn't
match its own per-file table for files 04 and 17) and re-checked minutes later — B1 had
self-corrected to the current 155/61/49 figures before its process exited. The final state is
internally consistent.

**Citation spot-checks — did the resolving evidence actually resolve?** I independently opened the
cited file:line ranges for a sample of IMPLEMENTED claims across different files, without reading
B1's own reasoning first:

- `00.1`/E21 — `tests/corpus/run.sh`, `tests/corpus/MANIFEST.sha256` exist; 105 corpus scripts confirmed.
- `01.3`,`17.9`/E1 — `run_with_evidence` at `main.rs:856` exists, refuses empty/duplicate diffs, records `NO_WORK_LANDED` — matches.
- `06.2`/E6,E20 — `sow_command` (line 226), `SOW_READY_AWAITING_REVIEW` (lines 299/313), `enforce_accepted_sow` (line 374) all exist as cited.
- `09.5`/E12 — `query_dependents` at `graph.rs:1202` really is a `WITH RECURSIVE` CTE with a depth bound and returns symbol/path/depth fields — matches the claim precisely.
- `20.1`/E19 — `contracts/attestation.v1.json` really requires an in-toto `_type`, a `subject[].digest.blake3` pattern field — matches exactly.
- `21.6`/E3 — `adjudication_table` at `main.rs:1648` implements all four named quadrants (ACCEPT / ORACLE_INADEQUATE / ORACLE_OVERCONSTRAINED / BUILDER_FAULT) — matches.
- `02.3` (ABSENT) — grepped the whole tree for `PyO3`/`Verdict(` in `crew/`: nothing found, correctly ABSENT.

Every citation I checked resolved to real code doing what the ledger says. I also checked that
every one of the 45 IMPLEMENTED rows cites either an evidence key or a backticked symbol/path (none
were bare prose).

**Independent re-execution of B1's cross-cutting claim.** Rather than trust the citation alone, I
drove the actual binary the way B1's own methodology implies a reader should: I ran
`fleet status --json` against a genuinely empty, freshly created `$FLEET_STATE`:

```
$ FLEET_STATE=<fresh empty dir> keel/target/debug/fleet status --json
{
  "checked": 0,
  "total": 0,
  "empty": true,
  "groups": [ ... all four groups checked:0,total:0 ... ]
}
exit=0
```

**Verify.sh cross-check.** I could not re-run B1's exact verify.sh in isolation (three peer workers
were concurrently running their own verify.sh the whole time), but I ran `tests/corpus/run.sh`
standalone under the same concurrent load and reproduced the same failure class B1 reported —
timeouts, not logic failures:

```
$ timeout 400 bash tests/corpus/run.sh
  TIMEOUT C1.sh exceeded 30s -- treated as FAILED
  TIMEOUT C11.sh exceeded 30s -- treated as FAILED
  TIMEOUT C2.sh exceeded 30s -- treated as FAILED
  ... (several more)
49.91s user 3.57s system 13% cpu 6:40.03 total
EXIT=124
```
13% average CPU over 6:40 wall-clock is a process mostly *waiting*, not computing — consistent with
cargo-lock/CPU contention from the three other concurrently-running verify.sh invocations (plus at
least one automated review pass also running its own verify.sh at the same time), not a code
regression. This matches the specific detector IDs B1 itself named (`T1`,`T20`,`C1`,`T15`,`T5`,
`C11` — all timeout-prone corpus scripts). I could not obtain a fully quiescent tree in the time
available to prove this conclusively, so I record it as a strong but not certain exoneration.

## Finding — one claim is over-classified as IMPLEMENTED

`docs/BLUEPRINT-COHERENCE.md:151`, claim `05.3`: *"A check of zero inputs fails, and every decision
publishes `{checked,total}`."* Marked **IMPLEMENTED**, citing only the ratchet adequacy path (E17).

That citation is real — `keel/fleet/src/ratchet.rs:446` does refuse `total == 0`:
```rust
if total == 0 {
    return Err(RatchetError::new(EXIT_INVARIANT, "adequacy total must be greater than zero")
        .with_denominator(total, total));
}
```
But the claim as written is a general system property ("a check of zero inputs fails"), and the
`status` command — which is also a decision that publishes `{checked,total}` — does the opposite.
`keel/fleet/src/status.rs`'s `report()` sets `total = tasks.len()` and returns `Ok(())` with exit 0
regardless; there is no zero-total refusal anywhere in that path. I confirmed this by direct
execution above: `checked=0,total=0,exit=0`. That is precisely the vacuous-success pattern the
project's own laws prohibit ("a check that measures nothing must FAIL, not pass vacuously").

One path enforcing the property does not make the general claim IMPLEMENTED when another path
governed by the same claim violates it. This should be **PARTIAL**, with the `status` gap named as
the deficit — the same treatment B1 correctly gave several other claims (e.g. `03.6`, `05.4`) where
it found the property true in some places and not others. I did not find a second instance of this
specific "true for one path, false for another" pattern in the ~10 other IMPLEMENTED rows I sampled,
but I did not audit all 45, so I cannot say it is unique to `05.3`.

## Other observations

- Every one of the 22 files has a row; no file was skipped, and no claim was left unclassified.
- `WITHDRAWN` is correctly 0 — I could not find any blueprint claim explicitly retracted by a `D<n>`
  in `docs/DELTA.md`; "not yet built" is correctly not conflated with "withdrawn."
- B1 touched only `docs/BLUEPRINT-COHERENCE.md` and `docs/delta.d/B1.md`. `docs/DELTA.md`,
  `handover/BACKLOG.md`, and peer files were unmodified by B1 (confirmed by mtimes and content).
- No git command appears anywhere in B1's transcript (checked all 52 `exec` blocks in
  `var/loop/codex-B1.log`); the diff-style output in the log is the coding tool's own patch-apply
  rendering, not an invocation of `git`.
- B1 self-reported `PARTIAL`, matching the required last-line contract, and did not convert its own
  red verify.sh into a green claim. This is the honest behavior the project's laws ask for.

## Verdict

**ACCEPT-WITH-FINDINGS**

The audit is real: 22/22 files covered, the denominator arithmetic is exactly checkable by hand
(verified independently, not just re-added from the summary table), and every citation I spot-checked
across seven different files resolved to real code doing what was claimed. The red verify.sh result
was reported honestly rather than hidden, and the most likely cause (corpus-stage timeouts under
four-way concurrent load) is not attributable to B1's own change, since B1 added no code.

It is not a clean ACCEPT because at least one IMPLEMENTED classification (`05.3`) does not survive
independent execution — the claim it encodes is false as a general property of this checkout, and
the citation given, while real, only covers one of at least two paths the claim governs. Before this
item is marked `[x]`: reclassify `05.3` as PARTIAL and name the `status` gap; re-run `tests/corpus/
run.sh` alone on a quiescent tree (no concurrent verify.sh elsewhere) to confirm the corpus failure
was contention, not a regression, and paste that result into the ledger.
