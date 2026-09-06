# Adversarial review: opus-walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-REAL-RUN`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
Verdict: **REJECT**

## Contract resolution

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The exact heading search
returned exit 1 with no output. The only matching contract is the unchecked B8 item at
`handover/BACKLOG.md:92-106`, which requires fixing all three paths, adding `tests/corpus/M9.sh`,
publishing its denominator, mutation-testing it, and getting `verify.sh` green.

## What the deliverable claims

- A complete `plan` → SOW → `run` → `status` → ledger verification → tamper → restore flow worked
  end to end (`opus-walkthrough.md:3-6`).
- `fleet run --task ""` exits 7 with zero bytes (`:11`).
- Tampered `fleet ledger verify` exits 8 with no output; the clean path reports `checked=12 total=12`
  (`:12-14`).
- Unset-state `fleet plan` emits `intent`, `agent`, and `skills` before its environment refusal
  (`:16-18`).

## Commands actually run and observations

All runs used the built binary at `keel/target/debug/fleet`; isolated state was under
`/tmp/fleet-opus-review.2rdoAt` or `/tmp/fleet-opus-flow.E2yJws`.

1. `env FLEET_STATE="$STATE" keel/target/debug/fleet run --task ""` returned **7**, but emitted
   **187 bytes**: `fleet: run: --repo is required` plus usage. The cited command is incomplete and
   does not reproduce the claimed silent path.
2. The fully shaped blank-task command,
   `env FLEET_STATE="$STATE" keel/target/debug/fleet run --task "" --repo . --agent stub`, returned
   **7** and emitted **0 bytes**. The silent refusal is real, but the deliverable must cite this
   actual command, including the required arguments.
3. `env -u FLEET_STATE HOME="$HOME_TMP" keel/target/debug/fleet plan "add --version"` returned
   **3** and emitted **319 bytes**, beginning:

   ```text
   intent: implement a change
   agent: builder
   skills: rust
   fleet: environment fault: FLEET_STATE is not set.
   ```

   This confirms the partial-output defect.
4. A complete SOW task returned **9** with an SOW id, and `fleet sow accept --id <that-id>` returned
   **0** with `SOW_ACCEPTED`. Running the shortened task text shown by the walkthrough afterward
   returned **7**: `task has no accepted SOW`, with a different task-bound SOW id. The exact task
   body is not present in the deliverable, so its claimed accepted-run sequence cannot be replayed.
5. On an isolated ledger, clean `fleet ledger verify` returned **0** with
   `verified checked=1 total=1`. After changing one hexadecimal character in the chain, the same
   command returned **8** and emitted **0 bytes**. Restoring the saved chain returned **0** with
   `verified checked=1 total=1`. The silent tamper defect is reproducible; `12/12` is not
   independently reproducible from the supplied steps.
6. `env FLEET_STATE="$EMPTY_STATE" keel/target/debug/fleet status --json` returned **0** with:

   ```json
   {"checked":0,"total":0,"empty":true,"groups":[{"state":"DONE","checked":0,"total":0,"tasks":[]},{"state":"PENDING","checked":0,"total":0,"tasks":[]},{"state":"FAILED","checked":0,"total":0,"tasks":[]},{"state":"NEEDS-ITERATION","checked":0,"total":0,"tasks":[]}]}
   ```

   This adjacent status step passes vacuously on an empty store, violating the handover law that a
   zero-input check fails or is explicitly treated as unmeasured.

## Required independent verifier

Command: `FLEET_MUTANTS=0 bash verify.sh`  
Exit code: **6**

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
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The failed corpus log includes these red observations:

```text
TIMEOUT A8.sh exceeded 30s -- treated as FAILED
TIMEOUT B10.sh exceeded 30s -- treated as FAILED
TIMEOUT C1.sh exceeded 30s -- treated as FAILED
TIMEOUT C2.sh exceeded 30s -- treated as FAILED
TIMEOUT C3.sh exceeded 30s -- treated as FAILED
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
TIMEOUT C8.sh exceeded 30s -- treated as FAILED
TIMEOUT C9.sh exceeded 30s -- treated as FAILED
TIMEOUT S4.sh exceeded 30s -- treated as FAILED
TIMEOUT S6.sh exceeded 30s -- treated as FAILED
TIMEOUT S9.sh exceeded 30s -- treated as FAILED
TIMEOUT T1.sh exceeded 30s -- treated as FAILED
TIMEOUT T5.sh exceeded 30s -- treated as FAILED
TIMEOUT T6.sh exceeded 30s -- treated as FAILED
TIMEOUT T15.sh exceeded 30s -- treated as FAILED
TIMEOUT T20.sh exceeded 30s -- treated as FAILED
M6: 23 of 24 documented commands exist (denominator: 24)
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

## Findings

1. **REJECT — the named acceptance contract is absent.** A reviewer cannot evaluate an item that
   does not exist in the backlog. B8 is unchecked and is a materially different contract.
2. **REJECT — the primary reproduction command is false as written.** `fleet run --task ""` is
   rejected at argument parsing and prints usage. Only after adding `--repo` and `--agent` does the
   silent blank-task refusal occur.
3. **REJECT — the end-to-end claim is not reproducible.** The file gives no exact task body, binary
   path, state path, repository fixture, agent, SOW id, receipt setup, tamper operation, or restore
   command. The only replayable SOW attempt showed that task-bound identity matters and the
   shortened follow-up is refused.
4. **REJECT — the arithmetic/evidence is incomplete.** `12/12` is a denominator for an unspecified
   success-path observation, not a published denominator for the full walkthrough. There is no
   case table, no excluded-case count, and no evidence establishing 12 receipts. My reproducible
   isolated ledger had `checked=1 total=1`.
5. **REJECT — the review cannot imply a green project gate.** The required verifier is red at exit 6,
   with one failed stage and one skipped stage; the corpus also reports timeouts and M6 at 23/24.

## Exactly what must change

1. Restore a real `opus-walkthrough` backlog item with explicit acceptance criteria, or explicitly
   bind this artifact to B8. Do not treat the B8 reference as an implicit completed contract.
2. Replace the prose-only walkthrough with a replayable transcript or script: quote the binary path,
   create state with `mktemp -d`, publish the exact SOW task text, capture the generated id, use the
   exact accepted task for `run` with `--repo` and `--agent`, and show stdout, stderr, byte counts,
   exit codes, receipt counts, tamper mutation, and restoration.
3. Correct the blank-task entry to distinguish the malformed command’s usage output from the fully
   specified command’s silent refusal.
4. Publish a case table whose checked and total values add up, including all hard cases and exclusions;
   substantiate any `12/12` claim from the transcript.
5. If B8 is the intended contract, fix the three user-facing refusal paths, add and mutation-test
   M9 with a non-zero denominator, and rerun `FLEET_MUTANTS=0 bash verify.sh` until its final result
   is green. Separately resolve the zero-input `status --json` policy instead of allowing
   `checked=0,total=0` to exit 0 silently.
