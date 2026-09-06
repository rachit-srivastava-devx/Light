# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-AUDIT`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
Verdict: **REJECT**

## Contract and claims

The requested standalone `opus-walkthrough` item is absent from `handover/BACKLOG.md`. The only
binding reference is B8 at lines 92–106, and B8 is still unchecked. B8 requires the three failure
paths to be fixed, `tests/corpus/M9.sh` to exist with a published denominator and mutation test,
and `FLEET_MUTANTS=0 bash verify.sh` to be green.

The deliverable claims that a full `plan` → SOW refusal/acceptance → `run` → `status` → ledger
verify → tamper → restore flow worked end to end. It then reports two silent failures—empty-task
run and tampered-ledger verification—and a third partial-output failure when `FLEET_STATE` is
unset. It proposes a detector for every non-zero exit, but supplies no executable transcript or
acceptance denominator.

## Commands actually run

All runs used the release binary from this quoted checkout and isolated temporary `FLEET_STATE`
directories. No git command was run by this review.

| Command / case | Exit | Captured result |
|---|---:|---|
| `fleet plan "add a --version flag to the cli"` | 0 | 499 stdout bytes; three planned commands; lane reported unavailable |
| `fleet plan "make me a sandwich"` | 7 | 0 stdout / 154 stderr bytes; refusal named three candidates |
| Literal `fleet run --task ""` | 7 | **0 stdout / 187 stderr bytes**; it refused first for missing `--repo`, so the document’s literal command does not reproduce its zero-byte claim |
| Complete empty task: `fleet run --task "" --repo "$PWD" --agent stub` | 7 | **0 stdout / 0 stderr bytes**; reproduced the silent `EMPTY_TASK` refusal |
| Fresh `fleet ledger verify` | 6 | **0 stdout / 0 stderr bytes**; an additional silent non-zero path |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 stdout / 264 stderr bytes; stdout printed `intent`, `agent`, and `skills` before the environment refusal |
| Incomplete `fleet sow --task "add a --version flag"` | 7 | 0 stdout / 806 stderr bytes; actionable missing-citation template |
| Complete multiline SOW | 9 | 1604 stdout / 194 stderr bytes; `SOW_READY_AWAITING_REVIEW` ID was printed on stderr |
| `fleet sow accept --id <ID>` | 0 | 125 stdout bytes; acceptance recorded |
| `fleet run --task <SOW> --repo "$PWD" --agent stub` after acceptance | 7 | 0 stdout / 519 stderr bytes; refused because this shared checkout had uncommitted changes |
| `fleet status` after that attempt | 0 | Reported `1 of 1` task from receipts, all failed; this is a receipt denominator, not walkthrough coverage |
| `fleet ledger verify` before tamper | 0 | `verified checked=4 total=4` |
| Same ledger after editing one event in the isolated chain | 8 | **0 stdout / 0 stderr bytes**; reproduced the silent tamper failure |
| Same ledger after restoring the chain | 0 | `verified checked=4 total=4` |
| Fresh `fleet status --json` | 0 | `checked: 0, total: 0, empty: true`; a vacuous green result |
| `bash tests/corpus/M9.sh` | 127 | `No such file or directory` |
| `bash tests/corpus/M6.sh` | 1 | `M6: 22 of 23 documented commands exist (denominator: 23)`; missing "fleet arch" |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | **`FAIL corpus`; `14 passed, 1 failed, 1 skipped (denominator: 16 stages)`** |

The verifier log published `DENOMINATOR checked=34 total=34 excluded=69 caught=17`; it also
reported multiple 30-second detector timeouts as failures and M6’s "fleet arch" mismatch. The
105 corpus scripts and 105 manifest lines agree, but that does not make the gate green.

## Findings

1. **The primary reproduction is incomplete.** `fleet run --task ""` omits mandatory `--repo`
   and `--agent`, and therefore tests argument validation rather than the claimed empty-task
   path. The complete invocation reproduces the silent refusal, but the document must publish
   that complete command and its isolated state/repository preconditions.

2. **The end-to-end claim is not independently reproducible from this document.** It gives no
   exact task text, state setup, binary path/version, repo precondition, SOW ID extraction, or
   tamper/restore command. The attempted accepted run was blocked by the dirty-tree invariant;
   no successful artifact or `12/12` ledger was produced in this review. `12/12` is not a
   walkthrough denominator.

3. **The arithmetic is incomplete.** “Two defects” plus “a third, smaller” is three findings,
   but there is no `checked/total` for the flow, no count of refusal surfaces exercised, and no
   classification of omitted or untriggerable hard cases. The document’s only denominator is the
   historical ledger count `12/12`, which cannot establish coverage.

4. **The B8 acceptance work is absent.** `tests/corpus/M9.sh` does not exist, the three defects
   remain live, and the required verifier is red. A green result from an unrelated targeted test
   would not satisfy B8.

5. **There are adjacent vacuity/silence defects the walkthrough does not classify.** A fresh
   ledger refuses with `rc=6` and no reason, while fresh `status --json` exits 0 with
   `checked=0,total=0`. This conflicts with the repository law that a check measuring zero inputs
   must fail or explicitly declare cold-start semantics.

## Required changes for acceptance

1. Restore an explicit `opus-walkthrough` acceptance item, or explicitly bind this artifact to
   B8 and state that mapping.
2. Rewrite the walkthrough as a self-contained transcript: exact quoted commands, binary/setup,
   isolated state and clean repository preconditions, captured stdout/stderr, exit codes, and the
   actual tamper/restore edit.
3. Publish separate hand-checkable denominators for end-to-end steps and refusal surfaces;
   classify every omitted/untriggerable case. Do not reuse ledger-row counts as coverage.
4. Fix the three reported user-facing paths, add `tests/corpus/M9.sh`, mutation-test both refusal
   and non-refusal directions, and rerun the full verifier until its final output is green.
5. Decide and document empty-store semantics, then assert them in acceptance tests; ensure every
   non-zero path, including empty ledger verification, emits a reason.

