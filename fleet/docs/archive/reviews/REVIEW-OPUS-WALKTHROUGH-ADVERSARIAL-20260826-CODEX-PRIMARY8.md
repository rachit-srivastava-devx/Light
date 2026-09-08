# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY8`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
Reviewer: independent runtime review; no Git command was run

## Contract

There is no literal `opus-walkthrough` item in `handover/BACKLOG.md`. The matching contract is
B8, `handover/BACKLOG.md:92-106`: fix the three user-facing failures, add
`tests/corpus/M9.sh` with a published denominator and mutation evidence, and finish with a green
verifier. I used B8 as the operative contract and record the naming mismatch as a finding.

## What was claimed

The deliverable claims:

1. A complete `plan` → refusal → run-without-SOW → SOW refusal → SOW acceptance → run → status →
   ledger verify → tamper → restore flow worked end to end.
2. `fleet run --task ""` exits `7` with zero bytes on both streams.
3. Tampered `fleet ledger verify` exits `8` with no output, while success prints
   `verified checked=12 total=12`.
4. An unset-state `fleet plan` prints `intent:`, `agent:`, and `skills:` before an environment
   fault.
5. There are “two defects”, followed by a “third” partial-output defect.

## Commands actually run

The three bare commands are not executable in this checkout because `fleet` is not on `PATH`:

```text
fleet run --task ""                 -> exit 127, stdout 0 bytes, stderr 32 bytes
fleet ledger verify                 -> exit 127, stdout 0 bytes, stderr 32 bytes
env -u FLEET_STATE fleet plan        -> exit 127, stdout 0 bytes, stderr 38 bytes
```

I rebuilt the repository binary with:

```text
cargo build --manifest-path 'keel/Cargo.toml' --quiet -> exit 0
```

Using the rebuilt `keel/target/debug/fleet` and temporary state directories:

| Case | Command/result | stdout | stderr |
|---|---|---:|---:|
| Empty task, with required preconditions | `env FLEET_STATE=<tmp> fleet run --task "" --repo . --agent stub` → exit **7** | **0** | **0** |
| Empty-task side effect | same case | ledger has **1** refusal receipt | — |
| Unset-state plan | `env -u FLEET_STATE fleet plan "add a --version flag"` → exit **3** | **55** bytes: 3 claimed lines | **264** bytes: environment reason |
| Fresh status | `env FLEET_STATE=<fresh-tmp> fleet status --json` → exit **0** | **452** bytes, `checked=0,total=0,empty=true` | **0** |
| Fresh ledger verify | `env FLEET_STATE=<fresh-tmp> fleet ledger verify` → exit **6** | **0** | **0** |
| 12-row ledger success | 12 `ledger append --event note --body '{}'` calls, each exit **0**; then `fleet ledger verify` → exit **0** | **29** bytes: `verified checked=12 total=12` | **0** |
| Tampered ledger | changed the first row’s event, then `fleet ledger verify` → exit **8** | **0** | **0** |
| Restored ledger | restored the saved chain, then `fleet ledger verify` → exit **0** | **29** bytes | **0** |

The SOW portion was also exercised in isolated temporary state:

```text
fleet plan "add a --version flag"                 -> exit 0
fleet plan "make me a sandwich"                  -> exit 7, 0/154 bytes, useful candidates
fleet run ... --repo . --agent stub (no SOW)      -> exit 7, 0/290 bytes, named SOW reason
fleet sow --task "do it"                          -> exit 7, 0/806 bytes, named SOW refusal
valid fleet sow --task <filled multiline task>    -> exit 9, 1502/194 bytes
fleet sow accept --id <reported id>               -> exit 0, 125/0 bytes
fleet run ... after acceptance                    -> exit 7, 0/397 bytes, dirty target refusal
```

The accepted-run refusal means the supplied document does not provide a clean repository fixture,
exact task text, exact SOW id extraction, tamper edit, restore command, or transcript sufficient to
reproduce its claimed successful path.

## Independent verifier

Required command, run exactly:

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

The corpus log at capture time published `DENOMINATOR checked=34 total=34 excluded=69 caught=27`
and included timeout failures. This is red evidence, not a green acceptance gate. The required
command also explicitly skipped mutation testing, so it cannot establish B8 mutation evidence.

## Findings

### F1 — REJECT: the deliverable is not a reproducible user walkthrough

The bare `fleet` commands fail with shell exit `127`, and the document does not say how to build,
locate, or invoke the binary. The successful 12-row result is an assertion, not a runnable recipe;
the document omits the commands that create the rows, tamper the chain, and restore it. The claimed
end-to-end success could not be replayed from the document.

Required change: provide copy-pasteable commands, binary/setup preconditions, an isolated state
directory, a clean repository fixture, the exact SOW input/id flow, the tamper operation, the
restore operation, and per-step exit/output evidence.

### F2 — REJECT: the empty-task and tampered-ledger defects are still live

The complete empty-task command returned exit `7` with `0/0` bytes while writing one receipt. The
tampered 12-row ledger returned exit `8` with `0/0` bytes. The code path explains both: the empty
task appends a refusal receipt and returns without printing, while `ledger_verify` uses `?` before
its success `println!` (`keel/fleet/src/main.rs:856-1173` and `2995-3000`).

Required change: preserve the typed exit codes and receipts, but emit a human-readable named reason
on both refusal paths; add assertions for both stream/output behavior.

### F3 — REJECT: the partial-plan defect remains, and an omitted vacuous success is present

The unset-state plan reproduced exit `3` after printing the three plan lines (`55` stdout bytes),
followed by the environment diagnostic. Separately, a fresh `status --json` returned exit `0` with
`checked=0,total=0,empty=true`. That is a vacuous success under the repository’s published law that
zero checked inputs fail. A fresh `ledger verify` also returned exit `6` silently. The walkthrough
does not classify or disclose these fresh-state cases.

Required change: decide and document the empty-store contract, assert it, fix the claim/code boundary
to match, and either validate environment prerequisites before rendering a plan or explicitly make
the partial output part of the contract.

### F4 — REJECT: count and classification arithmetic is incomplete

The document says “two defects” but lists two and then identifies “a third”. It publishes no
denominator for the claimed multi-step flow, no per-step pass/fail total, and no classification of
the fresh empty-ledger/status cases. `12/12` is only the ledger verifier’s row denominator, not the
walkthrough denominator.

Required change: publish one explicit denominator for the flow and separate denominators for ledger
rows, output checks, and any detector population; classify every attempted and omitted case.

### F5 — REJECT: B8 acceptance machinery is absent

`tests/corpus/M9.sh` does not exist. No denominator or mutation result for M9 exists. The required
verifier is red at corpus and skips mutants under `FLEET_MUTANTS=0`. Therefore B8’s acceptance
criteria are not met even though the walkthrough identified genuine defects.

Required change: implement M9 over every reachable refusable surface, publish a non-zero
`checked/total` denominator, mutation-test both the reason assertion and its negative control, then
rerun the full verifier and require exit `0` with no failed stages.

## Verdict

**REJECT**

