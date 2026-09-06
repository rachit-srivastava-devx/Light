# Adversarial review — `opus-walkthrough`

**Verdict: REJECT**

Review date: 2026-08-26  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract used: `handover/BACKLOG.md:92-106` (B8), because the backlog contains no item literally named `opus-walkthrough`; B8 is the only item that references this deliverable.  
Constraint observed: no direct Git command was issued against the shared checkout. The mandated acceptance/verifier scripts created throwaway Git repositories internally. No source, deliverable, backlog, DELTA, acceptance, or `keel/` file was edited; `verify.sh` wrote its documented `var/verify.log`, and this review is the only file intentionally added.

## What was claimed

The deliverable claims:

1. A complete `plan` → refused `plan` → run without SOW → refused SOW → accepted SOW → run → status → ledger verify → tamper → restore flow worked end to end.
2. The literal command `fleet run --task ""` exited 7 with zero bytes on stdout and stderr.
3. A tampered `fleet ledger verify` exited 8 silently, while the clean path printed `verified checked=12 total=12`.
4. With `FLEET_STATE` unset, `fleet plan` printed `intent:`, `agent:`, and `skills:` before an environment fault.
5. Every non-zero CLI exit should write at least one line naming the reason.

The prose calls the first two failure paths “Two defects” and then adds a third defect. It publishes no walkthrough coverage denominator, exact task, state directory, target-repository preconditions, binary path/version, SOW ID, artifact ID, or command-by-command transcript.

## What I actually ran

I rebuilt with `cargo build --release` (exit 0) and drove the explicit repo binary at `keel/target/release/fleet`. Every state store below was a fresh `mktemp -d` subdirectory. Exit codes were captured immediately after each command.

| Command / case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `cargo build --release` | 0 | build success | 0 B | Completed in 0.80s. |
| `fleet --version` | 0 | 83 B | 0 B | Reported `fleet 0.1.0 (db3b5d..., built 2026-08-24T18:42:07Z)`. |
| `FLEET_STATE=<tmp> fleet plan "add a --version flag to the cli"` | 0 | 499 B | 0 B | Rendered three commands and `commands: 3 planned (denominator: 3)`; route was unavailable. |
| `FLEET_STATE=<tmp> fleet plan "make me a sandwich"` | 7 | 0 B | 154 B | Refused and named three closest intents. |
| Literal `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 B | **187 B** | Did **not** reach empty-task validation; it printed `--repo is required`. |
| Complete empty-task form: `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | **0 B** | **0 B** | Reproduced the underlying silent `EMPTY_TASK` refusal. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 B | 264 B | Printed `intent`, `agent`, and `skills`, then a useful `FLEET_STATE is not set` diagnostic. |
| Run before accepted SOW, with the exact multiline task and full flags | 7 | 0 B | 290 B | Correctly named the missing SOW ID and recovery commands. |
| `fleet sow --task "add a --version flag"` | 7 | 0 B | 806 B | Correctly named the missing evidence citation and printed a complete template. |
| First reconstructed multiline SOW | 7 | 0 B | 868 B | Correctly rejected my malformed alternative syntax. This proves the walkthrough did not publish enough input to replay its claimed acceptance. |
| SOW using the tool's exact accepted schema | 9 | 1502 B | 194 B | Printed `SOW_READY_AWAITING_REVIEW` with ID `fcea10df...`. |
| `fleet sow accept --id fcea10df...` | 0 | 125 B | 0 B | Printed `SOW_ACCEPTED`. |
| Post-acceptance `fleet run ... --repo "$PWD" --agent stub` | 7 | 0 B | 519 B | Refused because the shared checkout had uncommitted changes. No artifact or successful end-to-end run was observed. |
| `fleet status` after the replay | 0 | 2571 B | 0 B | Reported 2 of 2 receipt-derived tasks, both failed; this is not proof of a successful run. |
| `fleet ledger verify` after the replay | 0 | 27 B | 0 B | Printed `verified checked=6 total=6`. |
| Twelve isolated `fleet ledger append` setup calls | 12/12 exited 0 | — | — | Setup denominator: `checked=12 total=12`. |
| Clean 12-row `fleet ledger verify` | 0 | 29 B | 0 B | Printed exactly `verified checked=12 total=12`. |
| Edit row 1 from `"i":1` to `"i":999` | 0 | — | — | `cmp` exited 1, proving the tamper changed bytes. |
| Tampered 12-row `fleet ledger verify` | 8 | **0 B** | **0 B** | Reproduced the silent mismatch. |
| Restore the original chain | 0 | — | — | Restored from the pre-tamper copy. |
| Restored 12-row `fleet ledger verify` | 0 | 29 B | 0 B | Returned to `verified checked=12 total=12`. |
| Fresh-state `fleet ledger verify` | 6 | **0 B** | **0 B** | Additional silent non-zero path omitted by the walkthrough. |
| Fresh-state `fleet status --json` | 0 | 452 B | 0 B | Vacuous success: `checked=0`, `total=0`, `empty=true`. |
| `bash tests/corpus/M9.sh` | 127 | — | error | `No such file or directory`; the B8 detector does not exist. |

## Arithmetic and hard cases

### Walkthrough arithmetic

- Claimed defects: `2 + 1 smaller = 3`, but the prose labels only two as defects.
- Walkthrough coverage: **no `checked/total` denominator**. This is a failure under the repo's central rule.
- Independently exercised cases: **24 of 24 command/case rows above produced a typed observation**; the twelve ledger setup calls were **12 of 12**. The successful post-SOW run was **0 of 1**, so the claimed full flow did not resolve in this replay.
- The clean/tamper/restore ledger arithmetic is valid: `12/12 → exit 8 after a confirmed byte change → 12/12 after restore`.

### Quietly dropped hard cases

1. The literal empty-task command does not reproduce the claimed 0/0 output; required `--repo` and `--agent` flags were omitted.
2. A fresh ledger exits 6 silently, not only a tampered ledger.
3. A fresh status exits 0 with `checked=0,total=0`, violating the stated “zero inputs fail” rule unless cold start is explicitly documented as a different contract.
4. The post-SOW run requires a clean Git repository with `main.rs`; the walkthrough publishes neither that fixture nor its artifact/attestation evidence.

## Known-cheat audit

### Existing “all refusal surfaces” gate is a scoped subset

`tests/acceptance/swarm.sh:249-270` reports `Q1 all 11 refusal surfaces are actionable (denominator: 11)`, but its selected list does not exercise:

- the complete empty-task invocation;
- tampered or empty `ledger verify`;
- `plan` with `FLEET_STATE` unset.

It therefore passes while all three deliverable findings remain reproducible. The denominator is published for the chosen subset, not for “every non-zero exit.” This is exactly the proxy-for-property failure B8 is meant to prevent.

### Current corpus gate arithmetic

The corpus result is arithmetically reconcilable:

- 105 shell files total;
- `run.sh` and `_selftest.sh` are skipped by the runner, leaving 103 executed;
- `checked=34 + excluded=69 = 103`;
- `caught=25 = 23 timeouts + M2 + M6`;
- checked-but-not-caught: `34 - 25 = 9`.

However, `detector-integrity` calls all 105 shell files “detectors” even though two are runner/self-test harness files. More importantly, 23 timeouts are collapsed into `caught`, so the published number does not separate “regression caught” from “detector timed out.”

### Other known cheats observed

- **Vacuous pass:** fresh `status --json` exits 0 at `checked=0,total=0`.
- **Detector firing on documentation:** `bash tests/corpus/M6.sh` exits 1 at `23 of 24` because "fleet arch" is documented but absent, matching backlog B12.
- **Detector labels are not verdicts:** `bash tests/corpus/C12.sh` prints its historical defect label and exits 0 because its scan finds no current hit. I did not count that label as a defect; the corpus `caught=25` arithmetic instead resolves to 23 timeouts plus M2 and M6.
- **No weakened margin found:** this deliverable does not touch Requirement 5's preregistered margin.
- **No fabricated null-as-zero metric found in the walkthrough:** the separate fresh-status 0/0 result is a vacuous-state contract defect, not enough evidence to call a nullable measurement fabricated.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real result, exit **6**:

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
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The referenced corpus log ended with:

```text
M2: 34433 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
M6: documented but missing: fleet arch
M6: 23 of 24 documented commands exist (denominator: 24)
DENOMINATOR checked=34 total=34 excluded=69 caught=25
```

It also named 23 individual 30-second detector timeouts. The gate is red; B8's `verify.sh green` acceptance criterion is not met.

### Conflicting direct controls

Two direct `bash tests/acceptance/p0.sh` runs returned exit 1 with `30 passed, 4 failed`; a direct `bash tests/acceptance/swarm.sh` returned exit 1 with `45 passed, 5 failed`. In the single full verifier run, those embedded stages reported `34 passed, 0 failed` and `50 passed, 0 failed`. Because concurrent workers share this checkout and Git inspection was prohibited, I did not guess at attribution. The only defensible conclusion is that the checkout did not provide a stable independently repeatable green control during this review.

## Findings

### F1 — The primary reproduction command is wrong

**Failure → Cause → Fix:** `fleet run --task ""` produces 187 bytes of useful stderr, not zero bytes → the document omitted mandatory `--repo` and `--agent` arguments, so the literal command stops at argument validation → publish the complete invocation and its isolated state/repository preconditions.

### F2 — “Works end to end” is unsupported

**Failure → Cause → Fix:** the replay reached SOW acceptance but no successful run/artifact/attestation → the document supplies no exact accepted task, clean fixture, SOW ID, artifact ID, exit codes, or transcript → add a complete reproducible transcript ending in run exit 0, artifact resolution, status, clean verify, confirmed tamper rejection, and restored verify.

### F3 — Missing denominator hides untested cases

**Failure → Cause → Fix:** the document cannot show how many user surfaces it checked → it narrates a flow and three findings without `checked/total` or an explicit universe → enumerate every claimed command and hard case, classify each, and publish the total; untriggered cases count against the result.

### F4 — The proposed general detector is absent and the existing proxy passes falsely

**Failure → Cause → Fix:** Q1 says all 11 selected refusals are actionable while the complete empty task and ledger mismatch remain silent → Q1's hand-selected denominator omits the exact cases → implement B8's M9 over the actual refusable dispatch surface, publish the surface denominator, and mutation-test both silent and verbose directions.

### F5 — The required gate is red

**Failure → Cause → Fix:** the independent verifier exits 6 at `14/16 passed, 1 failed, 1 skipped` → corpus has 23 timeouts plus M2/M6 catches → remove in-repo build-artifact traversal from detector scope or move build output outside it, retain timeout as a distinct failing outcome, fix M6's documentation false positive, and rerun the complete gate.

### F6 — The stated contract is ambiguous

**Failure → Cause → Fix:** there is no backlog item literally named `opus-walkthrough` → the only match is B8's reference to this document → add an explicit backlog item/ID or state that B8 is the acceptance contract before the next review.

## Exactly what must change before acceptance

1. Correct the literal empty-task command and publish all required flags, binary path/version, state path, and fixture preconditions.
2. Replace the unsupported end-to-end sentence with a complete resolving transcript, or mark the run `PARTIAL/BLOCKED` with the failed step and denominator.
3. Publish walkthrough `checked/total` arithmetic and include empty-ledger, empty-status, tampered-ledger, and post-SOW-run cases.
4. Implement B8's three user-facing fixes plus `tests/corpus/M9.sh`; M9 must cover the real non-zero surface, publish its denominator, and be mutation-tested in both directions.
5. Make `FLEET_MUTANTS=0 bash verify.sh` exit 0 with a stable repeat run; include the real output. Until then, the deliverable is **REJECTED**.
