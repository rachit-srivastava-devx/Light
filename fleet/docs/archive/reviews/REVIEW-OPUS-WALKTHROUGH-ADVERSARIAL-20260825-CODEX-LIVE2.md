# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260825-CODEX-LIVE2`

Verdict: **REJECT**

## Contract under review

`handover/BACKLOG.md` B8 requires all of the following:

1. The three reported paths are fixed.
2. `tests/corpus/M9.sh` exists, drives every refusable surface, publishes its denominator, and is mutation-tested.
3. `FLEET_MUTANTS=0 bash verify.sh` is green.

## What the deliverable claims

`docs/delta.d/opus-walkthrough.md` claims a full plan/SOW/run/status/ledger flow works, then reports:

- `fleet run --task ""` exits 7 with zero bytes on stdout and stderr.
- `fleet ledger verify` on a tampered chain exits 8 with no output.
- `fleet plan` with `FLEET_STATE` unset prints three lines before exit 3.
- A detector should enforce a reason on every non-zero exit.

The document publishes no denominator for the refusable surfaces, no per-surface classification, no M9 result, and no mutation result.

## Commands actually run

All commands below used the existing release binary `./keel/target/release/fleet` and temporary `FLEET_STATE` directories.

| Check | Exit | Observed |
|---|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 stdout bytes, **187 stderr bytes**: `--repo is required` and usage. The claim is not reproducible as written. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | **0 stdout / 0 stderr bytes**. The silent refusal reproduces on a valid invocation. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 stdout bytes containing `intent`, `agent`, `skills`; 264 stderr bytes naming the missing environment. This claim reproduces. |
| `fleet ledger append --event note --body '{}'` | 0 | Ledger setup succeeded. |
| `fleet ledger verify` before tamper | 0 | `verified checked=1 total=1`. Denominator present on success. |
| `fleet ledger verify` after changing ledger `seq:0` to `seq:9` | 8 | **0 stdout / 0 stderr bytes**. The silent mismatch reproduces. |
| `fleet ledger verify` after restoring the original chain | 0 | `verified checked=1 total=1`. |

I also exercised the stated SOW flow. A valid SOW returned exit 9, `fleet sow accept --id <id>` returned exit 0, and `run --agent env-probe` then refused with exit 7, 0 stdout bytes, and 519 stderr bytes because this shared checkout was dirty. Therefore the claimed successful end-to-end run was not independently confirmed in this constrained checkout; the refusal was explained, not silent.

## Acceptance-artifact checks

- `tests/corpus/M9.sh`: **absent**.
- M9-like detector: **absent**.
- Detector inventory: 105 scripts, but this is not an M9 denominator and does not establish coverage of every refusable surface.
- Mutation evidence for M9: **none**, because M9 does not exist.
- B8 remains unchecked: `## [ ] B8 — Every non-zero exit must print a reason`.

The walkthrough’s arithmetic is incomplete: “two defects” plus “a third, smaller” is not a published `checked/total` result. It does not show how many refusable surfaces were enumerated, how many were exercised, or which hard cases were excluded.

## Required verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit: **6**

Real terminal summary:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The verifier log reports:

```text
detector-integrity: 105 detectors match the manifest (denominator: 105)
TIMEOUT A4.sh exceeded 30s -- treated as FAILED
TIMEOUT C11.sh exceeded 30s -- treated as FAILED
TIMEOUT C2.sh exceeded 30s -- treated as FAILED
TIMEOUT C22.sh exceeded 30s -- treated as FAILED
TIMEOUT C3.sh exceeded 30s -- treated as FAILED
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
TIMEOUT S6.sh exceeded 30s -- treated as FAILED
TIMEOUT S9.sh exceeded 30s -- treated as FAILED
TIMEOUT T1.sh exceeded 30s -- treated as FAILED
TIMEOUT T15.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=13
```

The `FLEET_MUTANTS=0` setting also explicitly skipped the mutants stage; that is not mutation evidence for M9.

## Exactly what must change

1. Fix both valid silent paths so every non-zero `run` refusal and tampered `ledger verify` mismatch writes at least one human-readable reason naming the failure.
2. Add `tests/corpus/M9.sh`. Enumerate every refusable surface, execute each one, assert non-zero exit plus a reason line, and publish `checked`, `total`, and any exclusions. A zero-input or untriggerable surface must fail, not pass.
3. Mutation-test M9 in both directions: remove/bypass the reason assertion and prove M9 goes red; restore it and prove green. Record the actual mutation result.
4. Re-run `FLEET_MUTANTS=0 bash verify.sh` to a final exit code of 0, with the complete `14/1/1` failure resolved and the corpus denominator/result included in the evidence.
5. Rewrite the walkthrough evidence with exact runnable invocations, a denominator for all tested refusal surfaces, and separate environment-blocked runs from successful end-to-end runs.

