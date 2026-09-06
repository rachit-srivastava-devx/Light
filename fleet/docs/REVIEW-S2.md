# Adversarial review — S2 live status stream

**Verdict: REJECT**

Review date: 2026-09-02  
Deliverable: `docs/delta.d/S2.md`  
Contract: `handover/BACKLOG.md`, item S2

## Acceptance score

I split the backlog acceptance sentence into four atomic, observable checks so the denominator is
hand-checkable. **2 of 4 passed; 2 of 4 failed; 0 unclassified.**

| # | Acceptance check | Result | Observation |
|---|---|---|---|
| 1 | `contracts/lane-status.v1.json` exists | PASS | The file exists. |
| 2 | The contract is exercised by a real dispatch | FAIL | The dispatch emitted 7 `lane_status` bodies; **0 of 7** contain the schema-required `ledger_ref`. No runtime/test/verify path validates this schema. |
| 3 | Console output changes live as lane state changes during the actual dispatch | FAIL | One console was kept open across the dispatch. It showed `LANES · 0 tracked` before and still showed `LANES · 0 tracked` after the dispatch. Only closing and reopening it produced `LANES · 5 tracked`. |
| 4 | S2 explicitly excludes the orb-side voice/graph UI | PASS | The exclusion is explicit in the deliverable. |

## What was claimed

1. `lane-status.v1` is a real contract carried as the body of each `lane_status` ledger receipt and
   exercised by a real dispatch.
2. `keel-console` renders lane state live from real ledger events during an actual dispatch.
3. A real dispatch produces five queued lanes and transitions Builder through
   `queued -> running -> failed` in the documented `HARNESS_EXIT` environment.
4. `cargo test -p fleet` reports 95 passed, 0 failed, 1 ignored; swarm reports 45 passed and 8
   pre-existing failures; the full verifier reports 16 passed, 2 failed, 1 skipped out of 19.
5. The orb-side voice/graph UI is out of scope.

## What I actually ran

No Git command targeted the shared checkout. The deliverable's cited `git init` and empty commit were
run only in an isolated `mktemp -d` target repository used by the dispatch.

| Command / user flow | Exit | Actual result |
|---|---:|---|
| `cargo test -p fleet` from the documented repository root | 101 | Cargo could not find `Cargo.toml`. The command is not runnable from the location implied by the document. |
| `cd keel && cargo test -p fleet` | 0 | Cargo's top-level summaries total **105 passed, 0 failed, 1 ignored**: 8 library + 95 main-binary + 1 compile-fail wrapper + 1 doctest. The four trybuild fixture cases ran inside the wrapper. |
| `bash tests/acceptance/swarm.sh` | 0 | **50 passed, 0 failed**, not the deliverable's stale 45/8 result. |
| Isolated `mktemp -d`; cited `git init` + empty commit | 0 | Fresh real target repository created outside the shared checkout. |
| Open actual `fleet console` in a 180x40 tmux PTY, press `4` | 0 | Before dispatch: `LANES · 0 tracked`; ledger absent. |
| Real `swarm dispatch --task "S2 before/after console proof" --repo <isolated-repo> --role verifier` with the cited environment | 6 | Routed all five roles, selected Builder/Codex, and ended `fleet: agent codex refused: HARNESS_EXIT`. |
| Capture the **same still-open console pane** after dispatch | 0 | Still `LANES · 0 tracked`; no lane transition appeared. |
| Close and reopen the console against the same state, press `4` | 0 | `LANES · 5 tracked`; Builder was Failed at seq 9, the other four were Queued. |
| `grep '"event":"lane_status"' <state>/ledger/chain.jsonl` | 0 | Seven real rows resolved. |
| Required-shape/count audit with `jq -s` | 0 | 7 total = 5 queued + 1 running + 1 failed; 5 unique roles; 7/7 valid envelope hashes; **0/7 bodies with `ledger_ref`**; 0/7 bodies with `ts_wall`. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | **17 passed, 1 failed, 1 skipped (denominator: 19)**; `corpus` failed. |

The binary used for the PTY run was not stale: `keel/target/debug/fleet` had a later modification
time than both `keel/fleet/src/main.rs` and `keel/fleet/src/console.rs`.

## Observations and defects

### 1. The console is not live

**Failure ->** The same console process did not change from 0 tracked lanes after the dispatch wrote
all seven events.  
**Cause ->** `console::run` calls `load_state()` once before entering `event_loop`. The loop redraws
the same in-memory `App`; it never reloads, tails, polls, watches, or subscribes to the ledger.  
**Impact ->** The core S2 acceptance criterion is not met. Reopening a snapshot reader after the
dispatch is a before/after demonstration, not live rendering during the dispatch.

Observed PTY states:

```text
before dispatch, same pane: LANES · 0 tracked
after dispatch, same pane:  LANES · 0 tracked
after console reopen:       LANES · 5 tracked
```

### 2. The runtime payload does not satisfy the declared contract

`contracts/lane-status.v1.json` requires:

```json
["schema_version", "lane_id", "role", "state", "ledger_ref"]
```

`append_lane_status` writes only `schema_version`, `lane_id`, `role`, `state`, `agent`, and
`resolved_model` into the receipt body. `seq`, `hash`, and `ts_wall` exist only in the outer receipt
envelope. Therefore the document's statement that the contract shape is carried as the body is
false. Current dispatch denominator: **0 of 7 bodies conform to the required shape.**

This is not merely missing test coverage. Putting the producing receipt's own hash inside the body
would be self-referential because `append_receipt` hashes the body before adding the envelope hash.
The contract boundary must be corrected: either define a body contract without self-reference, or
define a projected lane-status object assembled from body + immutable envelope after append.

The named unit test does not catch this. Its two hand-built bodies also omit `schema_version` and
`ledger_ref`, and its hashes (`blake3:aaa1`, `blake3:aaa2`) violate the contract's 64-hex hash
pattern. The snapshot test is fed `sample_state()`, not a real ledger or live dispatch.

Repository search found **0 runtime or gate references** to `lane-status.v1.json` outside the
contract itself and S2 comments/docs. The new schema is declared, but not exercised.

### 3. Contract governance was bypassed

The repository hard rule says `contracts/*.json` must not be edited without an ADR in `docs/adr/`.
S2 adds `contracts/lane-status.v1.json` and changes the receipt event enum, but `docs/adr/` does not
exist and no ADR was supplied. This independently rejects the change even if runtime behavior were
correct.

### 4. The published command and arithmetic are misleading

- From the repo root, the cited `cargo test -p fleet` exits 101. The reproducible form is
  `cargo test --manifest-path keel/Cargo.toml -p fleet` or `cd keel && cargo test -p fleet`.
- `95 passed, 0 failed, 1 ignored` is only the `src/main.rs` harness. It is not the result of the
  whole Cargo command. Cargo's emitted summaries add to 105 passed and 1 ignored.
- Current swarm is 50/50 green, not 45 passed / 8 failed.
- Current verifier is 17 passed / 1 failed / 1 skipped, not 16 / 2 / 1.

The lane-event arithmetic itself does add up and no lane was quietly dropped:

```text
7 ledger events = 5 queued + 1 running + 1 failed
5 rendered rows after reopen = 4 queued roles + 1 failed Builder
contract conformance = 0 of 7 emitted bodies
```

### 5. Known-cheat audit

| Cheat | Result |
|---|---|
| Gate/margin/detector weakened to pass | No S2 detector exists; no weakening found. Absence of a validator is the defect. |
| `0` substituted for honest `null` | Not observed in the S2 receipts; envelope `exit_code` is `null`. |
| Vacuous zero-input check | No contract-conformance check runs at all. The acceptance claim therefore has no non-zero checked denominator. Independent check: 0/7 conform. |
| Detector fires on its own documentation | No S2 detector was found to test. |
| Done while evidence says partial/blocked | `handover/PROGRESS.md` records S2 as done, then `blocked-on-commit`; S2b later fixed swarm, but the current full verifier is still red on corpus. The deliverable's pasted verifier state is stale. |

## Independent verifier — real output

```text
== fleet verify ==
  .... fmt                         ok   fmt
  .... clippy -D warn              ok   clippy -D warn
  .... unit tests                  ok   unit tests
  .... acceptance builds           ok   acceptance builds
  .... cargo-deny                  ok   cargo-deny
  .... cargo-audit                 ok   cargo-audit
  .... secrets                     ok   secrets
  .... acceptance                  ok   acceptance
  .... readme                      ok   readme
  .... swarm                       ok   swarm
  .... policy                      ok   policy
  .... recur                       ok   recur
  .... semgrep                     ok   semgrep
  .... trivy                       ok   trivy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke                ok   attest-smoke
  .... pytest                      ok   pytest
  .... detectors                   ok   detectors
  .... corpus                      FAIL corpus                     (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
rc=6
```

Corpus published its own denominator:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=28 timeout_contention=0 timeout_confirmed=0 timeout_persistent=25
```

## Exactly what must change before S2 can be accepted

1. **Implement real in-process refresh.** While one console process remains open, ingest appended
   ledger rows by file tail/watch or deterministic polling. Preserve selection and avoid replaying
   already-applied sequence numbers. Prove `queued`, `running`, and terminal state changes in that
   same PTY during one actual dispatch.
2. **Make the contract match a real object.** Define whether `lane-status.v1` is the authored body
   or an envelope-derived projection. Do not put a receipt's self-hash inside its hashed body.
   Materialize and validate the chosen object from a real dispatch, publishing a non-zero result
   such as `7 of 7 valid`.
3. **Add the required ADR.** Record the new contract and receipt-enum change under `docs/adr/`
   before modifying `contracts/*.json`.
4. **Add non-vacuous regression proof outside `tests/acceptance/*`.** The test must fail when refresh
   is disabled, when zero lane inputs are examined, when a required field is absent, and when an
   invalid enum/hash is supplied. Mutation-test the guard and publish `{checked,total}`.
5. **Make the evidence reproducible and current.** Use the root-runnable Cargo command, publish the
   complete Cargo totals, replace stale swarm/verifier output, and retain red exactly as observed.

