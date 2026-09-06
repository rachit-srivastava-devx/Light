# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260825-CODEX-L8-INDEPENDENT`

Deliverable: `docs/delta.d/opus-walkthrough.md`

Contract used: B8 in `handover/BACKLOG.md`. There is no literal `opus-walkthrough`
item in the current `handover/BACKLOG.md`; B8 is the only item that names this
deliverable (`handover/BACKLOG.md:92-106`). That missing contract is itself a
review finding.

## What the deliverable claims

- A complete plan/refusal/SOW/run/status/ledger-tamper/restore flow worked end to
  end (`docs/delta.d/opus-walkthrough.md:3-6`).
- `fleet run --task ""` exits 7 with zero bytes on both streams
  (`docs/delta.d/opus-walkthrough.md:11`).
- Tampered-ledger verification exits 8 with no output
  (`docs/delta.d/opus-walkthrough.md:12-14`).
- An unset `FLEET_STATE` plan emits partial plan output before an environment fault
  (`docs/delta.d/opus-walkthrough.md:16-18`).
- These should be generalized into a check that every non-zero exit names its reason
  (`docs/delta.d/opus-walkthrough.md:20-22`).

## What I ran

No Git command was run. The binary was the existing
`keel/target/release/fleet`. Temporary state directories were created with
`mktemp -d`; no repository deliverable, `DELTA.md`, `BACKLOG.md`, or `keel/` file
was edited.

| Case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` exactly as written | 7 | 0 bytes | 187 bytes | Does not reproduce the claim; it explains that `--repo` is required and prints usage. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 bytes | 0 bytes | Reproduces the underlying silent empty-task refusal once required arguments are supplied. |
| `FLEET_STATE=<tmp> fleet plan` | 7 | 0 bytes | 41 bytes | Usage refusal, not the partial-plan case. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 bytes | 264 bytes | Reproduces `intent`, `agent`, `skills` before the named missing-state error. |
| `FLEET_STATE=<fresh> fleet ledger verify` | 6 | 0 bytes | 0 bytes | Additional silent non-zero path omitted by the walkthrough. |
| `FLEET_STATE=<state> fleet sow --task "review the fleet walkthrough"` | 7 | 0 bytes | 806 bytes | Refusal explains the missing SOW citation and prints a corrected template. |
| `FLEET_STATE=<state> fleet ledger verify` after that refusal receipt | 0 | 27 bytes | 0 bytes | `verified checked=1 total=1`. |
| Tamper first ledger `seq`, then `FLEET_STATE=<state> fleet ledger verify` | 8 | 0 bytes | 0 bytes | Confirms the silent mismatch claim. |
| `FLEET_STATE=<fresh> fleet status --json` | 0 | 452 bytes | 0 bytes | Vacuous success: JSON reports `checked: 0`, `total: 0`, `empty: true`. |

I also exercised the documented flow with the complete task shape from
`README.md:38-49`: plan exited 0; run before SOW exited 7; SOW creation exited 9;
acceptance exited 0; run after acceptance exited 7 because the shared target repo
was dirty; status exited 0; ledger verification exited 0 with `checked=5 total=5`.
The walkthrough supplies no clean-target precondition or setup, so its claimed
end-to-end success is not reproducible from the document in this checkout.

## Arithmetic and coverage

The walkthrough publishes no `checked/total` denominator for the flow, the refusal
surfaces, or the proposed “every non-zero exit” check. “Two defects” followed by a
“third” is also not a measured classification. The fresh-ledger mismatch and
`status --json` empty-store success are hard cases that were not classified in the
document.

The required independent gate was run exactly:

```text
$ FLEET_MUTANTS=0 bash verify.sh
== fleet verify ==
  .... fmt                       -> ok
  .... clippy -D warn            -> ok
  .... unit tests                -> ok
  .... acceptance builds         -> ok
  .... cargo-deny                -> ok
  .... cargo-audit               -> ok
  .... secrets                   -> ok
  .... acceptance                -> ok
  .... readme                    -> ok
  .... swarm                     -> ok
  .... policy                    -> ok
  SKIPPED WITH A REASON mutants  (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke              -> ok
  .... pytest                    -> ok
  .... detectors                 -> ok
  .... corpus                    -> FAIL (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
exit 6
```

The corpus log reported timed-out detectors as failures and ended with:

```text
M2: 34085 files in the tree (>15000). Build artifacts are almost certainly inside the repo; set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
DENOMINATOR checked=34 total=34 excluded=69 caught=19
```

This is red evidence, not a green acceptance gate. `tests/corpus/M9.sh` is absent,
so the B8 mechanization requirement is not present either.

## Findings

1. **REJECT — no explicit acceptance item.** The requested `opus-walkthrough`
   contract does not exist in `BACKLOG.md`. B8 names the file, but its actual
   acceptance requires fixing three paths, adding M9, publishing its denominator,
   mutation-testing it, and getting `verify.sh` green. The document is evidence,
   not completion of that contract.

2. **REJECT — the primary reproduction is inaccurate as written.** The exact
   command in the document omits mandatory `--repo` and `--agent` arguments and
   emits 187 bytes explaining that omission. Only the qualified command reproduces
   the silent empty-task path.

3. **REJECT — the walkthrough is not independently reproducible.** It gives no
   exact binary, state directory, task text, clean Git-target setup, output capture,
   or tamper operation. A direct user run therefore stops at the dirty-target
   refusal before the claimed successful run.

4. **REJECT — denominator and hard-case coverage are missing.** The document cannot
   establish how many commands were tested, how many non-zero surfaces were
   enumerated, or what was excluded. It omits two observed cases: silent fresh-ledger
   verification and vacuous empty-store status success.

5. **REJECT — B8 remains acceptance-incomplete.** M9 is absent and the required
   verifier is red: 14/16 stages passed, 1 failed, 1 skipped; corpus denominator
   `34 checked / 34 total`, with 19 caught failures.

## Verdict

**REJECT**

## Exactly what must change

1. Restore an explicit `opus-walkthrough` backlog item, or explicitly bind this
   review and the deliverable to B8 with its complete acceptance criteria.
2. Rewrite the walkthrough as a self-contained transcript with exact setup,
   commands, exit codes, stdout/stderr byte counts, a clean target precondition, and
   a deterministic tamper step. Correct the empty-task command or label it as a
   shorthand that does not reproduce the defect.
3. Publish a hand-checkable `checked/total` denominator for the complete flow and
   for the enumerated non-zero/refusal surfaces; classify fresh-ledger verification,
   empty-store status, environment faults, argument faults, and target-repo faults.
4. Fix the three B8 user-facing silent paths, add `tests/corpus/M9.sh` with its
   denominator, mutation-test the detector in both directions, and rerun the exact
   verifier until the final output is green.
