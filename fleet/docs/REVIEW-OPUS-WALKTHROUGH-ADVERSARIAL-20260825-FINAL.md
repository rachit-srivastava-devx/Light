# Adversarial review — opus walkthrough

Verdict: **REJECT**

Reviewed: `docs/delta.d/opus-walkthrough.md` against the current checkout on 2026-08-25.
I did not run git and did not edit the deliverable, `DELTA.md`, `BACKLOG.md`, or `keel/`.

## Contract problem

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md` and therefore no
acceptance criteria to apply. The only occurrence is B8's reference at `handover/BACKLOG.md:94`.
That is a contract defect, not evidence that this deliverable passes.

## What the document claims

- A complete `plan` → refusal → SOW refusal → accepted SOW → `run` → `status` → ledger verify →
  tamper → restore flow works end to end.
- The SOW refusal identifies the missing citation, gives the line, and prints a corrected template.
- `fleet run --task ""` exits 7 with zero bytes on both streams.
- A tampered `fleet ledger verify` exits 8 with no output; the success path reports
  `verified checked=12 total=12`.
- An unset `FLEET_STATE` plan prints partial output before its environment refusal.

## What I actually ran

All command tests used `keel/target/release/fleet` and isolated temporary `FLEET_STATE`
directories. Exit codes and byte counts were captured immediately.

| Command | Result | Observation |
|---|---:|---|
| `fleet plan "add a --version flag to the cli"` | rc 0, 498 bytes | Normal plan rendered; `commands: 3 planned (denominator: 3)`. |
| `fleet plan "make me a sandwich"` | rc 7, 153 bytes | Refused with three useful candidates. |
| `fleet run --task "add a --version flag" --repo "$PWD" --agent stub` before SOW | rc 7, 289 bytes | Refusal named the SOW id and accept command. |
| `fleet sow --task "add a --version flag"` | rc 7, 805 bytes | Refusal named the missing citation and printed the corrected template. |
| Exact cited `fleet run --task ""` | rc 7; stdout 0, stderr 187 | **Does not reproduce the claim**: it stops first with `fleet: run: --repo is required.` |
| `fleet run --task "" --repo "$PWD" --agent stub` | rc 7; stdout 0, stderr 0 | Reproduces the underlying silent empty-task refusal. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | rc 3; stdout 55, stderr 264 | Reproduced partial `intent:`, `agent:`, `skills:` output before the environment fault. |

I also ran the accepted-SOW path. The filled SOW returned rc 9 and `fleet sow accept --id <id>`
returned rc 0. The subsequent `run` returned rc 7 because the target repository had uncommitted
changes. Retrying against the sibling `../fleet` repository produced the same dirty-tree refusal.
Therefore the claimed successful `run`/`status` portion was not observed in this checkout. The
`status` observed after that refusal was an empty-store report (`0 of 0`), not a completed task.

The isolated ledger claim does reproduce when the ledger is populated independently:

```text
12 note appends: 0 failures
ledger verify before tamper: rc=0
verified checked=12 total=12
ledger verify after tamper: rc=8, 0 bytes
ledger verify after restore: rc=0
verified checked=12 total=12
```

This validates the ledger failure behavior, but it is not evidence of the document's claimed
successful task flow.

## Arithmetic and known-cheat checks

- The `12 of 12` ledger denominator is internally consistent: twelve appends were checked.
- The walkthrough publishes no denominator for its overall command coverage. It gives three
  findings, but does not state how many surfaces/cases were attempted, how many were untriggerable,
  or whether any were omitted. This fails the repository's explicit denominator rule.
- A fresh `fleet status --json` returned rc 0 with `checked: 0`, `total: 0`, and `empty: true`.
  That is a vacuous clean result. A fresh `fleet ledger verify` returned rc 6 with zero bytes.
  These adjacent empty-input behaviors should be classified in the walkthrough or explicitly
  excluded; silently omitting them would hide a known project-wide failure mode.

## Required independent gate

Command: `FLEET_MUTANTS=0 bash verify.sh`

Observed final output and exit code:

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
  FAIL swarm (see var/verify.log)
  ok   policy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
exit=6
```

The red details were also present in `var/verify.log`: swarm reported `49 passed, 1 failed`,
with `CC1` failing because `1 of 6` concurrent dispatches failed. Corpus reported timeout-driven
failures, including `A4`, `C11`, `C2`, `C6`, `S6`, `S9`, `T1`, and `T15`; its final summary was
`DENOMINATOR checked=34 total=34 excluded=69 caught=12`. The gate is not green.

## Findings and exact changes required

1. **Reject — the acceptance contract is missing.** Add a real `opus-walkthrough` item to
   `handover/BACKLOG.md`, or rename the review target to an existing item and state its exact
   acceptance criteria.
2. **Reject — the primary empty-task command is wrong as written.** Either make the CLI accept
   `fleet run --task ""` without requiring repository arguments, or change the claim to the fully
   specified command and report both behaviors separately.
3. **Reject — end-to-end success is unsupported.** Re-run the accepted-SOW flow against a clean,
   disposable target and include the actual `run`, `status`, successful ledger verification,
   tamper, and restore transcripts with exit codes. If no clean target is available, downgrade the
   claim to “not verified” rather than calling the flow complete.
4. **Reject — publish a walkthrough denominator.** Include every attempted surface, result class,
   exit code, output bytes, and any untriggerable/omitted case. Keep the `12/12` ledger denominator
   separate from the overall walkthrough denominator.
5. If B8 is the intended contract, the documented findings are not completion evidence: B8 also
   requires fixing the three user-facing paths, adding and mutation-testing M9, and obtaining a
   green verifier. None of that is established by this fragment or by the observed gate.

