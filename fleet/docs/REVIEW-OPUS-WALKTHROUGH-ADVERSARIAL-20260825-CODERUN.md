# Adversarial review — opus walkthrough

Reviewer: independent runtime verifier, 2026-08-25. I did not run `git` and did not edit the
deliverable, `DELTA.md`, `BACKLOG.md`, or `keel/`.

## Verdict

**REJECT**

The two silent failure reports are real and reproducible. The claimed successful end-to-end flow is
not reproducible from this deliverable, has no published step denominator, and failed at the actual
run step in the shared checkout. The document also does not contain the exact arguments and state
needed to reproduce its first command.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims:

1. `plan` → refusal → run without SOW → SOW refusal → SOW acceptance → run → status → ledger verify
   → tamper → restore works end to end.
2. An empty task is refused with exit 7 and zero bytes on both streams.
3. A tampered ledger is refused with exit 8 and no output, while success prints a `{checked,total}`
   denominator.
4. `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and `skills:` before its
   environment failure.
5. A detector should require every non-zero exit to write a reason.

The stated contract reference is also inconsistent: `rg -n -i 'opus-walkthrough|walkthrough'
handover/BACKLOG.md` found only the B8 reference to this document; there is no separate backlog item
named `opus-walkthrough` with acceptance criteria.

## What I actually ran

I used the existing release binary at the explicit setup path
`keel/target/release/fleet` and a fresh `mktemp -d` `FLEET_STATE`. Stdout and stderr were captured
separately; exit status was captured immediately after each command.

| Command / step | Exit | Stdout bytes | Stderr bytes | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` literally, with no other args | 7 | 0 | 187 | Refuses earlier because `--repo` is required; this is not the document's claimed empty-task reproduction. |
| `fleet run --task "" --repo "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs" --agent stub` | 7 | 0 | 0 | Exact silent empty-task defect reproduced. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 | 264 | Exactly three plan lines precede the environment diagnostic. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 | 41 | Bare `plan` is a different, actionable usage refusal. |
| `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | Names the missing evidence citation and emits a corrected template. |
| Complete SOW, `fleet sow --task "$TASK"` | 9 | 1,444 | 194 | Produced a 64-character SOW id. |
| `fleet sow accept --id <captured-id>` | 0 | 125 | 0 | Acceptance succeeded and printed actor/time. |
| `fleet run --task "$TASK" --repo "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs" --agent stub` | 7 | 0 | 519 | Refused because the shared checkout was dirty; no successful run or artifact was observed. |
| `fleet status` | 0 | 2,482 | 0 | Rendered a 1-of-1 receipt rollup, but only the empty-task failure was present. |
| `fleet ledger verify` before tamper | 0 | 27 | 0 | `verified checked=5 total=5`. |
| Same command after changing one ledger event in the temporary state | 8 | 0 | 0 | Exact silent tampered-chain defect reproduced. |
| Same command after restoring the ledger | 0 | 27 | 0 | `verified checked=5 total=5`. |

The SOW refusal, acceptance, status, ledger success, tamper, and restore steps were therefore
exercised. The claimed successful run step was not: the document gives no exact task text, clean
disposable target, or transcript that would let a reviewer distinguish a genuine successful run from
a refusal.

## Arithmetic and known-cheat checks

- The document says “Two defects” and then lists a third smaller defect. That is understandable
  prose, but it publishes no `N of M` flow-step denominator, no per-step pass/fail total, and no
  reproducible input set for the end-to-end assertion. The success claim is therefore not
  checkable by hand.
- The success denominator changed from the document's example `12/12` to `5/5` in my isolated
  state. That is not itself a defect; it demonstrates why the state, fixture, and ledger row count
  must be published with the transcript.
- Fresh-state checks exposed two adjacent vacuity/silent-surface issues not called out in the
  walkthrough: `fleet status --json` returned exit 0 with `{checked: 0, total: 0, empty: true}`, and
  empty `fleet ledger verify` returned exit 6 with zero bytes on both streams.
- The independent gate was run exactly as requested:

  ```text
  $ FLEET_MUTANTS=0 bash verify.sh
  == fleet verify ==
    .... fmt                       
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
    SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
    ok   attest-smoke
    ok   pytest
    ok   detectors
    FAIL corpus (see var/verify.log)
  -- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
  exit 6
  ```

  `var/verify.log` records 22 timeout failures: `A1`, `A3`, `A4`, `A8`, `B10`, `C1`, `C11`, `C2`,
  `C22`, `C24`, `C26`, `C3`, `C6`, `C9`, `S4`, `S6`, `S9`, `T1`, `T15`, `T20`, `T5`, and `T6`.
  The corpus summary is `DENOMINATOR checked=34 total=34 excluded=69 caught=24`; 34 + 69 = 103
  runnable detectors after the two framework scripts are excluded, while detector integrity reports
  105 manifest entries. The exclusions and the timeout failures are named, not silently dropped.

## Findings

### F1 — End-to-end success is unsupported and currently failed

**Failure → Cause → Fix:** The claimed complete flow did not reach a successful run → the document
omits the exact task, clean target, state, and per-step transcript, and the only user-run target
available in this shared checkout is dirty → provide a copy-pasteable disposable-target setup and
record every step's command, exit code, stdout/stderr result, artifact id, ledger denominator, tamper
failure, and restored success.

This is a contract failure, not merely a missing nicety: an API/command exit sequence is not proof
that the user-visible workflow completed.

### F2 — The first cited reproduction is underspecified

**Failure → Cause → Fix:** The literal `fleet run --task ""` does not reach empty-task validation →
the command is missing required `--repo` and `--agent` arguments → replace it with the complete,
quoted command and state that the shorter command exercises a different usage refusal.

### F3 — The document has no acceptance-item or denominator contract

**Failure → Cause → Fix:** There is no `opus-walkthrough` acceptance item in `BACKLOG.md`, and the
document has no `N of M` walkthrough-step result → the reviewer cannot determine what “works end to
end” means or what was omitted → restore an explicit backlog item or link the document to B8, then
publish a numbered flow table with a denominator and classify every step.

## Exactly what must change before acceptance

1. Add the missing contract reference: a real `opus-walkthrough` backlog item, or an explicit
   statement that B8 is the acceptance contract.
2. Replace the prose flow claim with an exact, reproducible transcript using a clean disposable
   target and isolated `FLEET_STATE`; include all inputs and all exit codes.
3. Publish the flow denominator, for example `N of M steps observed`, with the two silent failures,
   the SOW refusal/acceptance, successful run, status, ledger verify, tamper, and restore each
   classified.
4. Correct the empty-task reproduction to include `--repo` and `--agent`, and distinguish the
   missing-argument refusal from the empty-task refusal.
5. Keep the red verifier result red in the acceptance record; do not call the broader workflow
   verified while the corpus stage exits 6.
