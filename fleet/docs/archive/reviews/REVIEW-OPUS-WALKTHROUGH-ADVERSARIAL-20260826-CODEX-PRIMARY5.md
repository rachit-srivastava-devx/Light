# Adversarial review — opus walkthrough

Verdict: **REJECT**

## Contract used

`handover/BACKLOG.md` has no item literally named `opus-walkthrough`. The only contract that names
this deliverable is B8 at `handover/BACKLOG.md:92-106`: fix the three reported user-facing paths,
add and mutation-test `tests/corpus/M9.sh` with a published denominator, and finish with a green
`verify.sh`. I used B8 because it is the only matching acceptance item.

## What the document claims

- A full `plan` → SOW → `run` → status → ledger tamper/restore flow was run end to end.
- The valid empty-task run exits 7 silently.
- Tampered ledger verification exits 8 silently, while the clean path prints `verified checked=12 total=12`.
- An unset `FLEET_STATE` makes `plan` print `intent:`, `agent:`, and `skills:` before an environment refusal.
- A future detector should require a reason line for every non-zero exit.

## Commands actually run

The literal `fleet` command was not installed or on `PATH`:

```text
command -v fleet                         -> exit 127
fleet run --task ""                      -> exit 127, stdout 0 B, stderr 32 B
env -u FLEET_STATE fleet plan "..."      -> exit 127, stdout 0 B, stderr 38 B
```

I built the documented release binary with `cargo build --manifest-path keel/Cargo.toml --release`
(`exit 0`), then put `keel/target/release` on `PATH` to test the product itself.

The repository's executable README quickstart was also run independently: `bash tests/acceptance/readme.sh`
-> **exit 0**, `ok the README quickstart runs end to end (denominator: 31 lines)`.

| Scenario | Exit | Output | Observation |
|---|---:|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | stdout 0 B, stderr 187 B | Does **not** reach empty-task validation; it refuses first because `--repo` is missing. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | stdout 0 B, stderr 0 B | Silent empty-task refusal reproduced. |
| 12 ledger appends, clean `fleet ledger verify` | 0 | stdout 29 B, stderr 0 B | `verified checked=12 total=12`. |
| Same 12-row chain after changing one body field, `fleet ledger verify` | 8 | stdout 0 B, stderr 0 B | Silent tamper mismatch reproduced. |
| Restored chain, `fleet ledger verify` | 0 | stdout 29 B, stderr 0 B | Verification succeeds after restoration. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | stdout 55 B, stderr 264 B | Prints the three plan fields before the environment-fault message. |
| Fresh `fleet ledger verify` | 6 | stdout 0 B, stderr 0 B | Additional silent non-zero path omitted by the document. |
| Fresh `fleet status --json` | 0 | stdout 452 B, stderr 0 B | Vacuous success: `checked=0`, `total=0`, `empty=true`. |

The SOW flow was also exercised in an isolated state: refused plan `exit 7`, run before SOW
`exit 7`, vague SOW `exit 7`, complete SOW `exit 9`, accept `exit 0`. The post-acceptance run
against this shared checkout refused `exit 7` because the target tree had uncommitted changes.
The README quickstart proves that an isolated end-to-end path exists, but it does not supply the
missing transcript for the document's particular run.

## Required acceptance checks

`bash tests/corpus/M9.sh` -> **exit 127**:

```text
bash: tests/corpus/M9.sh: No such file or directory
```

`FLEET_MUTANTS=0 bash verify.sh` -> **exit 6**:

```text
ok fmt
ok clippy -D warn
ok unit tests
ok acceptance builds
ok cargo-deny
ok cargo-audit
ok secrets
ok acceptance
ok readme
FAIL swarm (see var/verify.log)
ok policy
SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
ok attest-smoke
ok pytest
ok detectors
FAIL corpus (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
```

The direct `bash tests/acceptance/swarm.sh` run independently returned **exit 1**:

```text
FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
== 49 passed, 1 failed ==
```

The `var/verify.log` path is shared by concurrent workers, so I do not attribute its detailed
corpus lines to this run. The verifier's own stdout is sufficient to establish the corpus stage
as failed. `FLEET_MUTANTS=0` intentionally skipped mutation testing; there is no M9 mutation result.

## Findings

### F1 — The first defect is stated with an invocation that cannot reach it

The document names `fleet run --task ""`, but that invocation lacks required `--repo` and
`--agent` arguments. The actual command emits a useful missing-argument diagnostic. Only the
fully specified command reproduces the silent `EMPTY_TASK` path. The finding is real, but the
transcript must show the exact runnable command and distinguish the two paths.
The source has the silent branch at `keel/fleet/src/main.rs:903-912`, after required-argument
validation.

### F2 — “Full flow works end to end” has no reproducible transcript

There is no binary path, state-directory setup, exact task/SOW payload, command transcript, or
exit code for the claimed successful flow. The separate README quickstart passes, but that is not
the same evidence as the undocumented flow claimed here. Replace the prose with a complete,
isolated transcript, or mark this particular success path unverified.

### F3 — The proposed “every non-zero exit” scope omits known failures

Fresh `ledger verify` exits 6 with no output, and fresh `status --json` exits 0 with
`checked=0,total=0`. The latter violates the repository law that measuring zero inputs must fail.
The detector proposal must enumerate its surface and classify these cases instead of silently
excluding them. `keel/fleet/src/main.rs:2995-3000` prints only after `verify_rows` succeeds;
`keel/fleet/src/main.rs:2831-2844` skips verification when the ledger is empty.

### F4 — B8 acceptance work is absent

The two silent paths and partial plan output remain present in the live binary. `M9` does not
exist, has no denominator, and has no mutation evidence. The required verifier is red at 2 of 16
stages. A findings note is not evidence that B8's implementation acceptance has been met.

### F5 — The suite explanation is too broad

The document says the suite missed all three because it only asserts exit codes. Current tests do
more than that: `tests/acceptance/swarm.sh:249-270` checks 11 refusal surfaces for actionable
output, and `tests/acceptance/p0.sh:110-113` checks empty-task exit and receipt creation. Those
tests still miss the exact empty-task diagnostic, tampered-ledger diagnostic, and partial-plan
output, but the causal claim should be narrowed to those uncovered assertions.

## What must change for ACCEPT

1. Correct the walkthrough commands and add a complete executable transcript with a non-zero
   checked/total denominator for the flow and explicit exclusions.
2. Make the valid empty-task refusal, tampered-ledger mismatch, and unset-state plan path produce
   the intended human-facing reason behavior. Decide and document the empty-store contract, then
   fix the vacuous `status --json` result.
3. Add `tests/corpus/M9.sh` covering every reachable refusable surface, including fresh ledger
   verification and hard-to-trigger paths. It must fail on zero inputs and publish `checked,total`
   plus exclusions/classifications.
4. Mutation-test M9 in both directions: remove/bypass the reason assertion and prove red; restore
   it and prove green.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` in a quiet, attributable environment and record exit 0
   with the final `13/16`-style denominator replaced by the actual green result.
