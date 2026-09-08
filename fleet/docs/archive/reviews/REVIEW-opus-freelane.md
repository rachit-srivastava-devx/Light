# Adversarial review: opus-freelane

## Verdict

**REJECT**

The live prompt lane works when explicitly configured, but the claimed contract is not
reproducibly satisfied. An unknown dialect still silently falls through to the OpenAI request
shape for the built-in lane, the claimed six-input dialect test is not present, and the required
full verifier is red.

## Contract status

`handover/BACKLOG.md` contains no `opus-freelane` item. The contract named by the review request is
therefore absent from this checkout (`rg -n -i opus-freelane handover/BACKLOG.md` exited 1). If this
deliverable is intended to satisfy B10, it also misses B10's explicit M11 requirement: no
`tests/corpus/M11.sh` exists.

## What was claimed

- DevToolBox rejected the old OpenAI-shaped body with HTTP 400 and accepts the prompt-shaped body.
- Per-lane `openai` and `prompt` dialects were added; unknown dialects are refused.
- DevToolBox works end to end through `bin/freelane.sh`, with `null` usage when unmeasured.
- The parser was directly tested on six inputs, including the wrong-shape/prompt-dialect case.
- The real three-lane config fails over from a dead first lane to a working second lane.
- Shell syntax checks pass; the full verifier had not been green at authoring time.

## Commands run and observed results

| Command | Exit | Observation |
|---|---:|---|
| `curl` OpenAI-shaped body to the configured DevToolBox URL | 0 | HTTP 400, `{"error": "Missing \"prompt\" field"}` |
| `curl` prompt-shaped body to the same URL | 0 | HTTP 200, response JSON returned |
| `bash bin/freelane.sh 'KEYLESS_REVIEW_PROBE'` with the wired config | 0 | Answered on `api.llm7.io`, `lane=1/3`; DevToolBox was not exercised |
| Forced DevToolBox: `FREELANE_DIALECT=prompt FREELANE_CONFIG=<empty> bash bin/freelane.sh ...` | 0 | Worked through the consumer; current response was explanatory text, not the documented literal `KEYLESS_OK`; usage fields were all `null` |
| Mixed config with dead built-in first lane and `bin/lanes.conf` | 0 | `transport-7` then BlockRun answered on `lane=2/3` with `18+483=501` tokens |
| Unknown config record `...|klingon` | 3 | Config record was ignored with the claimed diagnostic; remaining default lane failed transport |
| Unknown built-in `FREELANE_DIALECT=klingon` against a local OpenAI-shaped fixture | 0 | **Bug:** returned `SECOND_LANE_OK`; no refusal was printed. The unknown value was treated as OpenAI by the `else` branch at `bin/freelane.sh:187-191` |
| `bash -n bin/freelane.sh` | 0 | Passed |
| `shellcheck -S error bin/freelane.sh` | 0 | Passed |
| `cargo fmt --check` from repo root | 1 | No root `Cargo.toml`; the cited command is not runnable from the documented repo cwd |
| `cargo fmt --manifest-path keel/Cargo.toml --check` | 0 | The correctly scoped format check passed |
| `cargo test --manifest-path keel/Cargo.toml freelane -- --nocapture` | 0 | 5 Rust freelane tests passed; no dialect/body-shape test ran |
| `bash tests/lane/failover.sh` | 3 | Exited during the expected nonzero all-down invocation because `set -e` prevents its `down_rc=$?` assertion from running |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | `14 passed, 1 failed, 1 skipped (denominator: 16 stages)`; corpus failed. `var/verify.log` reports 9 detectors timed out and treated as failed: A4, C11, C2, C22, C6, S6, S9, T1, T15 |

The verifier also reports `DENOMINATOR checked=34 total=34 excluded=69 caught=12` for the detector
corpus. The stage-level result remains failed; the denominator does not turn the failed corpus into
a pass.

## Findings

### 1. Critical: built-in unknown dialect is silently accepted

`bin/freelane.sh:31-38` copies `FREELANE_DIALECT` directly into `LANE_DIALECTS` without calling
`add_lane`. The payload code at `bin/freelane.sh:187-191` selects `prompt` only for the exact
string `prompt` and sends every other value as OpenAI. Reproduction against a local fixture:

```text
FREELANE_DIALECT=klingon ... bash bin/freelane.sh 'fixture prompt'
SECOND_LANE_OK
[resolved_model=served-by-second-lane ...]   exit 0
```

This contradicts the central claim in `docs/delta.d/opus-freelane.md:25-30` and the code comment
at `bin/freelane.sh:27`. Fix by validating the built-in/default lane through the same dialect
allowlist before any request is sent, and add a test proving each unknown input source refuses.

### 2. High: the claimed six-input dialect test has no repository evidence

The targeted Rust run reported exactly 5 freelane tests, all about Rust-side output interpretation.
`rg` found no dialect, `klingon`, or DevToolBox test under `tests/`; the only shell lane test is
`tests/lane/failover.sh`, which uses the OpenAI response shape and has no dialect field. The
write-up publishes “six inputs” but neither names them nor provides a test artifact or result
denominator that can be rerun. Add a hermetic test covering both request/response dialects, bare
string and object errors, unknown dialect refusal, and wrong-shape rejection; publish the exact
`checked/total` result.

### 3. High: acceptance cannot be claimed while the required verifier is red

The required independent command exited `6`. The corpus stage failed after 9 named 30-second
timeouts. This is not green evidence for the deliverable, even though formatting, shellcheck,
unit, acceptance, and other stages passed. Re-run on a quiet machine and record a final green
result, or record and resolve the corpus failures as part of the contract.

### 4. Medium: the named acceptance contract and detector requirement are missing

There is no `opus-freelane` item in `handover/BACKLOG.md`, so the acceptance criteria cannot be
audited as requested. If this is the B10 slice described by the nearby backlog text, M11 is still
absent and the write-up itself admits the lane-through-consumer detector gap at lines 64-65. Add
or restore the exact backlog item, then implement the required detector and mutation-test it.

## Required changes before acceptance

1. Restore or identify the `opus-freelane` acceptance item in `handover/BACKLOG.md`.
2. Reject unknown `FREELANE_DIALECT` values for the built-in lane before network I/O; do not route
   unknown values through the OpenAI `else` branch.
3. Add hermetic request/response tests for `openai`, `prompt`, bare-string errors, object errors,
   unknown dialects, and wrong-shape responses. Publish the denominator and actual results.
4. Add the consumer-path detector required by the actual contract, including mutation evidence if
   that contract is B10/M11.
5. Re-run `FLEET_MUTANTS=0 bash verify.sh` to a terminal green result and paste the real summary,
   including denominators and any failures.
