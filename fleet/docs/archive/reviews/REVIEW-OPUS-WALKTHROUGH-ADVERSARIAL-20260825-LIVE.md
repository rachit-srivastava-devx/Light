# Adversarial review: opus-walkthrough

Date: 2026-08-25
Deliverable: `docs/delta.d/opus-walkthrough.md`
Contract: `handover/BACKLOG.md`, item B8 / `opus-walkthrough`

## What was claimed

The walkthrough reports three user-visible failure paths:

1. `fleet run --task ""` exits `7` with no stdout or stderr.
2. `fleet ledger verify` exits `8` on a tampered chain with no output.
3. `fleet plan` prints `intent:`, `agent:`, and `skills:` before failing with an unset
   `FLEET_STATE` environment fault.

It proposes a detector for every non-zero exit, but it does not claim that B8 is fixed.

## What I actually ran

All commands were run from the quoted repository path, using the built real binary
`keel/target/debug/fleet`. No source file, deliverable, backlog, or repository ledger was edited;
temporary state and the existing build output were used as needed to run the product.

### Direct user-path checks

- `cargo build --manifest-path keel/Cargo.toml --quiet` -> exit `0`.
- `fleet plan "add a --version flag"` with a temporary `FLEET_STATE` -> exit `0`, 498 bytes.
- `fleet plan "make me a sandwich"` -> exit `7`, 153 bytes, with a refusal and near matches.
- `fleet run --task "" --repo "$PWD" --agent stub` -> exit `7`, **0 bytes**.
- `fleet ledger append --event note --body '{"review":1}'` -> exit `0`.
- `fleet ledger verify` before tampering -> exit `0`, `verified checked=1 total=1`.
- Changed the first row's `hash` in the temporary `ledger/chain.jsonl`, then ran
  `fleet ledger verify` -> exit `8`, **0 bytes**.
- Restored the saved chain and ran `fleet ledger verify` -> exit `0`,
  `verified checked=1 total=1`.
- `env -u FLEET_STATE fleet plan "add a --version flag"` -> exit `3`, 318 bytes. The
  first three lines were printed before `fleet: environment fault: FLEET_STATE is not set.`

The full SOW sequence was also attempted. Planning, refusal, SOW creation (`9`), and SOW
acceptance (`0`) worked. The subsequent run was refused because this shared checkout had
uncommitted changes, so I do not count that as evidence of a successful end-to-end execution.

### Contract checks

- `bash tests/corpus/M9.sh` -> exit `127`: `tests/corpus/M9.sh: No such file or directory`.
  The detector required by B8 does not exist.
- `FLEET_MUTANTS=0 bash verify.sh` -> red in the corpus stage. The real corpus output ended:

  ```text
  DENOMINATOR checked=34 total=34 excluded=69 caught=12
  ```

  At least one detector timed out and the corpus runner returned `1`; `verify.sh` therefore
  cannot be green. The expected typed gate result from its stage logic is exit `6` with one
  failed stage and one skipped mutation stage. The process was run concurrently with other
  reviewers sharing this checkout, so the wrapper's final stdout line was not recoverable;
  the red corpus result and denominator are recoverable evidence, not a green claim.
- `bash bin/mutants-gate.sh` was started by the required review sequence and reached an isolated
  `cargo-mutants` build. Its final stdout and exit code were not recoverable from the concurrent
  wrapper, so no mutation score or exit code is accepted as evidence.

## Findings

### F1 — REJECT: the three defects remain reproducible

The walkthrough is accurate as a defect report, but the B8 acceptance criterion requires all
three paths to be fixed. The live binary still produces the exact failures the backlog names:
empty task and tampered ledger mismatch are silent; unset-state planning emits partial output.
The implementation matches the observations: the empty-task branch records a receipt and returns
without printing (`keel/fleet/src/main.rs:888-907`), `ledger_verify` prints only after
`verify_rows` succeeds (`keel/fleet/src/main.rs:2995-3000`), and `plan_command` prints the first
three fields before calling `state_dir()` (`keel/fleet/src/main.rs:2655-2674`).

### F2 — REJECT: required M9 detector is absent

The backlog requires `tests/corpus/M9.sh`, a published denominator, and mutation testing. The
file is absent and `bash tests/corpus/M9.sh` exits `127`. The existing corpus inventory contains
7 `M*` detectors and 103 total detector scripts, but no M9. Therefore the required property is
not mechanised and there is no denominator for the refusable surface it claims to cover.

### F3 — REJECT: the required verifier is red

The verifier's corpus stage measured `34/34` checked inputs, excluded `69`, and caught `12`.
That is a real non-zero failure, not a vacuous pass. B8 explicitly requires `verify.sh` green.

## Verdict

**REJECT**

## Exactly what must change

1. Fix the empty-task path to emit a reason before returning exit `7`.
2. Fix `ledger verify` to emit a mismatch reason, including checked/total where available,
   before returning exit `8`.
3. Check `FLEET_STATE` before printing plan output, or make the partial-output behavior an
   explicit accepted contract; B8 currently requires every non-zero exit to explain itself.
4. Add `tests/corpus/M9.sh` covering every refusable surface, with a non-zero denominator and
   an assertion that each non-zero command emits a reason naming the cause. A surface that
   cannot be triggered must fail the detector.
5. Run mutation testing against M9, publish its caught/total result, restore all temporary
   mutations, and rerun `FLEET_MUTANTS=0 bash verify.sh` to a captured final exit code and
   summary with zero failed stages.
