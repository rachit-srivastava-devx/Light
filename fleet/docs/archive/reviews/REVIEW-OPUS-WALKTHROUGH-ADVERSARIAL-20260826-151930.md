# Adversarial review: opus walkthrough

- Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-151930`
- Reviewed: `docs/delta.d/opus-walkthrough.md`
- Contract: `handover/BACKLOG.md`, B8, lines 92-106
- Date: 2026-08-26
- Verdict: **REJECT**

The note correctly identifies three user-facing defects, but it does not satisfy B8. All three
remain reproducible in the current release build, the required `tests/corpus/M9.sh` does not
exist, no M9 denominator or mutation proof exists, and the required verifier is red. I also found
a fourth non-zero path that emits no reason: `ledger verify` on an empty state exits 6 with zero
bytes on both streams.

## What was claimed

1. The manual `plan -> run -> sow -> run -> status -> ledger verify -> tamper -> restore` flow
   works end to end.
2. `fleet run --task ""` exits 7 with no output.
3. `fleet ledger verify` on a tampered chain exits 8 with no output; success publishes
   `checked=12 total=12`.
4. `fleet plan` with `FLEET_STATE` unset prints `intent`, `agent`, and `skills` before exiting on
   an environment fault.
5. A detector should enforce that every non-zero exit names the reason.

B8's acceptance contract is stronger than the note: fix the three paths, add mutation-tested
`tests/corpus/M9.sh` covering every refusable surface with a published denominator, and make
`verify.sh` green.

## What I actually ran

I rebuilt the current source first:

| Command | Exit | Observed |
|---|---:|---|
| `cargo build --manifest-path keel/Cargo.toml --release` | 0 | Current release binary built successfully. |
| `FLEET_STATE=<tmp> fleet plan "add a --version flag"` | 0 | 499 stdout bytes, 0 stderr bytes; prints `commands: 3 planned (denominator: 3)`. |
| `FLEET_STATE=<tmp> fleet plan "clone https://example.invalid/repo"` | 7 | 0 stdout bytes, 195 stderr bytes; refusal names three near matches. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 stdout bytes, 41 stderr bytes; usage only. This exact abbreviated command does not reproduce the claimed three partial lines. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 stdout bytes, 264 stderr bytes; prints `intent`, `agent`, `skills`, then the environment fault. Defect reproduced with the missing prompt supplied. |
| `FLEET_STATE=<tmp> fleet run --task <complete-task> --repo . --agent stub` before SOW acceptance | 7 | 0 stdout bytes, 290 stderr bytes; explains the missing accepted SOW. |
| `FLEET_STATE=<tmp> fleet sow --task "fix it"` | 7 | 0 stdout bytes, 806 stderr bytes; explains the missing evidence citation and supplies a corrected template. |
| `FLEET_STATE=<tmp> fleet sow --task <complete-task>` | 9 | 2,073 stdout bytes, 194 stderr bytes; emits a SOW and `SOW_READY_AWAITING_REVIEW`. |
| `FLEET_STATE=<tmp> fleet sow accept --id 442e...d64d` | 0 | 125 stdout bytes, 0 stderr bytes; acceptance recorded. |
| `FLEET_STATE=<tmp> fleet run --task <same-task> --repo <disposable-copy> --agent stub` after acceptance | 7 | 0 stdout bytes, 477 stderr bytes; the copied shared worktree was dirty, so Fleet correctly refused. The note does not provide a clean fixture or exact arguments; its end-to-end success claim was not independently reproduced. |
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 stdout bytes, 187 stderr bytes; complains that `--repo` is missing. The cited command is under-specified. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo . --agent stub` | 7 | **0 stdout bytes, 0 stderr bytes**; the actual empty-task defect remains. |
| `FLEET_STATE=<tmp> fleet ledger verify` on a valid one-row chain | 0 | 27 stdout bytes, 0 stderr bytes: `verified checked=1 total=1`. |
| Tamper the row body without recomputing its hash, then run `fleet ledger verify` | 8 | **0 stdout bytes, 0 stderr bytes**; the tamper defect remains. |
| Restore the saved chain, then run `fleet ledger verify` | 0 | 27 stdout bytes, 0 stderr bytes: `verified checked=1 total=1`. |
| `FLEET_STATE=<new-empty-state> fleet ledger verify` | 6 | **0 stdout bytes, 0 stderr bytes**; additional B8-class defect. |
| `FLEET_STATE=<tmp> fleet status` | 0 | Publishes `1 of 1 tasks`; the empty task is classified as `FAILED` with `refused: EMPTY_TASK`. |

No direct Git command was run. Temporary state and the disposable target copy were outside the
shared checkout.

## Findings

### F1 - The three acceptance defects are still live

**Failure -> Cause -> Fix**

- Empty task: exit 7 and 0/0 output -> `run_with_evidence` writes a receipt and returns at
  `keel/fleet/src/main.rs:904-912` without printing -> emit an actionable `EMPTY_TASK` diagnostic
  before returning.
- Tampered ledger: exit 8 and 0/0 output -> `ledger_verify` propagates `verify_rows` failure before
  its only `println!` at `keel/fleet/src/main.rs:2995-2999` -> name the mismatched row/property on
  stderr before exit 8.
- Missing `FLEET_STATE` during planning: three plan fields precede the error -> output begins at
  `keel/fleet/src/main.rs:2659-2661`, but state is checked later at line 2664 -> validate required
  environment before printing a plan, or emit a coherent atomic failure with no partial plan.

This alone rejects B8 because its first acceptance clause is "the three are fixed."

### F2 - The required detector, denominator, and mutation proof are absent

`tests/corpus/` contains `M1.sh` through `M7.sh`; there is no `M9.sh`. `MANIFEST.sha256` likewise
has no M9 entry. Therefore:

- surfaces checked by M9: **0**
- required surface denominator: **not published**
- M9 mutation runs: **0**
- B8 detector acceptance: **0 of 3 required evidence components**

A missing denominator is a fail. It must not be inferred from a different test.

### F3 - The green Q1 result is a proxy that drops the hard cases

The verifier log says:

```text
ok   Q1 all 11 refusal surfaces are actionable (denominator: 11)
```

That label overstates what the test measures. `tests/acceptance/swarm.sh:249-269` uses a curated
12-entry command list, drops commands that exit 0, and reports the remaining 11. Its run case is
`run --task` with a missing value. That reaches the helpful usage branch, not the fully specified
`run --task "" --repo ... --agent stub` branch that remains silent. It also does not trigger a
tampered ledger, an empty ledger, or the partial-plan environment fault.

`tests/acceptance/p0.sh:110-113` does reach the empty-task branch, but redirects both streams to
`/dev/null` and asserts only exit 7 plus receipt growth. This is exactly the proxy failure described
by the walkthrough: correct exit code and receipt, broken user experience.

Hand arithmetic:

- Q1 command specifications: 12
- successful specifications removed from `QN`: 1
- published Q1 denominator: 11
- B8 hard cases among those 11: 0 of 4 reproduced silent/partial-output cases

Q1 is not M9 and cannot be used as B8 evidence.

### F4 - A fourth non-zero path is silent

On a new empty `FLEET_STATE`, `fleet ledger verify` exits 6 with zero stdout and zero stderr. This
is an invariant failure on zero inputs, so the exit itself is honest, but it violates B8's broader
rule that every non-zero exit names its reason. M9's denominator must include this case rather than
testing only a non-empty tampered chain.

### F5 - The claimed end-to-end walkthrough is not reproducible from the document

The note names command nouns but omits the exact plan prompt, SOW task, accepted SOW ID, target repo,
agent, state setup, tamper operation, and restoration operation. Exact abbreviated invocations do
not always reach the claimed branches. I reproduced plan success/refusal, pre-SOW run refusal, SOW
refusal/readiness/acceptance, status, valid ledger verification, tamper detection, and restoration.
The post-acceptance run was blocked because the disposable copy faithfully inherited the shared
worktree's uncommitted state. The historical "works end to end" sentence is therefore unverified in
this review, not accepted evidence.

### F6 - Independent verification is red

Command:

```sh
FLEET_MUTANTS=0 bash verify.sh
```

Exit: **6**

Real terminal result:

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
  FAIL swarm                      (see var/verify.log)
  ok   policy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
```

The log contains:

```text
FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
== 49 passed, 1 failed ==
detector-integrity: 105 detectors match the manifest (denominator: 105)
DENOMINATOR checked=34 total=34 excluded=69 caught=25
```

The corpus failure included 23 named detector timeouts. Another verifier was concurrently active
during part of this run, so resource contention may have contributed to the timing failures. That
caveat does not turn exit 6 into green, and it does not affect the directly reproduced B8 defects.

## Known-cheat audit

- **Found:** hard cases dropped behind a green proxy (`Q1`), with a denominator that covers only its
  curated command list rather than every refusable surface.
- **Found:** empty-input behavior is non-vacuous in exit status but still silent (`ledger verify`
  exits 6 with no reason).
- **Not found in this deliverable:** `null` rewritten as fabricated numeric zero.
- **Not found:** B8 falsely marked done; it remains `[ ]` in `handover/BACKLOG.md`.
- **Not established:** a detector firing solely on its own documentation. The observed corpus red
  was timeout-based, not a demonstrated self-documentation match.

## Exactly what must change before ACCEPT

1. Fix all four reproduced non-zero paths so each emits at least one actionable line naming the
   reason: fully specified empty task, tampered non-empty ledger, empty ledger, and missing-state
   plan without partial success output.
2. Add `tests/corpus/M9.sh` that drives every refusable/non-zero CLI surface. Publish
   `checked=<n> total=<n>`, fail when `checked == 0`, and count an untriggerable surface against the
   result. Include the four cases above with their fully specified arguments.
3. Integrity-seal M9 in `tests/corpus/MANIFEST.sha256` and mutation-test it. Suppressing each
   diagnostic, removing a surface, allowing empty input, or accepting partial plan output must make
   M9 red and name the failed surface; restore and prove green.
4. Replace the historical flow summary with exact copy-pasteable setup and commands, including the
   task text, repo fixture, agent, SOW ID extraction, tamper step, restore step, exit codes, stream
   byte counts, and a full-flow checked/total denominator. Do not claim end-to-end success until the
   post-acceptance `run` is directly observed on a clean target.
5. Run `FLEET_MUTANTS=0 bash verify.sh` again without competing verifier load and publish the green
   expected result: `15 passed, 0 failed, 1 skipped (denominator: 16 stages)`. Separately publish
   the successful M9 mutation evidence required by B8.
