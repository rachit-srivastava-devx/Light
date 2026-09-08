# Adversarial review: `docs/delta.d/opus-walkthrough.md`

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-215805-IST`  
Date: 2026-08-26 IST  
Contract used: `handover/BACKLOG.md:92-106` (B8, the item that cites this walkthrough)  
Deliverable reviewed: `docs/delta.d/opus-walkthrough.md:1-22`

## Verdict: REJECT

The two important underlying silent failures are real, and the partial-plan output is real. The
deliverable is still not acceptable: one command does not produce the claimed output as written,
the asserted end-to-end success is not reproducible from the document, no walkthrough denominator
is published, an additional silent non-zero surface is omitted, B8's detector does not exist, and
the independent verifier is red.

## What was claimed

1. A complete `plan -> refused plan -> run without SOW -> sow refused -> sow accepted -> run ->
   status -> ledger verify -> tamper -> restore` flow works end to end.
2. `fleet run --task ""` exits 7 with zero bytes on stdout and stderr.
3. Tampered `fleet ledger verify` exits 8 with no output; the clean example prints
   `verified checked=12 total=12`.
4. A prompted `fleet plan` with `FLEET_STATE` unset emits `intent:`, `agent:`, and `skills:` before
   reporting an environment fault.
5. Every non-zero exit should eventually be checked for at least one line naming the reason.

## Runtime setup

- `cargo build --manifest-path keel/Cargo.toml --quiet` -> exit 0.
- `command -v fleet` in the normal shell -> exit 1. The review therefore prepended
  `keel/target/debug` to `PATH`; the document does not state this prerequisite.
- Every Fleet probe used a fresh `mktemp -d` state. No Git command was run against the shared
  checkout.
- The first target was a copied repository polluted by generated graph directories and correctly
  refused as dirty. For the second attempt, those generated directories were moved outside the
  copied target. Fleet then passed its clean-tree check.
- The complete SOW used one leaf, one cited challenge, two named alternatives, an estimate, and one
  edge case. The target did not contain `main.rs`; the deterministic stub is hardcoded to append to
  `main.rs`, a precondition the walkthrough never names.

## Commands actually run

| Probe | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| `fleet run --task ""` | 7 | 0 B / 187 B | **Claim does not reproduce as written.** Validation stops at missing `--repo` and prints usage. |
| `fleet run --task "" --repo <clean-copy> --agent stub` | 7 | 0 B / 0 B | The intended silent `EMPTY_TASK` refusal is real once mandatory arguments are supplied. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 B / 264 B | Reproduced: three plan lines precede the environment reason. |
| `fleet plan "add a --version flag to the cli"` | 0 | 499 B / 0 B | Plan rendered three commands and published `denominator: 3`; nothing executed. |
| `fleet plan "make me a sandwich"` | 7 | 0 B / 154 B | Actionable refusal with three candidates. |
| `fleet run --task <complete-task> --repo <clean-copy> --agent stub` before SOW | 7 | 0 B / 290 B | Correct `SOW_NOT_ACCEPTED` refusal with recovery commands. |
| `fleet sow --task "add a --version flag"` | 7 | 0 B / 806 B | Names the missing citation and prints a corrected template. |
| `fleet sow --task <complete-task>` | 9 | 1414 B / 194 B | `SOW_READY_AWAITING_REVIEW`; SOW id emitted. |
| `fleet sow accept --id <id>` | 0 | 125 B / 0 B | Acceptance receipt emitted. |
| Accepted run on the first copied target | 7 | 0 B / 535 B | Correctly refused generated uncommitted files; no artifact. |
| Accepted run on the cleaned copied target | 6 | 0 B / 204 B | **End-to-end claim failed.** Stub exited without an fd-3 result; no artifact was produced. |
| `fleet status` after that clean attempt | 0 | 2478 B / 0 B | Task remained `PENDING`: `run started; no terminal receipt`. |
| Clean `fleet ledger verify` | 0 | 27 B / 0 B | `verified checked=6 total=6`. |
| Tampered `fleet ledger verify` | 8 | 0 B / 0 B | Silent mismatch reproduced. |
| Restored `fleet ledger verify` | 0 | 27 B / 0 B | Restored to `verified checked=6 total=6`. |
| Fresh-state `fleet ledger verify` | 6 | 0 B / 0 B | **Additional silent non-zero surface omitted by the walkthrough.** |
| Fresh-state `fleet status --json` | 0 | 452 B / 0 B | Vacuous green: `checked=0`, `total=0`, `empty=true`. |

The clean-target run's exact failure was:

```text
fleet: agent stub exited without an fd-3 result. Its non-interactive adapter failed before it could report details; check the Python crew adapter and CLI authentication, or retry with `--agent freelane`.
```

This is not proof that no target can ever complete: `tests/acceptance/readme.sh` passed inside the
independent verifier. It is proof that the walkthrough's end-to-end claim is not reproducible from
the walkthrough. The omitted tracked-`main.rs` fixture is load-bearing, and a green extraction test
does not replace the missing user-run evidence.

## Arithmetic and denominator audit

### Walkthrough

- Reported defects: 3 total (two numbered plus one called “third, smaller”).
- Exact claims reproduced as written: **2 of 3**. The empty-task defect only reproduces after adding
  the omitted mandatory `--repo` and `--agent` arguments.
- Underlying defect classes reproduced after supplying valid arguments: **3 of 3**.
- Successful accepted manual runs in this review: **0 of 2 attempts**; the clean attempt was exit 6.
- Published walkthrough command denominator: **absent**. This is a contract failure on its own.
- The state-dependent historical `checked=12 total=12` is not independently reproducible from the
  document. This run observed `6/6` and `4/4` in two fresh states.

### Independent verifier

- Stage arithmetic: `14 passed + 1 failed + 1 skipped = 16` stages. Correct.
- Corpus files: 105 shell files; excluding `run.sh` and `_selftest.sh` leaves 103 detectors.
- Corpus arithmetic: `34 checked + 69 excluded = 103`. Correct.
- Corpus caught arithmetic: `25 caught = 23 timeouts + 2 non-timeout failures` (`M2`, `M6`).
- Therefore 9 of the 34 checked detectors passed and 25 failed/caught.
- No hard cases were silently dropped by the runner: 69 are explicitly `NOT MECHANISABLE`, and
  timeouts are named failures. However, `M6` is the known detector-on-documentation false positive:
  it reports "fleet arch" missing because the absent command is documented in backticks.

## Known-cheat audit

1. **Wrong validation branch:** `fleet run --task ""` omits required arguments, so the literal
   command cannot prove the stated zero-byte empty-task behavior.
2. **Vacuous success:** fresh `fleet status --json` exits 0 with `checked=0,total=0`; it does publish
   the denominator, but treats measured-nothing as success.
3. **Detector self-fire:** `M6` fires on documentation about a missing command.
4. **Acceptance overclaim:** `tests/corpus/M9.sh` is absent; no M9 denominator or mutation proof
   exists; the three B8 paths remain unfixed.
5. **Done-versus-red mismatch:** B8 is unchecked and the required verifier is red, so no done claim
   is supportable.

No weakened gate margin or `null` rewritten as a numeric zero was found in this deliverable. The
fresh-status `0/0` result is instead an explicitly empty but vacuously successful check.

## Independent verification

Command:

```sh
FLEET_MUTANTS=0 bash verify.sh
```

Typed exit: **6** (one failed stage; `verify.sh` exits 6 on `FAIL > 0`). Real terminal output:

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
  ok   swarm
  .... policy
  ok   policy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke
  ok   attest-smoke
  .... pytest
  ok   pytest
  .... detectors
  ok   detectors
  .... corpus
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The corpus red was not one ambiguous failure: 23 detectors timed out, `M2` returned exit 1 because
the tree had 34,432 files, and `M6` returned exit 1 with `23 of 24 documented commands exist`.

Targeted acceptance was independently rerun:

```text
$ bash tests/acceptance/p0.sh
== P0 acceptance ==
...
  ok   I1 missing FLEET_STATE exits 3
  ok   I2 env fault explains itself (263 bytes)
  ok   I3 doctor still reports with FLEET_STATE unset (444 bytes)
== 34 passed, 0 failed ==
```

Exit: 0. This does not override the full verifier's corpus failure or prove the manually failed
accepted run.

## Exactly what must change before acceptance

1. Make the walkthrough reproducible: publish the exact binary/PATH, `FLEET_STATE`, prompt, complete
   SOW, clean target fixture, tracked `main.rs` precondition, tamper operation, every exit code, and
   stdout/stderr byte count.
2. Correct the empty-task command to include `--repo <clean-target> --agent stub`, or change its
   stated observation to the actual 187-byte missing-repo diagnostic.
3. Remove “the flow works end to end” until a direct user run produces an artifact, passes
   `fleet attest verify <artifact>`, and ends in a terminal status. Publish successful steps / total
   steps; do not substitute the green README test for that user evidence.
4. Expand the failure inventory to include fresh/empty `ledger verify`; publish a checked/total
   denominator for every refusable surface. Do not repeat the state-dependent `12/12` without the
   state-building commands and log.
5. Satisfy B8 itself: fix all three named output paths, add mutation-tested `tests/corpus/M9.sh` with
   a non-zero checked denominator, then rerun `FLEET_MUTANTS=0 bash verify.sh` to a real exit 0 and
   include its final stage denominator.

Until all five changes are evidenced, the acceptance verdict remains **REJECT**.
