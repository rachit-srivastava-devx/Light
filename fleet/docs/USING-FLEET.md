# Using fleet

**Rewritten 2026-09-08.** Everything below was executed against a real debug build of `fleet`
(`target/debug/fleet`, or `target/release/fleet` after `cargo build --release`) and the output
pasted verbatim. Where the previous version of this file showed nested subcommands
(`sow accept`, `ledger verify`, `agents list`, `oracle o1 --suite ...`, `attest verify <ID>`,
`lifecycle states`, `meter show`, `rollback --artifact <ID>`) — **none of that shape exists**. The
real CLI is flat: every command takes only top-level flags, never a second positional
subcommand word. See `docs/DX-AUDIT.md` for the full adversarial sweep this rewrite is based on,
and `docs/QUICKSTART.md` for the shortest path from a fresh clone to something working.

Exit codes are real, captured as `cmd > out.txt 2>&1; echo "EXIT:$?"` on its own line — never
`$?` after a pipe, which reads the pipe's own status and has bitten this project repeatedly.

---

## Careful: two different programs are called `fleet` on this machine

`~/.local/bin/fleet` may be a symlink into a **separate, older `fleet` repository** (a bash
implementation elsewhere on disk). Typing bare `fleet` in a shell can run that one, not this
repo's binary. Until you've confirmed which one `fleet` resolves to, use an explicit path:
`target/debug/fleet` or `target/release/fleet` from this repo's root.

## Setup

```bash
cargo build
export FLEET_STATE_DIR="$PWD/var/fleet"   # NOT "FLEET_STATE" -- see below
export PATH="$PWD/target/debug:$PATH"
mkdir -p "$FLEET_STATE_DIR"
```

**Verified defect, not in the audit:** the config loader (`src/runtime/config.rs`) reads
`Config.state_dir` through `figment::providers::Env::prefixed("FLEET_")`, which maps the field
name `state_dir` to the env var `FLEET_STATE_DIR`. The env var is **`FLEET_STATE_DIR`, not
`FLEET_STATE`.** Setting `FLEET_STATE` (as every earlier revision of this file, and every
`docs/runbook/*.md` and `docs/TROUBLESHOOTING.md` row, told you to) is silently ignored — fleet
falls back to the default `.fleet-state` under the current directory instead, and every
stateful command then fails with a confusing "No such file or directory" on a lock file you never
pointed it at. Reproduced:

```
$ FLEET_STATE=/tmp/x ./target/debug/fleet ledger
fleet: could not open .fleet-state/ledger.lock: No such file or directory (os error 2)
EXIT:7

$ FLEET_STATE_DIR=/tmp/x ./target/debug/fleet ledger      # /tmp/x created first
rows: 0
EXIT:0
```
This file and the four runbooks under `docs/runbook/` have been corrected to `FLEET_STATE_DIR`.

`fleet doctor` does not itself check `FLEET_STATE_DIR` (it reports host capacity, not state
health — see the `status`/`doctor` note below), so a missing/wrong state dir will not show up
there; you'll only see it when a stateful command fails.

---

## The exit codes — the whole contract in six numbers

| Code | Means |
|---|---|
| `0` | ok |
| `2` | clap parse error (bad/missing flags) — not part of fleet's own contract, but the one you'll hit most while learning the real flag names |
| `3` | environment fault, or an unimplemented stub (see "Not implemented yet" below — both currently share exit 3) |
| `6` | invariant violated / not-yet-configured input (fleet's own rules, or missing setup) |
| `7` | refusal — fleet understood you and declined |
| `8` | mismatch — recomputed evidence disagrees with what was stored |
| `9` | reserved for "awaiting human review" in the design; not observed from any command below as of this rewrite |

`7` is not an error in the "something's broken" sense — it's fleet telling you no. Note that `3`
is overloaded: both "your environment is missing something" (e.g. `gate --id <bad-id>`) and "this
subcommand isn't built yet" (the 8 stubs) return it, with no way to tell them apart except by
reading the message. `6` is similarly overloaded across "lane not configured" (`meter`),
"agents.toml has no matching id" (`agents`), and "bad task id" (`lifecycle`). Documented here as
found; not fixed by this docs pass (see `docs/DX-AUDIT.md` §6 "Consistency matrix").

---

## Walkthrough: as far as the real CLI goes today

### 1. See the environment

```bash
$ fleet doctor
cargo: found
git: found
total_memory_mb: 16384
available_memory_mb: 3840
load_avg_1m: 10.66
logical_cores: 8
per_lane_budget_mb: 2048
load_factor_threshold: 2
derived_ram_lanes: 1
decision: allow (concurrency_cap=1)
EXIT:0
```

### 2. See the role table

```bash
$ fleet roles
lead: bandwidth=2 gate=contract
builder: bandwidth=4 gate=implementation
verifier: bandwidth=3 gate=independent-verification
designer: bandwidth=2 gate=design-a11y
meter: bandwidth=2 gate=budget
EXIT:0
```

### 3. `fleet plan` — no positional argument exists

The old walkthrough opened with `fleet plan "add a --version flag to the cli"`. That fails:

```
$ fleet plan "add a --version flag to the cli"
error: unexpected argument 'add a --version flag to the cli' found
Usage: fleet plan [OPTIONS]
EXIT:2
```

`fleet plan`'s only flag is `--model <MODEL>` (default `fleet-cli`); there is no flag that takes
English intent. Called correctly it ignores the model value and prints a fixed template — it is a
doc-scaffold generator, not the "name intent/agent/skills/lane" tool the old docs described:

```
$ fleet plan
# fleet acceptance checks — DRAFT
drafting_model: fleet-cli
blueprint_state: DRAFT
check_1: every SOW acceptance threshold is measurable and linked to the recorded request hash.
check_2: every feature leaf has explicit inputs, outputs, acceptance, and design_decision=none.
check_3: every challenge cites docs/design/FAILURE-CORPUS.md or learn/THREAD-LESSONS.md and names a trigger and mitigation.
check_4: every clarification is business or technical, linked to a gap, and every blocking row is answered before gate exit 0.
EXIT:0
```

### 4. `fleet sow` — the real shape, and what it actually validates

`--help` says `fleet sow --text <TEXT> --intent-hash <INTENT_HASH>`. `--text` is the **literal SOW
body** (not a file path — `src/dispatch/plan_cmd.rs` passes `args.text` straight into
`fleet_plan::validate_sow_text`), and `--intent-hash` must match a `source_intent_hash: <value>`
line inside that body verbatim. Six section headings are required, each as its own `## Heading`
line (`crates/fleet-plan/src/intake/sow_checks.rs::section_body`); a `request:` line; a measurable
threshold (`%`, `p95`, or `exit 0`); an explicit non-goal (`not`/`out of scope`/`excluded`/
`forbidden`); and — layered on top by a separate content probe (`fleet-scan`'s `BusinessProbe`) —
the body must contain a success-metric word (`metric`, `kpi`, `success`, `measure`, `target`,
`goal`) and an audience word (`user`, `customer`, `team`, `client`, `audience`, `stakeholder`).
None of this is in `--help`, and the previous docs described a different vocabulary entirely
(`leaves:`, `challenges:`, `alternatives:`) that the real checker never looks at.

A body that satisfies all of it, run for real:

```bash
$ cat sow.txt
source_intent_hash: abc123
request: add a --version flag to the cli

## Request restatement
Add a --version flag that prints the semver and exits 0.

## Built for
CLI users and the fleet team; the success metric is that --version works.

## Must do
- Add a --version flag
- Print semver and exit 0

## Explicitly will not do
- Not adding any other flags
- Out of scope: build metadata

## Done when
--version prints a semver and exits 0

## Acceptance threshold
100% of runs of `fleet --version` exit 0

$ fleet sow --text "$(cat sow.txt)" --intent-hash abc123
ok: sow valid
EXIT:0
```

There is no `sow accept` subcommand — `--help` and running `fleet sow accept --id x` both confirm
it (`error: unexpected argument 'accept' found`, exit 2). `fleet sow` either refuses (exit 7, with
the specific missing pieces listed) or prints `ok: sow valid` (exit 0). It does not write anything
to the ledger by itself — running `fleet ledger` right after still shows `rows: 0`. There is
currently no observable link between a valid SOW and `fleet run`/`fleet swarm` refusing or
proceeding; the "no work without an accepted SOW" gate described in the previous walkthrough was
not reproduced against this binary.

### 5. `fleet run` — the flagship command, now completing (was hanging)

`--help`: `fleet run --repo <REPO> --task <TASK>`. No `--agent` flag exists (the old docs' `--agent
<stub|env-probe|freelane>` is not accepted). **`docs/DX-AUDIT.md` recorded this command hanging
indefinitely earlier the same day** (S1-1, zero bytes for a 15s window with stdin closed). Retested
during this docs pass against the same binary (unchanged mtime), `fleet run` now completes
reliably in ~8s — a fix appears to have landed in `src/` concurrently while this docs pass was
running (other agents are actively working `src/`; see `docs/NIGHT-PLAN.md` lane 1). Verified 3
consecutive times, `timeout 15`/`timeout 20`/`timeout 60`, all exit before the timeout:

```bash
$ git init -q /tmp/scratch-repo && git -C /tmp/scratch-repo commit --allow-empty -q -m init
$ time timeout 15 ./target/debug/fleet run --repo /tmp/scratch-repo --task "add a version flag" < /dev/null
fleet: run: stage Event starting
fleet: run: stage Classify starting
fleet: run: stage Scan starting
fleet: run: stage Plan starting
fleet: run: stage Dispatch starting
fleet: run: stage Verify starting
fleet: verify: running `cargo test --workspace` (budget 7.988859584s)
fleet: verify: running `cargo mutants` (budget ...)
...
fleet: Verify("gate(s) failed: unit tests: NonZeroExit(101); mutants: NonZeroExit(1); ...")
EXIT:7
( ... )  3.62s user 1.70s system 65% cpu 8.115 total
```
It refuses with exit 7 because `run` executes the *target repo's own* verify gates
(`cargo test --workspace`, `cargo mutants`, `semgrep`, `trivy`, ...) and `/tmp/scratch-repo` is an
empty git init with no `Cargo.toml` — those gates legitimately fail against it. This is expected
for a throwaway scratch repo, not a defect. **If you see `fleet run` sit with zero output past
~20s, that matches the audit's hang** — retry once with `timeout 60` before treating it as stuck;
this session saw it complete every time once given a real budget.

### 6. `fleet oracle` / `fleet gate` (no args) — also now completing (were hanging)

Same story as `run`: `docs/DX-AUDIT.md` recorded both hanging for a full 60s with zero output
(S1-2, S1-3). Retested here, both now complete in ~8-10s and run the same gate pipeline `run`
does, refusing with a real per-gate breakdown:

```
$ timeout 20 ./target/debug/fleet gate < /dev/null
fleet: verify: running `cargo test --workspace` (budget 7.991423916s)
...
unit tests: fail: NonZeroExit(101)
mutants: fail: NonZeroExit(1)
semgrep: pass
trivy: fail: NonZeroExit(1)
recur: fail: NonZeroExit(3)
detectors: pass
policy: pass
corpus: fail: NonZeroExit(124)
fleet: verification failed: 5 failed, 0 skipped (of 8 gate(s))
EXIT:6
```

`fleet gate --id <id>` isolates one gate by name and does not hang (never did, per the audit):

```
$ ./target/debug/fleet gate --id nonexistent-id
fleet: no gate matches id "nonexistent-id"
EXIT:3
```

### 7. `fleet swarm` — cross-wiring also fixed

`docs/DX-AUDIT.md` recorded (S1-4) that a non-empty `--task` was rejected as empty whenever
`--prompt` was omitted (`--prompt` defaults to `""` per `--help`). Retested: `--task` alone now
works without `--prompt`, in the sense that it is no longer rejected as an empty prompt. But the
literal command below is a *chat-only* task — "hello world task" has no fenced-code-apply step,
so the model can only reply with prose — and `swarm` now runs an honesty check (`change_detect`)
that refuses exactly that case, correctly:

```
$ fleet swarm --repo /tmp/scratch-repo --task "hello world task" --role builder
    [builder-93932-0] spawned
    [builder-93932-0] outcome: Refused { reason: "worker reported done but left the worktree unchanged relative to its starting commit dba653f1...48 -- the adapter returned advice, not an applied change. worker response: {\"agent\":\"freelane\", ..., \"response\":\"# Hello World Task\\n\\nHere's a simple \\\"Hello World\\\" task in several programming languages:\\n\\n## Python\\n```python\\nprint(\\\"Hello, World!\\\")\\n```\\n...\", \"status\":\"done\", ...}" }
fleet: worker reported done but left the worktree unchanged relative to its starting commit dba653f1...48 -- the adapter returned advice, not an applied change. worker response: {...}
EXIT:7
```

**Why this is correct, not a regression:** the model answered with a well-formed "hello world"
reply, and the adapter reported `done` — but nothing on disk in `/tmp/scratch-repo`'s worktree
actually changed (no new commit, no diff). `swarm` compares the worktree's `HEAD` before and
after the lane ran (plus `git status` for uncommitted edits); when neither moved, a claimed
`Done` is downgraded to `Refused` with exit `7`, because "the adapter produced text" and "the
adapter applied a change" are different claims, and only the diff is proof of the second one. A
worker that actually edits and/or commits real files — not just chat prose — gets `Done`/`EXIT:0`
for the identical flow; see `crates/fleet-worker/src/spawn/change_detect/` for the check and its
tests.

**Note:** this makes a real outbound network call (a keyless LLM endpoint, per
`docs/NIGHT-PLAN.md`) — don't run it somewhere without network access, or in a loop. There is no
`swarm dispatch` subcommand; the flat `fleet swarm --repo <P> --task <T> --role <ROLE>
[--prompt <P>]` is the whole surface.

### 8. Ask what happened — `fleet status` still doesn't show task status

```
$ fleet status
concurrency_cap: 1
EXIT:0
$ fleet status --json
{
  "concurrency_cap": 1
}
EXIT:0
```
This is capacity/scheduling data — the same shape `fleet doctor` reports — not a task rollup.
`docs/DX-AUDIT.md`'s S1-5 is unchanged: there is no way to ask fleet "what did you just do" from
`status`; use `fleet ledger` (row count) instead, per the next section.

### 9. Check the evidence

```
$ fleet ledger
rows: 0
EXIT:0
$ fleet ledger --verify        # NOT "fleet ledger verify" -- that's exit 2, unrecognized argument
fleet: the ledger is empty
EXIT:7
```
`fleet ledger [--verify]` is the entire surface — no `count`/`dump`/`append` subcommands exist
(`fleet ledger count` and `fleet ledger dump` both fail with `error: unexpected argument ... found`,
exit 2). Once a command has appended real rows (e.g. `fleet run`'s `run_start` event), `--verify`
walks the hash chain:
```
$ fleet ledger
rows: 1
$ fleet ledger --verify
verified: 1/1
EXIT:0
```

---

## Every command, by real `--help` shape

### Doing work
| Command | Real shape | Status |
|---|---|---|
| `fleet plan [--model <M>]` | no positional intent; prints a fixed acceptance-check template | WORKS (does not do what old docs described — see §3) |
| `fleet sow --text <T> --intent-hash <H>` | `--text` is the literal SOW body; see §4 for the real required vocabulary | WORKS once you match the real vocabulary |
| `fleet run --repo <P> --task <T>` | no `--agent` flag | WORKS — was hanging per `DX-AUDIT.md`, fixed during this docs pass, see §5 |
| `fleet swarm --repo <P> --task <T> --role <R> [--prompt <P>]` | flat, no `dispatch` subcommand; makes a real network call | WORKS — `--task`/`--prompt` cross-wire fixed during this docs pass, see §7 |
| `fleet rollback --repo <P> --worktree <W>` | addresses by repo+worktree path, not an artifact id; `<W>` must be a path under `<P>/.worktrees` | WORKS — refuses (exit 7) outside `.worktrees`, removes cleanly inside it; see below |

### Evidence
| Command | Real shape | Status |
|---|---|---|
| `fleet status [--json]` | prints `concurrency_cap`, not a task rollup | WORKS but semantically wrong — see §8 |
| `fleet ledger [--verify]` | no `count`/`dump`/`append` subcommands | WORKS — see §9 |
| `fleet oracle` (no flags) | | WORKS — was hanging per `DX-AUDIT.md`, fixed during this docs pass, see §6 |
| `fleet adjudicate <ARTIFACT>` | positional, not `--artifact` | **NOT IMPLEMENTED** — always exit 3, "no adjudication-table fn exposed yet" |
| `fleet attest --artifact <ID>` | no `verify` subcommand | **NOT IMPLEMENTED** — always exit 3, "no builder-flow fn yet" |
| `fleet pr --repo <P> --branch <B>` | | **NOT IMPLEMENTED** — always exit 3, "no pr-emit fn exposed yet" |

### The machinery
| Command | Real shape | Status |
|---|---|---|
| `fleet roles` | no args | WORKS |
| `fleet route --role <ROLE>` | | WORKS — refuses (exit 7) if no lane has quota; message names the stage |
| `fleet role-check --role <ROLE>` | | WORKS |
| `fleet agents --repo <P> --agent-id <ID>` | no `list` subcommand; `<ID>` must be one of the repo's `.fleet/agents.toml` entries, or (if the repo has none) the built-in fallback ids `lead`/`builder`/`verifier`/`designer`/`meter` | WORKS with a valid id — see below |
| `fleet skills [--check]` | | **NOT IMPLEMENTED** — always exit 3, "skills_registry module is private" |
| `fleet lifecycle --task-id <ID> --evidence <DIR>` | no `states`/`show`/`advance` subcommands; `--evidence` is a directory path, appends to a receipts log | WORKS |
| `fleet meter --lane <L> --cost-est <N> [--settle]` | no `show`/`reserve`/`plan` subcommands; the lane must already exist in `$FLEET_STATE_DIR/meter.json` — see below | WORKS once the lane file is pre-seeded |
| `fleet graph --repo <P>` | | WORKS but silent-zero on a bad path (exit 0, `files_scanned: 0`) — same for a real empty repo, no way to distinguish |
| `fleet impact --symbol <S>` | | WORKS but accepts an empty `--symbol ""` silently (exit 0, `matching_symbols: 0`) |
| `fleet doctor` | no args | WORKS |
| `fleet completions <bash\|zsh\|fish\|elvish\|powershell>` | | WORKS |
| `fleet contract --name <N>` | | **NOT IMPLEMENTED** — always exit 3 |
| `fleet gate [--id <ID>]` | | WORKS (no-args form was hanging, fixed during this docs pass; `--id` form never hung) — see §6 |
| `fleet freeze --path <P>` | | **NOT IMPLEMENTED** — always exit 3 |
| `fleet console [--task <T>]` | | **NOT IMPLEMENTED** — always exit 3 |
| `fleet mcp --lease <L>` | | **NOT IMPLEMENTED** — always exit 3 |
| `fleet version` | no args | WORKS — `fleet-cli: 0.1.0` |

**8 of 28 commands are undisclosed stubs as of this writing**: `console`, `skills`, `adjudicate`,
`attest`, `pr`, `contract`, `freeze`, `mcp`. Every invocation (flags valid or not) returns exit 3
with `fleet: this subcommand's owning crate does not yet expose a public entry point: <reason>`.
Nothing in `fleet <cmd> --help` says so — you only find out by running it. Treat any of these 8
names you see elsewhere in this repo's docs as **not usable today**, regardless of what a `--help`
listing or a table of contents implies.

### `fleet agents`, worked example (no `.fleet/agents.toml` needed for the built-in ids)

```
$ fleet agents --repo /tmp/scratch-repo --agent-id builder
skills: {"debugging", "rust"}
system_prompt: agent=builder skills={"debugging", "rust"} capabilities=["read", "write", "test"]
EXIT:0
```
A repo with no `.fleet/agents.toml` of its own falls back to a compiled-in default roster
(`crates/fleet-worker/templates/agents.toml`) with exactly 5 ids: `lead`, `builder`, `verifier`,
`designer`, `meter`. An unrecognized id (e.g. the previous docs' `builder-1`) hits the *same* error
as a genuinely missing file — `fleet: repo .fleet/ tree is missing required file: agents.toml` —
even though the file may be present and simply not have that id. Misleading, not fixed here.

### `fleet meter`, worked example (pre-seeding the lane)

`fleet meter --lane <L> --cost-est <N>` refuses with `fleet: lane "<L>" is not configured` unless
`$FLEET_STATE_DIR/meter.json` (despite the `.json` name, a hand-rolled TSV — see
`crates/fleet-govern/src/meter_codec.rs`/`row_codec.rs`) already has a row for that lane. There is
no CLI subcommand that creates one — `fleet meter --settle` only settles an *existing*
reservation. Verified working by seeding the file directly:

```
$ printf 'fleet-govern-meter-v1\nbuilder\t1000\t0\t\t\t0\n' > "$FLEET_STATE_DIR/meter.json"
$ fleet meter --lane builder --cost-est 100
reservation: 1
EXIT:0
```
The row format is `lane\twindow\tused\treservations\tresolved_model\tunknown_observed`
(tab-separated); `window`/`used` are token counts, empty means "unknown" (never `0`).

### `fleet rollback`, worked example (worktree must be under `<repo>/.worktrees`)

```
$ git worktree add /tmp/scratch-repo/.worktrees/wt1 -b fleet/test-wt
$ fleet rollback --repo /tmp/scratch-repo --worktree /tmp/scratch-repo/.worktrees/wt1
ok: worktree removed
EXIT:0
```
Giving a worktree path *outside* `<repo>/.worktrees` — including, dangerously, the repo's own
working directory — is refused, not silently accepted:
```
$ fleet rollback --repo /tmp/scratch-repo --worktree /tmp/scratch-repo
fleet: refusing to remove /tmp/scratch-repo: not inside /tmp/scratch-repo/.worktrees -- fleet only removes worktrees it owns
EXIT:7
```

---

## Where the documented workflow genuinely stops today

1. **There is no verified link from an accepted SOW to `fleet run`/`fleet swarm` proceeding.** The
   old docs' central claim — "no work happens without a plan a human accepted" — was not
   reproduced: `fleet sow` and `fleet run` do not appear to share any state in this binary.
2. **`fleet status` cannot answer "what did fleet just do."** It reports host capacity, not a task
   or receipt rollup, regardless of `--json`.
3. **8 of 28 commands are unimplemented stubs.** `adjudicate`/`attest`/`pr` in particular mean the
   "two independent oracles, 2×2 adjudication, frozen diff and attestation" story from the old
   walkthrough's step 7 cannot be verified end to end — `run`'s own internal verify pipeline is the
   only oracle-shaped thing that currently runs.
4. **Ambient state persists across commands** (the ledger, `meter.json`, worktrees) — every example
   above was run in a fresh `$FLEET_STATE_DIR` and a scratch git repo; re-running against a
   pre-populated state directory will show different row counts and may hit different refusals.

Two defects the *previous* revision of this file recorded as open are no longer reproducible —
retested here and both now print a real message:

```
$ fleet run --repo /tmp/scratch-repo --task ""
fleet: task_id must not be empty
EXIT:7

$ fleet ledger --verify        # after hand-editing one byte of the stored ledger row
fleet: row 0 hash does not match its recomputed content hash -- tampered after write
EXIT:7
```

One new mismatch found while re-verifying: the tamper case above exits **7** (refusal), not the
**8** ("mismatch") the exit-code table above and `docs/USING-FLEET.md`'s own contract promise for
"recomputed evidence disagrees with what was stored" — this looks like exactly the case exit 8 was
defined for. Not fixed by this docs pass; flagged here since it's a real discrepancy between the
documented contract and the observed exit code, found by execution rather than by reading the
table.
