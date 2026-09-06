# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-LIVE-CODEX`

## Verdict

**REJECT**

The walkthrough identifies real silent-failure defects, but it is not a reproducible end-to-end
acceptance record, one command is wrong as written, the named backlog item does not exist, and the
required B8 work remains incomplete.

## What was claimed

- A complete user flow worked: plan, refusals, SOW creation/acceptance, run, status, ledger verify,
  tamper, and restore (`docs/delta.d/opus-walkthrough.md:3-6`).
- `fleet run --task ""` exits 7 with zero bytes on both streams (`:11`).
- A tampered ledger exits 8 with no output, while the success path prints
  `verified checked=12 total=12` (`:12-14`).
- `fleet plan` with `FLEET_STATE` unset emits partial output before its environment fault (`:16-18`).
- The proposed fix is a detector requiring a reason on every non-zero exit (`:20-22`).

The nearest backlog contract is B8, not `opus-walkthrough`. B8 is still unchecked and requires all
three fixes, `tests/corpus/M9.sh`, a published denominator, mutation testing, and a green verifier
(`handover/BACKLOG.md:92-106`).

## Contract check

Command:

```text
rg -n '^## .*opus-walkthrough' handover/BACKLOG.md
```

Exit: `1`.

There is no standalone `opus-walkthrough` item. The only match is a B8 prose reference at
`handover/BACKLOG.md:94`. The review target therefore has no acceptance criteria under the name given
by the request. I evaluated it against B8 as the nearest explicit contract.

## What I actually ran

The checkout had no `fleet` executable on `PATH`; `keel/target/debug/fleet` existed. I prepended that
directory to `PATH` for the product commands. Each targeted test used a fresh `mktemp -d` state
directory.

| Case | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` | 7 | 0 | 187 | Does **not** reproduce the claim; it refuses earlier because `--repo` is missing. |
| `fleet run --task "" --repo . --agent stub` | 7 | 0 | 0 | Reproduces the silent empty-task refusal. |
| `fleet ledger verify` on a fresh state | 6 | 0 | 0 | Additional silent empty-ledger failure. |
| 12 appends, then `fleet ledger verify` | 0 | 29 | 0 | `verified checked=12 total=12`. |
| Tamper row 6, then `fleet ledger verify` | 8 | 0 | 0 | Reproduces the silent mismatch failure. |
| Restore row 6, then `fleet ledger verify` | 0 | 29 | 0 | Restores `verified checked=12 total=12`. |
| `env -u FLEET_STATE fleet plan "diagnose fleet"` | 3 | 61 | 264 | Reproduces three stdout lines before the environment diagnostic. |

The exact accepted-SOW flow was also attempted in isolated state: plan exited `0`; run before SOW
exited `7` with a useful refusal; incomplete SOW exited `7` with the missing citation and corrected
template; a completed SOW exited `9`; `fleet sow accept` exited `0`. The subsequent accepted run
against `--repo .` exited `7` because the current target repository had uncommitted changes. The
claimed complete run is therefore not reproducible from this checkout, and the deliverable supplies
no clean-target setup or transcript to make it independently replayable.

## Arithmetic and known-cheat checks

- The document says “Two defects” and then adds a “third, smaller” defect, but publishes no
  checked/total denominator for the proposed detector or for the surfaces exercised.
- `tests/corpus/M9.sh` is absent. The corpus contains `105` shell detector files.
- The verifier's corpus log reports `DENOMINATOR checked=34 total=34 excluded=69 caught=18`; this is
  not M9 coverage and does not establish that every refusable surface was checked.
- `FLEET_MUTANTS=0` explicitly skipped the mutation stage. No M9 mutation result exists to verify.
- P0 passed `34 passed, 0 failed`, but its empty-task assertion checks only exit 7 and redirects
  both streams to `/dev/null` (`tests/acceptance/p0.sh:108-113`). That green result does not test the
  missing user-visible reason.
- The proposed “every non-zero exit” rule has no classified set of inputs. Untriggered or excluded
  surfaces are not published, so a future detector could pass while measuring only a subset.

## Independent verifier result

Command: `FLEET_MUTANTS=0 bash verify.sh`

Exit: `6`.

Real output:

```text
== fleet verify ==
  .... fmt
  ok   fmt
  .... clippy -D warn
  ok   clippy -D warn
  .... unit tests
  ok   unit tests
  .... acceptance builds
  ok   acceptance builds
  .... cargo-deny
  ok   cargo-deny
  .... cargo-audit
  ok   cargo-audit
  .... secrets
  ok   secrets
  .... acceptance
  ok   acceptance
  .... readme
  ok   readme
  .... swarm
  FAIL swarm (see var/verify.log)
  .... policy
  ok   policy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke
  ok   attest-smoke
  .... pytest
  ok   pytest
  .... detectors
  ok   detectors
  .... corpus
  FAIL corpus (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
VERIFY_RC=6
```

The separate required P0 run exited `0` and printed `== 34 passed, 0 failed ==`; its expected
permission-denied line while probing the read-only artifact was followed by a passing assertion.

## Findings

1. **The primary empty-task reproduction is incomplete as written.** The documented command reaches
   the missing-`--repo` parser refusal and emits 187 stderr bytes. The silent defect requires
   `--repo . --agent stub` (or an equivalent complete invocation). A reader copying the document
   cannot reproduce its stated observation.

2. **The claimed end-to-end success is not independently replayable.** No binary path, state setup,
   clean target setup, stream capture, or per-step exit code is recorded. The accepted run currently
   stops at the dirty-target guard with exit 7, so “works end to end” is not verified evidence.

3. **The two main product defects remain.** Complete empty-task execution returns `7` with zero
   bytes; tampered ledger verification returns `8` with zero bytes. The code inspection matches the
   behavior: `ledger_verify` prints success only after `verify_rows` returns (`keel/fleet/src/main.rs:2995-3000`),
   and `run_command` prints only an artifact after successful work (`keel/fleet/src/main.rs:842-846`).

4. **The detector acceptance is missing.** M9 does not exist, its input denominator is not published,
   excluded surfaces are not classified, and mutation testing was skipped. B8 cannot be accepted
   from P0's exit-code-only assertion or from the unrelated `34/34` corpus subset.

5. **The acceptance contract reference is broken.** `handover/BACKLOG.md` has no standalone
   `opus-walkthrough` item. A reviewer cannot know whether this document is itself a deliverable or
   merely evidence for B8 without an explicit mapping.

## Exactly what must change for acceptance

1. Add a standalone `opus-walkthrough` backlog item, or explicitly assign this document to B8 and
   state that mapping in the contract.
2. Correct the empty-task command to include all required arguments. Record isolated `FLEET_STATE`,
   binary path, target-repository precondition, stdout, stderr, and exit code for every flow step.
3. Fix the empty-task, tampered-ledger, and partial-plan paths so every non-zero exit emits a
   human-readable reason without leaking partial output before an environment refusal.
4. Add `tests/corpus/M9.sh` with a published `checked/total` denominator. Include every refusable
   surface, classify excluded or untriggerable cases, assert the reason content, and test both a
   genuine failure and the detector's negative control.
5. Mutation-test M9, then rerun `FLEET_MUTANTS=0 bash verify.sh`; acceptance requires final exit `0`
   with no failed stages and no unreported skips.
