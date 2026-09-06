# Adversarial review: B-VERIFY-LOG-RACE

Review date: 2026-08-30 IST  
Deliverable: `docs/delta.d/B-VERIFY-LOG-RACE.md`  
Intended contract: item `B-VERIFY-LOG-RACE` in `handover/BACKLOG.md`  
Constraint observed: no direct `git` command was run. The deliverable, `docs/DELTA.md`,
`handover/BACKLOG.md`, `tests/acceptance/*`, and `keel/` were not edited. This review is the only
file created.

## Verdict

**REJECT**

The underlying shared-log defect is real and was independently reproduced. The submitted
deliverable is still not acceptable: its claimed backlog item does not exist, it explicitly says
the defect is not fixed, its original observation does not establish which worker produced the
rows, and the stated `morning-report.sh` mitigation can display a nested policy subtotal as if it
were the verifier's result. The required full verifier is also red.

## What was claimed

1. Concurrent `verify.sh` callers share and can corrupt `var/verify.log`.
2. Seeing `N8`/`N9`/`Q1` rows mid-fanout was empirical proof that a different worker's output was
   read, possibly interleaved with another run.
3. The defect was left as backlog item `B-VERIFY-LOG-RACE`, not fixed in the deliverable.
4. `bin/morning-report.sh` was patched to warn instead of presenting a concurrent log as fact.
5. A future fix should use a PID/caller-scoped path or a log override.

## What I ran

### 1. Contract lookup

```text
$ rg -n '^## \[[^]]\] B-VERIFY-LOG-RACE\b' handover/BACKLOG.md
exit 1

$ rg -n 'B-VERIFY-LOG-RACE' handover/BACKLOG.md
exit 1
```

Observed: the named backlog item and its acceptance criteria are absent from the complete 405-line
file. The only other non-review source reference is `bin/morning-report.sh:42`. There is therefore
no contract denominator to grade and no backlog state (`[ ]`, `[~]`, `[x]`, or `[!]`).

### 2. Independent required verifier

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
exit 6
```

Arithmetic: `16 + 2 + 1 = 19`. Source enumeration also gives 18 unconditional `stage` calls plus
one mutants pass/skip branch, so the published verifier denominator is correct. This is not green.

### 3. Focused user run of the first red stage

```text
$ bash tests/acceptance/swarm.sh
  FAIL N1 free text routes to swarm dispatch   rc=3
  FAIL N3 a multi-step intent plans >1 command   n=0
  FAIL N4 the plan publishes its denominator   no denominator in the plan
  ok   Q1 all 11 refusal surfaces are actionable (denominator: 11)
  FAIL N8 the change plan names the SOW step   plan sends the user straight to a refusal
  FAIL N9 ordinary implementation phrasings route (handle division by zero)   refused
  FAIL N9 ordinary implementation phrasings route (support UTF-8 filenames)   refused
  FAIL N9 ordinary implementation phrasings route (validate the config on load)   refused
  FAIL N9 ordinary implementation phrasings route (delete the dead retry path)   refused
  FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
== 44 passed, 9 failed ==
exit 1
```

Arithmetic: the 53 emitted assertions count by hand and `44 + 9 = 53`. The summary does not label
53 as a denominator, but no emitted assertion was quietly dropped. `Q1` passes; it merely appears
next to the failing `N8`/`N9` rows.

### 4. Controlled two-caller reproduction

I started caller A with `FLEET_MUTANTS=0 bash verify.sh`, waited until its acceptance output had
populated the log, then started caller B with the same command. Both were run from the quoted repo
path. Before B started:

```text
BEFORE inode=515298364 size=17067
sha256=eabda5531972bd94906b79fa5e0479ed213bf8663b9cc96bf492daaebc111d43
```

After B started, while both verifier PIDs were live:

```text
AFTER_B_START inode=515298364 size=12367
sha256=b0c71c444ef90071e093886db6c902c3d18a2f24d8e76624d07f8a0eb28771e1
PID 1226  bash verify.sh
PID 5532  bash verify.sh
```

The same inode lost 4,700 bytes after the second caller executed `: > "$LOG"`; caller A then kept
appending to that truncated file. Both reproduction runs were intentionally interrupted after the
collision was captured and their PTYs exited 1; those interrupted exits are not verifier verdicts.
This directly proves destructive cross-run interference. It does not, by itself, prove that any
specific line was torn at the byte level.

### 5. Implemented-path inspection

- `verify.sh:24` fixes every caller to `var/verify.log` and truncates it at startup.
- `verify.sh:42` redirects every stage into that same path.
- `verify.sh:88` prints the final 19-stage summary only to the caller's stdout, not to the log.
- `bin/morning-report.sh:46` searches the shared detail log for any `-- N passed` line.
- The current `var/loop/MORNING-REPORT.md:43` therefore shows
  `-- 3 passed, 0 failed (denominator: 3 policies) --` under “Last known verify.sh summary”, not the
  independently observed verifier result `16 passed, 2 failed, 1 skipped`.

I did not execute `bin/morning-report.sh`: it writes `var/loop/MORNING-REPORT.md` and invokes `git`,
which would violate this review's explicit write and no-git constraints. Its current emitted
artifact and exact read path were inspected instead.

## Findings

### F1 — No acceptance contract exists (P1, confidence 10/10)

Failure -> the review request names backlog item `B-VERIFY-LOG-RACE`, but no such item occurs in
`handover/BACKLOG.md`.  
Cause -> the deliverable says “Left as a backlog item” without actually registering one.  
Fix -> add an explicit backlog item with falsifiable acceptance criteria and a state before asking
for acceptance. Until then, there is no denominator and the item cannot be accepted.

### F2 — The original provenance claim is unsupported (P1, confidence 10/10)

Failure -> the note treats unrelated `N8`/`N9`/`Q1` rows as evidence that another worker wrote the
log. A verifier always runs the full suite, so rows unrelated to the caller's code are expected.
The focused single-caller run reproduced failing `N8`/`N9` and passing `Q1`.  
Cause -> content was used as a worker identity even though the log contains no run ID, PID, start
time, stage boundary, or caller marker.  
Fix -> replace “confirmed empirically” with the exact reproducible two-caller evidence, and label
the original other-worker attribution as unverified.

### F3 — The warning mitigation still reports the wrong summary (P1, confidence 10/10)

Failure -> `morning-report.sh` calls an inner `-- 3 passed ... policies` line the last verifier
summary. The real verifier summary is never written to `var/verify.log`.  
Cause -> the reader greps an untyped concatenation of stage output for a pattern also emitted by
nested stages.  
Fix -> consume a completed, invocation-scoped result record containing typed counts and exit code;
never infer the top-level verdict by grepping arbitrary child output.

### F4 — The race is documented but deliberately unfixed (P1, confidence 10/10)

Failure -> a second caller truncates the first caller's evidence and both then append to the same
inode. The deliverable explicitly says “Not fixed here.”  
Cause -> one global mutable file is both an active write target and a later evidence source.  
Fix -> allocate an invocation-scoped log before any stage, make its path available to that caller,
write stage headers plus the final `{passed,failed,skipped,total,exit_code}` result into it, and
publish “latest completed run” only via an atomic manifest/symlink update after completion.

### F5 — No regression proof or published concurrency denominator (P1, confidence 10/10)

Failure -> the note has no positive/negative test, no caller count/result table, and no evidence
that its suggested PID/override design prevents collision.  
Cause -> the incident note was submitted as the deliverable before implementation and verification.
  
Fix -> add a non-acceptance-suite regression test that overlaps two controlled callers and checks
`checked=2,total=2`: each caller must retain only its own marker, summary, and exit code. Also prove
the ordinary one-caller path. Mutation-test by restoring the shared path and show the test red.

## Known-cheat audit

- Gate weakened to pass: not observed; the full verifier stayed red.
- `0` substituted for unknown: not observed in this deliverable.
- Vacuous zero-input pass: the deliverable has no acceptance check at all, which is worse than a
  zero-input pass and is rejected under the denominator rule.
- Detector firing on its own documentation: not observed for the race itself. A related parser bug
  exists: morning-report mistakes a nested policy summary for the verifier summary.
- Done while PARTIAL/BLOCKED: there is no backlog state. The deliverable itself says “Not fixed
  here”, so it cannot support an implemented/done verdict.

## Exactly what must change before re-review

1. Create the missing `B-VERIFY-LOG-RACE` backlog contract with explicit checked/total criteria.
2. Implement invocation-scoped logs and a typed, atomic completed-run record; remove active writes
   to the shared `var/verify.log` evidence path.
3. Make `morning-report.sh` read only a completed top-level verifier result, never nested child text.
4. Add two-caller and one-caller regression proofs, publish the denominator, and mutation-test the
   shared-path regression.
5. Correct the deliverable's provenance claim and paste a fresh `FLEET_MUTANTS=0 bash verify.sh`
   result, including any red stages and the real exit code.
